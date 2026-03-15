use super::*;

pub(super) fn write_report(
    state: &RuntimeComposeState,
    template: &RunTemplate,
    input: &ReportWriteInput<'_>,
    execution: &RuntimeComposeExecution,
    run: &RuntimeRunRecord,
) -> Result<Vec<RuntimeComposePatchArtifact>, String> {
    let Some(tool) = state.artifact_tool.as_ref() else {
        return Ok(Vec::new());
    };

    let base = format!(".axiomrunner/artifacts/{}", input.intent_id);
    let verification_checks = render_string_list(&run.verification.checks);
    let verifier_evidence = render_string_list(&execution.tool_outputs);
    let step_journal = render_step_journal(&run.step_journal);
    let tool_outputs = render_string_list(&execution.tool_outputs);
    let changed_paths = render_patch_targets(&execution.patch_artifacts);
    let patch_artifacts = render_patch_artifact_paths(&execution.patch_artifacts);
    let evidence = render_patch_evidence(&execution.patch_artifacts);
    let health = state.health();
    let checkpoint_summary = run
        .checkpoint
        .as_ref()
        .map(|checkpoint| {
            format!(
                "metadata={},restore_path={},execution_workspace={},reason={}",
                checkpoint.metadata_path,
                checkpoint.restore_path,
                checkpoint.execution_workspace,
                checkpoint.reason
            )
        })
        .unwrap_or_else(|| String::from("none"));
    let rollback_summary = run
        .rollback
        .as_ref()
        .map(|rollback| {
            format!(
                "metadata={},restore_path={},cleanup_path={},reason={}",
                rollback.metadata_path,
                rollback.restore_path,
                rollback.cleanup_path.as_deref().unwrap_or("none"),
                rollback.reason
            )
        })
        .unwrap_or_else(|| String::from("none"));
    let verifier_non_executed_reason = if run.verification.status == "passed" {
        String::from("none")
    } else {
        run.verification.summary.clone()
    };
    let report_summary = format!(
        "phase={} outcome={} reason={}",
        run_phase_name(run.phase),
        run_outcome_name(run.outcome),
        run.reason
    );
    let risk_summary = report_risk_summary(run);
    let next_action = report_next_action(run);
    let files = [
        (
            format!("{base}.plan.md"),
            format!(
                "# Plan\n\nphase={}\nintent_id={}\nkind={}\noutcome={}\npolicy={}\ngoal={}\nsummary={}\ndone_when={}\nplanned_steps={}\nsteps={}\n",
                run_phase_name(RuntimeRunPhase::Planning),
                input.intent_id,
                template_kind(template),
                outcome_name(input.outcome),
                input.policy_code,
                run.plan.goal,
                run.plan.summary,
                run.plan.done_when,
                run.plan.planned_steps,
                run.plan
                    .steps
                    .iter()
                    .map(|step| format!("{}:{}:{}", step.phase, step.label, step.done_when))
                    .collect::<Vec<_>>()
                    .join(" | "),
            ),
        ),
        (
            format!("{base}.apply.md"),
            format!(
                "# Apply\n\nphase={}\nprovider={}\nmemory={}\ntool={}\neffects={}\nprovider_cwd={}\nprovider_output={}\n",
                run_phase_name(RuntimeRunPhase::ExecutingStep),
                step_name(&execution.provider),
                step_name(&execution.memory),
                step_name(&execution.tool),
                input.effect_count,
                execution.provider_cwd,
                execution.provider_output.as_deref().unwrap_or("<none>"),
            ),
        ),
        (
            format!("{base}.verify.md"),
            format!(
                "# Verify\n\nphase={}\nrepair_phase={}\nstatus={}\nsummary={}\nchecks={}\nverifier_evidence={}\nrepair_attempted={}\nrepair_attempts={}\nrepair_status={}\nrepair_summary={}\nfirst_failure={}\nstep_journal={}\n",
                run_phase_name(RuntimeRunPhase::Verifying),
                run_phase_name(RuntimeRunPhase::Repairing),
                run.verification.status,
                run.verification.summary,
                verification_checks,
                verifier_evidence,
                run.repair.attempted,
                run.repair.attempts,
                run.repair.status,
                run.repair.summary,
                execution
                    .first_failure()
                    .map(|(stage, message)| format!("{stage}:{message}"))
                    .unwrap_or_else(|| String::from("none")),
                step_journal,
            ),
        ),
        (
            format!("{base}.report.md"),
            format!(
                "# Report\n\nintent_id={}\nkind={}\noutcome={}\npolicy={}\nsummary={}\nrisk={}\nnext_action={}\nrun_phase={}\nrun_outcome={}\nrun_reason={}\nrun_reason_code={}\nrun_reason_detail={}\nrun_elapsed_ms={}\nverifier_strength={}\nverifier_summary={}\nverifier_non_executed_reason={}\ncheckpoint={}\nrollback={}\nprovider_health_state={}\nprovider_health_detail={}\nprovider={}\nprovider_cwd={}\nmemory={}\ntool={}\noutputs={}\nverifier_evidence={}\nchanged_paths={}\nchanged_files={}\npatch_artifacts={}\nevidence={}\n",
                input.intent_id,
                template_kind(template),
                outcome_name(input.outcome),
                input.policy_code,
                report_summary,
                risk_summary,
                next_action,
                run_phase_name(run.phase),
                run_outcome_name(run.outcome),
                run.reason,
                run_reason_code(&run.reason),
                run_reason_detail(&run.reason),
                run.elapsed_ms,
                verifier_strength_label(&run.verification.status),
                run.verification.summary,
                verifier_non_executed_reason,
                checkpoint_summary,
                rollback_summary,
                health.provider.state,
                health.provider.detail,
                step_name(&execution.provider),
                execution.provider_cwd,
                step_name(&execution.memory),
                step_name(&execution.tool),
                tool_outputs,
                verifier_evidence,
                changed_paths,
                changed_paths,
                patch_artifacts,
                evidence,
            ),
        ),
    ];

    let mut patch_artifacts = Vec::new();
    for (path, contents) in files {
        let result = tool
            .execute(ToolRequest::FileWrite {
                path,
                contents,
                append: false,
            })
            .map_err(|error| format!("runtime_compose.report: {error}"))?;
        let ToolResult::FileWrite(FileWriteOutput { path, evidence, .. }) = result else {
            return Err(String::from(
                "runtime_compose.report: unexpected non-file-write result",
            ));
        };
        patch_artifacts.push(patch_artifact_from_write_output(path, evidence));
    }

    Ok(patch_artifacts)
}

