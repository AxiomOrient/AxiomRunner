use crate::cli_command::RunTemplate;
use crate::cli_runtime::lifecycle;
use crate::runtime_compose::{
    ReportWriteInput, RuntimeComposeExecution, RuntimeComposePatchArtifact, RuntimeComposeState,
    RuntimeRunRecord,
};
use crate::storage::state::{RuntimeStateSnapshot, StateStore};
use crate::storage::trace::{TraceEventInput, TraceStore};
use axiomrunner_core::{AgentState, DecisionOutcome};
use std::fs;
use std::path::Path;

pub struct PreparedRunCommit<'a> {
    pub actor_id: &'a str,
    pub intent_id: &'a str,
    pub kind: &'a str,
    pub outcome: DecisionOutcome,
    pub policy_code: &'a str,
    pub template: &'a RunTemplate,
    pub report_input: ReportWriteInput<'a>,
    pub execution: RuntimeComposeExecution,
    pub record: RuntimeRunRecord,
    pub state: &'a AgentState,
    pub snapshot: Option<RuntimeStateSnapshot>,
    pub apply_done_conditions: bool,
    pub write_checkpoint: bool,
}

#[derive(Debug)]
pub struct RunCommitOutcome {
    pub record: RuntimeRunRecord,
    pub memory_warning: Option<String>,
}

pub fn commit_prepared_run(
    compose_state: &mut RuntimeComposeState,
    trace_store: &TraceStore,
    state_store: &StateStore,
    prepared: PreparedRunCommit<'_>,
) -> Result<RunCommitOutcome, String> {
    let PreparedRunCommit {
        actor_id,
        intent_id,
        kind,
        outcome,
        policy_code,
        template,
        report_input,
        execution,
        mut record,
        state,
        snapshot,
        apply_done_conditions,
        write_checkpoint,
    } = prepared;

    let execution_patch_artifacts = execution.patch_artifacts.clone();
    let execution_tool_outputs = execution.tool_outputs.clone();
    let mut all_report_patch_artifacts = Vec::new();

    if write_checkpoint {
        record.checkpoint =
            compose_state.write_checkpoint_metadata(intent_id, &record.plan.run_id)?;
    }

    let mut report_patch_artifacts =
        match compose_state.write_report(template, &report_input, &execution, &record) {
            Ok(artifacts) => artifacts,
            Err(error) => {
                cleanup_commit_artifacts(
                    &[],
                    &execution_patch_artifacts,
                    &execution_tool_outputs,
                    record.checkpoint.as_ref(),
                    record.rollback.as_ref(),
                );
                return Err(error);
            }
        };
    all_report_patch_artifacts.extend(report_patch_artifacts.clone());
    if apply_done_conditions {
        let (updated_record, applied) = lifecycle::apply_goal_done_conditions(
            template,
            &execution,
            &report_patch_artifacts,
            record,
        );
        record = updated_record;
        if applied {
            report_patch_artifacts =
                match compose_state.write_report(template, &report_input, &execution, &record) {
                    Ok(artifacts) => artifacts,
                    Err(error) => {
                        cleanup_commit_artifacts(
                            &all_report_patch_artifacts,
                            &execution_patch_artifacts,
                            &execution_tool_outputs,
                            record.checkpoint.as_ref(),
                            record.rollback.as_ref(),
                        );
                        return Err(error);
                    }
                };
            all_report_patch_artifacts.extend(report_patch_artifacts.clone());
        }
    }

    record.rollback = match compose_state.write_rollback_metadata(intent_id, &execution, &record) {
        Ok(rollback) => rollback,
        Err(error) => {
            cleanup_commit_artifacts(
                &all_report_patch_artifacts,
                &execution_patch_artifacts,
                &execution_tool_outputs,
                record.checkpoint.as_ref(),
                record.rollback.as_ref(),
            );
            return Err(error);
        }
    };
    if record.rollback.is_some() {
        report_patch_artifacts =
            match compose_state.write_report(template, &report_input, &execution, &record) {
                Ok(artifacts) => artifacts,
                Err(error) => {
                    cleanup_commit_artifacts(
                        &all_report_patch_artifacts,
                        &execution_patch_artifacts,
                        &execution_tool_outputs,
                        record.checkpoint.as_ref(),
                        record.rollback.as_ref(),
                    );
                    return Err(error);
                }
            };
        all_report_patch_artifacts.extend(report_patch_artifacts.clone());
    }

    let mut patch_artifacts = execution_patch_artifacts.clone();
    patch_artifacts.extend(report_patch_artifacts.iter().cloned());

    if let Err(error) = trace_store.append_intent_event(TraceEventInput {
        actor_id,
        intent_id,
        kind,
        outcome,
        policy_code,
        effect_count: patch_artifacts.len(),
        state,
        execution: &execution,
        report_written: true,
        report_error: None,
        patch_artifacts: &patch_artifacts,
        run: &record,
    }) {
        cleanup_commit_artifacts(
            &all_report_patch_artifacts,
            &execution_patch_artifacts,
            &execution_tool_outputs,
            record.checkpoint.as_ref(),
            record.rollback.as_ref(),
        );
        return Err(format!("runtime trace error: {error}"));
    }

    if let Some(snapshot) = snapshot
        && let Err(error) = state_store.save_snapshot(&snapshot)
    {
        cleanup_commit_artifacts(
            &all_report_patch_artifacts,
            &execution_patch_artifacts,
            &execution_tool_outputs,
            record.checkpoint.as_ref(),
            record.rollback.as_ref(),
        );
        let trace_cleanup = trace_store
            .remove_last_event_for_intent(intent_id)
            .map_err(|cleanup_error| {
                format!(
                    "runtime state persistence failed: {error}; trace cleanup failed: {cleanup_error}"
                )
            })?;
        if !trace_cleanup {
            return Err(format!(
                "runtime state persistence failed: {error}; trace cleanup skipped"
            ));
        }
        return Err(format!("runtime state persistence failed: {error}"));
    }

    let memory_warning = compose_state
        .remember_run_summary(&record, &execution, intent_id)
        .err()
        .map(|error| format!("runtime memory recall warning: {error}"));

    Ok(RunCommitOutcome {
        record,
        memory_warning,
    })
}

