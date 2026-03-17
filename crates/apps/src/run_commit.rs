use crate::cli_command::RunTemplate;
use crate::cli_runtime::lifecycle;
use crate::runtime_compose::{
    ReportWriteInput, RuntimeComposeExecution, RuntimeComposePatchArtifact, RuntimeComposeState,
    RuntimeRunRecord,
};
use crate::storage::state::{RuntimeStateSnapshot, StateStore};
use crate::storage::trace::{TraceEventInput, TraceStore};
use axiomrunner_adapters::{RESTORE_MODE_DELETE_CREATED, RESTORE_MODE_DIR, RESTORE_MODE_FILE};
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
                write_commit_report_failure_trace(
                    compose_state,
                    trace_store,
                    actor_id,
                    intent_id,
                    kind,
                    outcome,
                    policy_code,
                    template,
                    &report_input,
                    &execution,
                    state,
                    &record,
                    "runtime_report_write_failed",
                    error.clone(),
                );
                return Err(error);
            }
        };
    all_report_patch_artifacts.extend(report_patch_artifacts.iter().cloned());
    if apply_done_conditions {
        let (updated_record, applied) = lifecycle::apply_goal_done_conditions(
            template,
            &execution,
            &report_patch_artifacts,
            std::path::Path::new(&execution.provider_cwd),
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
                        write_commit_report_failure_trace(
                            compose_state,
                            trace_store,
                            actor_id,
                            intent_id,
                            kind,
                            outcome,
                            policy_code,
                            template,
                            &report_input,
                            &execution,
                            state,
                            &record,
                            "runtime_report_write_failed",
                            error.clone(),
                        );
                        return Err(error);
                    }
                };
            all_report_patch_artifacts.extend(report_patch_artifacts.iter().cloned());
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
                    write_commit_report_failure_trace(
                        compose_state,
                        trace_store,
                        actor_id,
                        intent_id,
                        kind,
                        outcome,
                        policy_code,
                        template,
                        &report_input,
                        &execution,
                        state,
                        &record,
                        "runtime_report_write_failed",
                        error.clone(),
                    );
                    return Err(error);
                }
            };
        all_report_patch_artifacts.extend(report_patch_artifacts.iter().cloned());
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

    if let Err(error) = compose_state.remember_run_summary(&record, &execution, intent_id) {
        cleanup_commit_artifacts(
            &all_report_patch_artifacts,
            &execution_patch_artifacts,
            &execution_tool_outputs,
            record.checkpoint.as_ref(),
            record.rollback.as_ref(),
        );
        rewrite_failed_commit_visibility(
            compose_state,
            trace_store,
            actor_id,
            intent_id,
            kind,
            outcome,
            policy_code,
            template,
            &report_input,
            &execution,
            state,
            &record,
            "runtime_memory_persistence_failed",
            error.clone(),
        )?;
        return Err(format!("runtime memory persistence failed: {error}"));
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
        if let Err(memory_cleanup_error) = compose_state.forget_run_summary(intent_id) {
            return Err(format!(
                "runtime state persistence failed: {error}; memory cleanup failed: {memory_cleanup_error}"
            ));
        }
        rewrite_failed_commit_visibility(
            compose_state,
            trace_store,
            actor_id,
            intent_id,
            kind,
            outcome,
            policy_code,
            template,
            &report_input,
            &execution,
            state,
            &record,
            "runtime_state_persistence_failed",
            error.clone(),
        )?;
        return Err(format!("runtime state persistence failed: {error}"));
    }

    Ok(RunCommitOutcome {
        record,
        memory_warning: None,
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
        remove_restore_artifact(&artifact.artifact_path);
        remove_if_file(&artifact.target_path);
        let _ = fs::remove_file(&artifact.artifact_path);
    }
    for artifact in execution_patch_artifacts {
        cleanup_execution_target(artifact);
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

fn cleanup_execution_target(artifact: &RuntimeComposePatchArtifact) {
    let Ok(raw) = fs::read_to_string(&artifact.artifact_path) else {
        if artifact.before_digest.is_none() {
            remove_if_file(&artifact.target_path);
        }
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        if artifact.before_digest.is_none() {
            remove_if_file(&artifact.target_path);
        }
        return;
    };

    let restore_artifact_path = value
        .get("restore_artifact_path")
        .and_then(serde_json::Value::as_str);

    match value
        .get("restore_mode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(RESTORE_MODE_DELETE_CREATED)
    {
        RESTORE_MODE_FILE => {
            if let Some(path) = restore_artifact_path {
                let _ = restore_file_target(&artifact.target_path, path);
                let _ = fs::remove_file(path);
            }
        }
        RESTORE_MODE_DIR => {
            if let Some(path) = restore_artifact_path {
                let _ = restore_directory_target(&artifact.target_path, path);
                let _ = fs::remove_file(path);
            }
        }
        _ => remove_if_file(&artifact.target_path),
    }
}

fn remove_restore_artifact(patch_artifact_path: &str) {
    let Ok(raw) = fs::read_to_string(patch_artifact_path) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return;
    };
    if let Some(path) = value
        .get("restore_artifact_path")
        .and_then(serde_json::Value::as_str)
    {
        let _ = fs::remove_file(path);
    }
}

#[allow(clippy::too_many_arguments)]
fn write_commit_report_failure_trace(
    compose_state: &RuntimeComposeState,
    trace_store: &TraceStore,
    actor_id: &str,
    intent_id: &str,
    kind: &str,
    outcome: DecisionOutcome,
    policy_code: &str,
    template: &RunTemplate,
    report_input: &ReportWriteInput<'_>,
    execution: &RuntimeComposeExecution,
    state: &AgentState,
    prior_record: &RuntimeRunRecord,
    failure_code: &str,
    failure_detail: String,
) {
    let failed_record = failed_commit_record(prior_record, failure_code, failure_detail.clone());
    let report_artifacts = compose_state
        .write_report(template, report_input, execution, &failed_record)
        .unwrap_or_default();
    let report_written = !report_artifacts.is_empty();
    let _ = trace_store.append_intent_event(TraceEventInput {
        actor_id,
        intent_id,
        kind,
        outcome,
        policy_code,
        effect_count: report_artifacts.len(),
        state,
        execution,
        report_written,
        report_error: if report_written {
            None
        } else {
            Some(failure_detail.as_str())
        },
        patch_artifacts: &report_artifacts,
        run: &failed_record,
    });
}

fn rewrite_failed_commit_visibility(
    compose_state: &RuntimeComposeState,
    trace_store: &TraceStore,
    actor_id: &str,
    intent_id: &str,
    kind: &str,
    outcome: DecisionOutcome,
    policy_code: &str,
    template: &RunTemplate,
    report_input: &ReportWriteInput<'_>,
    execution: &RuntimeComposeExecution,
    state: &AgentState,
    prior_record: &RuntimeRunRecord,
    failure_code: &str,
    failure_detail: String,
) -> Result<(), String> {
    let _ = trace_store.remove_last_event_for_intent(intent_id);
    let failed_record = failed_commit_record(prior_record, failure_code, failure_detail);
    let report_patch_artifacts =
        compose_state.write_report(template, report_input, execution, &failed_record)?;
    trace_store.append_intent_event(TraceEventInput {
        actor_id,
        intent_id,
        kind,
        outcome,
        policy_code,
        effect_count: report_patch_artifacts.len(),
        state,
        execution,
        report_written: true,
        report_error: None,
        patch_artifacts: &report_patch_artifacts,
        run: &failed_record,
    })
}

fn failed_commit_record(
    prior_record: &RuntimeRunRecord,
    failure_code: &str,
    failure_detail: String,
) -> RuntimeRunRecord {
    let (reason, reason_code, reason_detail) =
        crate::runtime_compose::runtime_run_reason(failure_code, failure_detail);
    RuntimeRunRecord {
        verification: crate::runtime_compose::RuntimeRunVerification {
            status: "failed",
            summary: reason.clone(),
            checks: prior_record.verification.checks.clone(),
        },
        phase: crate::runtime_compose::RuntimeRunPhase::Failed,
        outcome: crate::runtime_compose::RuntimeRunOutcome::Failed,
        reason,
        reason_code,
        reason_detail,
        ..prior_record.clone()
    }
}

fn restore_file_target(target_path: &str, restore_artifact_path: &str) -> Result<(), String> {
    let raw = fs::read_to_string(restore_artifact_path)
        .map_err(|error| format!("read restore artifact failed: {error}"))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| format!("parse restore artifact failed: {error}"))?;
    let contents_hex = value
        .get("contents_hex")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| String::from("restore artifact missing contents_hex"))?;
    let bytes = decode_hex(contents_hex)?;
    if let Some(parent) = Path::new(target_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(target_path, bytes).map_err(|error| format!("restore target failed: {error}"))
}

fn restore_directory_target(target_path: &str, restore_artifact_path: &str) -> Result<(), String> {
    let raw = fs::read_to_string(restore_artifact_path)
        .map_err(|error| format!("read restore artifact failed: {error}"))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| format!("parse restore artifact failed: {error}"))?;
    let entries = value
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| String::from("restore artifact missing entries"))?;

    let target = Path::new(target_path);
    let _ = fs::remove_dir_all(target);
    fs::create_dir_all(target).map_err(|error| format!("restore root failed: {error}"))?;

    for entry in entries {
        let rel = entry
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| String::from("restore entry missing path"))?;
        if rel == "." {
            continue;
        }
        let path = target.join(rel);
        match entry
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("file")
        {
            "dir" => {
                fs::create_dir_all(&path)
                    .map_err(|error| format!("restore directory failed: {error}"))?;
            }
            "file" => {
                let contents_hex = entry
                    .get("contents_hex")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| String::from("restore file missing contents_hex"))?;
                let bytes = decode_hex(contents_hex)?;
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| format!("restore file parent failed: {error}"))?;
                }
                fs::write(&path, bytes).map_err(|error| format!("restore file failed: {error}"))?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) {
        return Err(String::from("hex length must be even"));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|error| format!("invalid hex: {error}"))
        })
        .collect()
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
        compose_state_with_memory(repo_root, artifact_root, None)
    }

    fn compose_state_with_memory(
        repo_root: &Path,
        artifact_root: &Path,
        memory_path: Option<PathBuf>,
    ) -> RuntimeComposeState {
        let mut state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path,
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

    fn make_restore_patch_artifact(
        root: &Path,
        name: &str,
        target_contents: &str,
        restore_contents: &str,
    ) -> RuntimeComposePatchArtifact {
        let target_path = root.join(format!("{name}.txt"));
        fs::write(&target_path, target_contents).expect("target should exist");
        let restore_dir = root.join(".axiomrunner").join("restores");
        fs::create_dir_all(&restore_dir).expect("restore dir should exist");
        let restore_artifact_path = restore_dir.join(format!("{name}.restore.json"));
        fs::write(
            &restore_artifact_path,
            serde_json::json!({
                "schema": "axiomrunner.restore.file.v1",
                "target_path": target_path.display().to_string(),
                "contents_hex": restore_contents
                    .as_bytes()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            })
            .to_string(),
        )
        .expect("restore artifact should exist");

        let patch_dir = root.join(".axiomrunner").join("patches");
        fs::create_dir_all(&patch_dir).expect("patch dir should exist");
        let artifact_path = patch_dir.join(format!("{name}.json"));
        fs::write(
            &artifact_path,
            serde_json::json!({
                "schema": "axiomrunner.patch.v2",
                "restore_mode": "restore_file",
                "restore_artifact_path": restore_artifact_path.display().to_string(),
            })
            .to_string(),
        )
        .expect("patch artifact should exist");

        RuntimeComposePatchArtifact {
            operation: String::from("overwrite"),
            target_path: target_path.display().to_string(),
            artifact_path: artifact_path.display().to_string(),
            before_digest: Some(String::from("before")),
            after_digest: Some(String::from("after")),
            before_excerpt: Some(String::from("before")),
            after_excerpt: Some(String::from("after")),
            unified_diff: Some(String::from("diff")),
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
        fs::write(&execution_patch.target_path, "created during run\n")
            .expect("execution target should exist");
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
                .len()
                == 1
        );
        assert!(
            artifact_root
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
        assert!(
            !Path::new(&execution_patch.target_path).exists(),
            "execution target should be rolled back on failed commit"
        );
        let latest = trace_store
            .latest_event()
            .expect("trace should load")
            .expect("failure event should remain");
        assert_eq!(
            latest.run.as_ref().map(|run| run.reason_code.as_str()),
            Some("runtime_state_persistence_failed")
        );
        let report =
            fs::read_to_string(artifact_root.join(".axiomrunner/artifacts/cli-1.report.md"))
                .expect("failure report should remain");
        assert!(report.contains("run_reason_code=runtime_state_persistence_failed"));

        let _ = fs::remove_dir_all(&repo_root);
        let _ = fs::remove_dir_all(&artifact_root);
        let _ = fs::remove_file(state_parent_blocker);
    }

    #[test]
    fn commit_restores_overwritten_execution_target_when_snapshot_save_fails() {
        let repo_root = unique_path("restore-repo", "dir");
        let artifact_root = unique_path("restore-artifacts", "dir");
        let state_parent_blocker = unique_path("restore-parent", "file");
        init_git_repo(&repo_root);
        fs::write(&state_parent_blocker, "block").expect("state blocker should exist");
        let state_path = state_parent_blocker.join("state.snapshot");

        let mut compose = compose_state(&repo_root, &artifact_root);
        let template = goal_template(&repo_root);
        let run_record = record(&compose, &template, "run-1", RuntimeRunOutcome::Failed);
        let execution_patch =
            make_restore_patch_artifact(&artifact_root, "restored", "after\n", "before\n");
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
                    tool_outputs: vec![command_output],
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
        assert_eq!(
            fs::read_to_string(&execution_patch.target_path).expect("target should be readable"),
            "before\n"
        );

        let _ = fs::remove_dir_all(&repo_root);
        let _ = fs::remove_dir_all(&artifact_root);
        let _ = fs::remove_file(state_parent_blocker);
    }

    #[test]
    fn commit_promotes_memory_store_failure_to_transaction_error() {
        let repo_root = unique_path("memory-repo", "dir");
        let artifact_root = unique_path("memory-artifacts", "dir");
        let state_path = unique_path("memory-state", "snapshot");
        let memory_path = unique_path("memory-db", "md");
        init_git_repo(&repo_root);

        let mut compose =
            compose_state_with_memory(&repo_root, &artifact_root, Some(memory_path.clone()));
        fs::create_dir_all(memory_path.with_extension("tmp"))
            .expect("memory temp path blocker should exist");
        let template = goal_template(&repo_root);
        let run_record = record(&compose, &template, "run-1", RuntimeRunOutcome::Success);
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
                    tool_outputs: vec![command_output],
                    patch_artifacts: vec![execution_patch],
                },
                record: run_record,
                state: &AgentState::default(),
                snapshot: None,
                apply_done_conditions: false,
                write_checkpoint: true,
            },
        )
        .expect_err("memory store failure should fail the commit");

        assert!(err.contains("memory"), "error was: {err}");
        assert!(
            trace_store
                .load_events()
                .expect("trace should stay readable")
                .len()
                == 1,
            "trace must retain the final failed commit event"
        );
        let latest = trace_store
            .latest_event()
            .expect("trace should load")
            .expect("failure event should remain");
        assert_eq!(
            latest.run.as_ref().map(|run| run.reason_code.as_str()),
            Some("runtime_memory_persistence_failed")
        );
        let report =
            fs::read_to_string(artifact_root.join(".axiomrunner/artifacts/cli-1.report.md"))
                .expect("failure report should remain");
        assert!(report.contains("run_reason_code=runtime_memory_persistence_failed"));

        let _ = fs::remove_dir_all(&repo_root);
        let _ = fs::remove_dir_all(&artifact_root);
        let _ = fs::remove_dir_all(memory_path.with_extension("tmp"));
        let _ = fs::remove_file(memory_path);
    }

    #[test]
    fn commit_removes_written_memory_entry_when_snapshot_save_fails() {
        let repo_root = unique_path("memory-snapshot-repo", "dir");
        let artifact_root = unique_path("memory-snapshot-artifacts", "dir");
        let state_parent_blocker = unique_path("memory-snapshot-parent", "file");
        let memory_path = unique_path("memory-snapshot-db", "md");
        init_git_repo(&repo_root);
        fs::write(&state_parent_blocker, "block").expect("state blocker should exist");
        let state_path = state_parent_blocker.join("state.snapshot");

        let mut compose =
            compose_state_with_memory(&repo_root, &artifact_root, Some(memory_path.clone()));
        let template = goal_template(&repo_root);
        let run_record = record(&compose, &template, "run-1", RuntimeRunOutcome::Success);
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
                    tool_outputs: vec![command_output],
                    patch_artifacts: vec![execution_patch],
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
        let memory_contents =
            fs::read_to_string(&memory_path).expect("memory file should remain readable");
        assert!(
            !memory_contents.contains("key_hex="),
            "memory entry should be deleted after snapshot failure"
        );

        let _ = fs::remove_dir_all(&repo_root);
        let _ = fs::remove_dir_all(&artifact_root);
        let _ = fs::remove_file(state_parent_blocker);
        let _ = fs::remove_file(memory_path);
    }
}