fn isolated_workspace_context(
    state: &RuntimeComposeState,
) -> Option<(
    &std::path::PathBuf,
    &std::path::PathBuf,
    &Arc<dyn ToolAdapter>,
)> {
    let base = state.base_tool_workspace.as_ref()?;
    let exec = state.config.tool_workspace.as_ref()?;
    if base == exec {
        return None;
    }
    let tool = state.artifact_tool.as_ref()?;
    Some((base, exec, tool))
}

pub(super) fn write_checkpoint_metadata(
    state: &RuntimeComposeState,
    intent_id: &str,
    run_id: &str,
) -> Result<Option<RuntimeRunCheckpointMetadata>, String> {
    let Some((base_workspace, execution_workspace, tool)) = isolated_workspace_context(state)
    else {
        return Ok(None);
    };

    let path = format!(".axiomrunner/artifacts/{intent_id}.checkpoint.json");
    let contents = format!(
        concat!(
            "{{\n",
            "  \"schema\": \"axiomrunner.checkpoint.v1\",\n",
            "  \"run_id\": \"{}\",\n",
            "  \"intent_id\": \"{}\",\n",
            "  \"reason\": \"{}\",\n",
            "  \"restore_path\": \"{}\",\n",
            "  \"execution_workspace\": \"{}\"\n",
            "}}\n"
        ),
        escape_json_string(run_id),
        escape_json_string(intent_id),
        escape_json_string("pre_execution_isolated_worktree"),
        escape_json_string(&base_workspace.display().to_string()),
        escape_json_string(&execution_workspace.display().to_string()),
    );
    let result = tool
        .execute(ToolRequest::FileWrite {
            path,
            contents,
            append: false,
        })
        .map_err(|error| format!("runtime_compose.checkpoint: {error}"))?;
    let ToolResult::FileWrite(FileWriteOutput { path, .. }) = result else {
        return Err(String::from(
            "runtime_compose.checkpoint: unexpected non-file-write result",
        ));
    };

    Ok(Some(RuntimeRunCheckpointMetadata {
        metadata_path: path.display().to_string(),
        restore_path: base_workspace.display().to_string(),
        execution_workspace: execution_workspace.display().to_string(),
        reason: String::from("pre_execution_isolated_worktree"),
    }))
}