fn cleanup_commit_artifacts(
    report_patch_artifacts: &[RuntimeComposePatchArtifact],
    execution_patch_artifacts: &[RuntimeComposePatchArtifact],
    execution_tool_outputs: &[String],
    checkpoint: Option<&crate::runtime_compose::RuntimeRunCheckpointMetadata>,
    rollback: Option<&crate::runtime_compose::RuntimeRunRollbackMetadata>,
) {
    for artifact in report_patch_artifacts {
        remove_if_file(&artifact.target_path);
        let _ = fs::remove_file(&artifact.artifact_path);
    }
    for artifact in execution_patch_artifacts {
        let _ = fs::remove_file(&artifact.artifact_path);
    }
    for artifact_path in tool_output_artifact_paths(execution_tool_outputs) {
        remove_if_file(&artifact_path);
    }
    if let Some(checkpoint) = checkpoint {
        let _ = fs::remove_file(&checkpoint.metadata_path);
    }
    if let Some(rollback) = rollback {
        let _ = fs::remove_file(&rollback.metadata_path);
    }
}

fn tool_output_artifact_paths(tool_outputs: &[String]) -> Vec<String> {
    tool_outputs
        .iter()
        .filter_map(|output| serde_json::from_str::<serde_json::Value>(output).ok())
        .filter_map(|value| {
            value
                .get("artifact_path")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

fn remove_if_file(path: &str) {
    if Path::new(path).is_file() {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::{PreparedRunCommit, commit_prepared_run};
    use crate::cli_command::GoalFileTemplate;
    use crate::runtime_compose::{
        ReportWriteInput, RuntimeComposeConfig, RuntimeComposeExecution,
        RuntimeComposePatchArtifact, RuntimeComposeState, RuntimeComposeStep, RuntimeRunOutcome,
        RuntimeRunPhase, RuntimeRunRecord, RuntimeRunRepair, RuntimeRunVerification,
    };
    use crate::storage::state::{RuntimeStateSnapshot, StateStore};
    use crate::storage::trace::TraceStore;
    use axiomrunner_core::{
        AgentState, DecisionOutcome, DoneCondition, DoneConditionEvidence, RunApprovalMode,
        RunBudget, RunGoal, VerificationCheck,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_path(label: &str, extension: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiomrunner-run-commit-{label}-{}-{tick}.{extension}",
            std::process::id()
        ))
    }

    fn init_git_repo(path: &Path) {
        fs::create_dir_all(path).expect("repo directory should exist");
        run_checked_command("git", &["init"], path);
        run_checked_command("git", &["config", "user.email", "test@example.com"], path);
        run_checked_command("git", &["config", "user.name", "AxiomRunner Test"], path);
        fs::write(path.join("README.md"), "fixture\n").expect("fixture file should exist");
        run_checked_command("git", &["add", "README.md"], path);
        run_checked_command("git", &["commit", "-m", "init"], path);
    }

    fn run_checked_command(program: &str, args: &[&str], cwd: &Path) {
        let output = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("command should run");
        assert!(
            output.status.success(),
            "command failed: {} {:?}\nstdout:\n{}\nstderr:\n{}",
            program,
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn goal_template(workspace_root: &Path) -> GoalFileTemplate {
        GoalFileTemplate {
            path: String::from("GOAL.json"),
            goal: RunGoal {
                summary: String::from("test goal"),
                workspace_root: workspace_root.display().to_string(),
                constraints: Vec::new(),
                done_conditions: vec![DoneCondition {
                    label: String::from("report"),
                    evidence: DoneConditionEvidence::ReportArtifactExists,
                }],
                verification_checks: vec![VerificationCheck {
                    label: String::from("pwd"),
                    detail: String::from("pwd"),
                }],
                budget: RunBudget::bounded(5, 10, 8000),
                approval_mode: RunApprovalMode::Never,
            },
            workflow_pack: None,
        }
    }

    fn record(
        compose: &RuntimeComposeState,
        template: &GoalFileTemplate,
        run_id: &str,
        outcome: RuntimeRunOutcome,
    ) -> RuntimeRunRecord {
        RuntimeRunRecord {
            plan: compose.plan_template(template, run_id, "cli-1"),
            step_journal: vec![crate::runtime_compose::RuntimeRunStepRecord {
                id: format!("{run_id}/step-1"),
                label: String::from("finalize"),
                phase: String::from("failed"),
                status: String::from("failed"),
                evidence: String::from("failed"),
                failure: Some(String::from("failed")),
            }],
            verification: RuntimeRunVerification {
                status: "failed",
                summary: String::from("verification failed"),
                checks: vec![String::from("check")],
            },
            repair: RuntimeRunRepair::skipped("none"),
            checkpoint: None,
            rollback: None,
            elapsed_ms: 1,
            phase: RuntimeRunPhase::Failed,
            outcome,
            reason: String::from("verification_failed"),
            reason_code: String::from("verification_failed"),
            reason_detail: String::from("none"),
        }
    }

    fn compose_state(repo_root: &Path, artifact_root: &Path) -> RuntimeComposeState {
        let mut state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path: None,
            tool_workspace: Some(repo_root.to_path_buf()),
            artifact_workspace: Some(artifact_root.to_path_buf()),
            git_worktree_isolation: true,
            tool_log_path: artifact_root.join("runtime.log").display().to_string(),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 1024,
            command_timeout_ms: 1_000,
            command_allowlist: None,
        })
        .expect("compose state should initialize");
        state
            .prepare_execution_workspace("run-1")
            .expect("execution workspace should bind");
        state
    }

    fn make_patch_artifact(root: &Path, name: &str) -> RuntimeComposePatchArtifact {
        let patches_dir = root.join(".axiomrunner").join("patches");
        fs::create_dir_all(&patches_dir).expect("patch dir should exist");
        let artifact_path = patches_dir.join(format!("{name}.json"));
        fs::write(&artifact_path, "{}").expect("patch artifact should exist");
        RuntimeComposePatchArtifact {
            operation: String::from("overwrite"),
            target_path: root.join(format!("{name}.txt")).display().to_string(),
            artifact_path: artifact_path.display().to_string(),
            before_digest: None,
            after_digest: None,
            before_excerpt: None,
            after_excerpt: None,
            unified_diff: None,
        }
    }

    fn make_command_output(root: &Path, name: &str) -> String {
        let command_dir = root.join(".axiomrunner").join("commands");
        fs::create_dir_all(&command_dir).expect("command dir should exist");
        let artifact_path = command_dir.join(format!("{name}.json"));
        fs::write(&artifact_path, "{}").expect("command artifact should exist");
        serde_json::json!({
            "label": name,
            "strength": "strong",
            "exit_code": 0,
            "command": "pwd",
            "artifact_path": artifact_path.display().to_string(),
            "expectation": "pwd exits 0"
        })
        .to_string()
    }

    #[test]
    fn commit_cleans_report_checkpoint_and_execution_artifacts_when_rollback_write_fails() {
        let repo_root = unique_path("rollback-repo", "dir");
        let artifact_root = unique_path("rollback-artifacts", "dir");
        let state_path = unique_path("rollback-state", "snapshot");
        init_git_repo(&repo_root);
        fs::create_dir_all(artifact_root.join(".axiomrunner").join("artifacts"))
            .expect("artifact dir should exist");
        fs::create_dir_all(
            artifact_root
                .join(".axiomrunner")
                .join("artifacts")
                .join("cli-1.rollback.json"),
        )
        .expect("rollback blocker dir should exist");

        let mut compose = compose_state(&repo_root, &artifact_root);
        let template = goal_template(&repo_root);
        let run_record = record(&compose, &template, "run-1", RuntimeRunOutcome::Failed);
        let execution_patch = make_patch_artifact(&artifact_root, "execution");
        let command_output = make_command_output(&artifact_root, "verify");
        let trace_store =
            TraceStore::from_workspace_root(Some(artifact_root.clone())).expect("trace store");
        let state_store = StateStore::from_app_config(&crate::config_loader::AppConfig {
            profile: String::from("prod"),
            provider: String::from("mock-local"),
            provider_model: None,
            workspace: Some(repo_root.clone()),
            state_path: Some(state_path),
            command_allowlist: None,
        })
        .expect("state store");

        let err = commit_prepared_run(
            &mut compose,
            &trace_store,
            &state_store,
            PreparedRunCommit {
                actor_id: "system",
                intent_id: "cli-1",
                kind: "goal",
                outcome: DecisionOutcome::Accepted,
                policy_code: "allowed",
                template: &template,
                report_input: ReportWriteInput {
                    intent_id: "cli-1",
                    outcome: DecisionOutcome::Accepted,
                    policy_code: "allowed",
                    effect_count: 1,
                },
                execution: RuntimeComposeExecution {
                    provider_output: None,
                    provider_cwd: repo_root.display().to_string(),
                    provider: RuntimeComposeStep::Skipped,
                    memory: RuntimeComposeStep::Skipped,
                    tool: RuntimeComposeStep::Applied,
                    tool_outputs: vec![command_output.clone()],
                    patch_artifacts: vec![execution_patch.clone()],
                },
                record: run_record,
                state: &AgentState::default(),
                snapshot: None,
                apply_done_conditions: false,
                write_checkpoint: true,
            },
        )
        .expect_err("rollback write should fail");

        assert!(err.contains("runtime_compose.rollback"));
        assert!(
            !artifact_root
                .join(".axiomrunner/artifacts/cli-1.report.md")
                .exists()
        );
        assert!(
            !artifact_root
                .join(".axiomrunner/artifacts/cli-1.checkpoint.json")
                .exists()
        );
        assert!(!Path::new(&execution_patch.artifact_path).exists());
        let command_artifact = serde_json::from_str::<serde_json::Value>(&command_output)
            .expect("command output should parse");
        assert!(
            !Path::new(
                command_artifact["artifact_path"]
                    .as_str()
                    .expect("artifact path")
            )
            .exists()
        );

        let _ = fs::remove_dir_all(&repo_root);
        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[test]
    fn commit_cleans_all_artifacts_when_trace_append_fails() {
        let repo_root = unique_path("trace-repo", "dir");
        let artifact_root = unique_path("trace-artifacts", "dir");
        let state_path = unique_path("trace-state", "snapshot");
        init_git_repo(&repo_root);
        fs::create_dir_all(artifact_root.join(".axiomrunner")).expect("artifact root should exist");
        fs::write(artifact_root.join(".axiomrunner/trace"), "block")
            .expect("trace blocker should exist");

        let mut compose = compose_state(&repo_root, &artifact_root);
        let template = goal_template(&repo_root);
        let run_record = record(&compose, &template, "run-1", RuntimeRunOutcome::Failed);
        let execution_patch = make_patch_artifact(&artifact_root, "execution");
        let command_output = make_command_output(&artifact_root, "verify");
        let trace_store =
            TraceStore::from_workspace_root(Some(artifact_root.clone())).expect("trace store");
        let state_store = StateStore::from_app_config(&crate::config_loader::AppConfig {
            profile: String::from("prod"),
            provider: String::from("mock-local"),
            provider_model: None,
            workspace: Some(repo_root.clone()),
            state_path: Some(state_path),
            command_allowlist: None,
        })
        .expect("state store");

        let err = commit_prepared_run(
            &mut compose,
            &trace_store,
            &state_store,
            PreparedRunCommit {
                actor_id: "system",
                intent_id: "cli-1",
                kind: "goal",
                outcome: DecisionOutcome::Accepted,
                policy_code: "allowed",
                template: &template,
                report_input: ReportWriteInput {
                    intent_id: "cli-1",
                    outcome: DecisionOutcome::Accepted,
                    policy_code: "allowed",
                    effect_count: 1,
                },
                execution: RuntimeComposeExecution {
                    provider_output: None,
                    provider_cwd: repo_root.display().to_string(),
                    provider: RuntimeComposeStep::Skipped,
                    memory: RuntimeComposeStep::Skipped,
                    tool: RuntimeComposeStep::Applied,
                    tool_outputs: vec![command_output.clone()],
                    patch_artifacts: vec![execution_patch.clone()],
                },
                record: run_record,
                state: &AgentState::default(),
                snapshot: None,
                apply_done_conditions: false,
                write_checkpoint: true,
            },
        )
        .expect_err("trace append should fail");

        assert!(err.contains("runtime trace error"));
        assert!(
            !artifact_root
                .join(".axiomrunner/artifacts/cli-1.report.md")
                .exists()
        );
        assert!(
            !artifact_root
                .join(".axiomrunner/artifacts/cli-1.checkpoint.json")
                .exists()
        );
        assert!(
            !artifact_root
                .join(".axiomrunner/artifacts/cli-1.rollback.json")
                .exists()
        );
        assert!(!Path::new(&execution_patch.artifact_path).exists());
        let command_artifact = serde_json::from_str::<serde_json::Value>(&command_output)
            .expect("command output should parse");
        assert!(
            !Path::new(
                command_artifact["artifact_path"]
                    .as_str()
                    .expect("artifact path")
            )
            .exists()
        );

        let _ = fs::remove_dir_all(&repo_root);
        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[test]
    fn commit_removes_trace_event_and_artifacts_when_snapshot_save_fails() {
        let repo_root = unique_path("snapshot-repo", "dir");
        let artifact_root = unique_path("snapshot-artifacts", "dir");
        let state_parent_blocker = unique_path("snapshot-parent", "file");
        init_git_repo(&repo_root);
        fs::write(&state_parent_blocker, "block").expect("state blocker should exist");
        let state_path = state_parent_blocker.join("state.snapshot");

        let mut compose = compose_state(&repo_root, &artifact_root);
        let template = goal_template(&repo_root);
        let run_record = record(&compose, &template, "run-1", RuntimeRunOutcome::Failed);
        let execution_patch = make_patch_artifact(&artifact_root, "execution");
        let command_output = make_command_output(&artifact_root, "verify");
        let trace_store =
            TraceStore::from_workspace_root(Some(artifact_root.clone())).expect("trace store");
        let state_store = StateStore::from_app_config(&crate::config_loader::AppConfig {
            profile: String::from("prod"),
            provider: String::from("mock-local"),
            provider_model: None,
            workspace: Some(repo_root.clone()),
            state_path: Some(state_path),
            command_allowlist: None,
        })
        .expect("state store");

        let err = commit_prepared_run(
            &mut compose,
            &trace_store,
            &state_store,
            PreparedRunCommit {
                actor_id: "system",
                intent_id: "cli-1",
                kind: "goal",
                outcome: DecisionOutcome::Accepted,
                policy_code: "allowed",
                template: &template,
                report_input: ReportWriteInput {
                    intent_id: "cli-1",
                    outcome: DecisionOutcome::Accepted,
                    policy_code: "allowed",
                    effect_count: 1,
                },
                execution: RuntimeComposeExecution {
                    provider_output: None,
                    provider_cwd: repo_root.display().to_string(),
                    provider: RuntimeComposeStep::Skipped,
                    memory: RuntimeComposeStep::Skipped,
                    tool: RuntimeComposeStep::Applied,
                    tool_outputs: vec![command_output.clone()],
                    patch_artifacts: vec![execution_patch.clone()],
                },
                record: run_record,
                state: &AgentState::default(),
                snapshot: Some(RuntimeStateSnapshot::default()),
                apply_done_conditions: false,
                write_checkpoint: true,
            },
        )
        .expect_err("snapshot save should fail");

        assert!(err.contains("runtime state persistence failed"));
        assert!(
            trace_store
                .load_events()
                .expect("trace should stay readable")
                .is_empty()
        );
        assert!(
            !artifact_root
                .join(".axiomrunner/artifacts/cli-1.report.md")
                .exists()
        );
        assert!(
            !artifact_root
                .join(".axiomrunner/artifacts/cli-1.checkpoint.json")
                .exists()
        );
        assert!(
            !artifact_root
                .join(".axiomrunner/artifacts/cli-1.rollback.json")
                .exists()
        );
        assert!(!Path::new(&execution_patch.artifact_path).exists());

        let _ = fs::remove_dir_all(&repo_root);
        let _ = fs::remove_dir_all(&artifact_root);
        let _ = fs::remove_file(state_parent_blocker);
    }
}