pub(super) fn write_rollback_metadata(
    state: &RuntimeComposeState,
    intent_id: &str,
    execution: &RuntimeComposeExecution,
    run: &RuntimeRunRecord,
) -> Result<Option<RuntimeRunRollbackMetadata>, String> {
    if !matches!(
        run.outcome,
        RuntimeRunOutcome::Failed | RuntimeRunOutcome::Blocked
    ) {
        return Ok(None);
    }

    let Some((base_workspace, execution_workspace, tool)) = isolated_workspace_context(state)
    else {
        return Ok(None);
    };

    let path = format!(".axiomrunner/artifacts/{intent_id}.rollback.json");
    let contents = format!(
        concat!(
            "{{\n",
            "  \"schema\": \"axiomrunner.rollback.v1\",\n",
            "  \"run_id\": \"{}\",\n",
            "  \"intent_id\": \"{}\",\n",
            "  \"reason\": \"{}\",\n",
            "  \"restore_path\": \"{}\",\n",
            "  \"cleanup_path\": {},\n",
            "  \"execution_workspace\": \"{}\",\n",
            "  \"provider_cwd\": \"{}\"\n",
            "}}\n"
        ),
        escape_json_string(&run.plan.run_id),
        escape_json_string(intent_id),
        escape_json_string(&run.reason),
        escape_json_string(&base_workspace.display().to_string()),
        execution_workspace
            .to_str()
            .map(escape_json_string)
            .map(|value| format!("\"{value}\""))
            .unwrap_or_else(|| String::from("null")),
        escape_json_string(&execution_workspace.display().to_string()),
        escape_json_string(&execution.provider_cwd),
    );
    let result = tool
        .execute(ToolRequest::FileWrite {
            path,
            contents,
            append: false,
        })
        .map_err(|error| format!("runtime_compose.rollback: {error}"))?;
    let ToolResult::FileWrite(FileWriteOutput { path, .. }) = result else {
        return Err(String::from(
            "runtime_compose.rollback: unexpected non-file-write result",
        ));
    };

    Ok(Some(RuntimeRunRollbackMetadata {
        metadata_path: path.display().to_string(),
        restore_path: base_workspace.display().to_string(),
        cleanup_path: Some(execution_workspace.display().to_string()),
        reason: run.reason.clone(),
    }))
}

pub(super) fn patch_artifact_from_write_output(
    path: std::path::PathBuf,
    evidence: FileMutationEvidence,
) -> RuntimeComposePatchArtifact {
    RuntimeComposePatchArtifact {
        operation: evidence.operation,
        target_path: path.display().to_string(),
        artifact_path: evidence.artifact_path.display().to_string(),
        before_digest: evidence.before_digest,
        after_digest: evidence.after_digest,
        before_excerpt: evidence.before_excerpt,
        after_excerpt: evidence.after_excerpt,
        unified_diff: evidence.unified_diff,
    }
}

fn render_string_list(items: &[String]) -> String {
    if items.is_empty() {
        String::from("none")
    } else {
        items.join(" | ")
    }
}

fn render_step_journal(steps: &[RuntimeRunStepRecord]) -> String {
    if steps.is_empty() {
        return String::from("none");
    }

    steps
        .iter()
        .map(|step| {
            format!(
                "{}:{}:{}:{}",
                step.phase, step.status, step.label, step.evidence
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn render_patch_targets(artifacts: &[RuntimeComposePatchArtifact]) -> String {
    if artifacts.is_empty() {
        return String::from("none");
    }

    artifacts
        .iter()
        .map(|artifact| artifact.target_path.as_str())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn render_patch_artifact_paths(artifacts: &[RuntimeComposePatchArtifact]) -> String {
    if artifacts.is_empty() {
        return String::from("none");
    }

    artifacts
        .iter()
        .map(|artifact| artifact.artifact_path.as_str())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn report_risk_summary(run: &RuntimeRunRecord) -> &'static str {
    match run.outcome {
        RuntimeRunOutcome::Success => "low",
        RuntimeRunOutcome::ApprovalRequired => "needs_approval",
        RuntimeRunOutcome::BudgetExhausted => "budget_blocked",
        RuntimeRunOutcome::Blocked => "blocked",
        RuntimeRunOutcome::Failed => "failed",
        RuntimeRunOutcome::Aborted => "operator_aborted",
    }
}

fn report_next_action(run: &RuntimeRunRecord) -> &'static str {
    match run.outcome {
        RuntimeRunOutcome::Success => "review report and replay evidence",
        RuntimeRunOutcome::ApprovalRequired => "approve and resume the pending run",
        RuntimeRunOutcome::BudgetExhausted => "raise budget or reduce planned scope",
        RuntimeRunOutcome::Blocked => "inspect verifier summary and unblock the run",
        RuntimeRunOutcome::Failed => "inspect failure boundary and repair before retry",
        RuntimeRunOutcome::Aborted => "decide whether to restart with a new run",
    }
}

fn render_patch_evidence(artifacts: &[RuntimeComposePatchArtifact]) -> String {
    if artifacts.is_empty() {
        return String::from("none");
    }

    artifacts
        .iter()
        .map(|artifact| {
            format!(
                "{}:{}:{}",
                artifact.operation,
                artifact.target_path,
                artifact.after_excerpt.as_deref().unwrap_or("no_excerpt")
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}
