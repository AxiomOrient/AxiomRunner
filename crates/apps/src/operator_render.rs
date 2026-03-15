use crate::display::mode_name;
use crate::doctor::DoctorReport;
use crate::runtime_compose::{run_reason_code, run_reason_detail};
use crate::status::StatusSnapshot;
use crate::trace_store::{ReplaySummary, TraceArtifactIndex, TraceIntentEvent};

pub fn render_status_lines(snapshot: &StatusSnapshot) -> Vec<String> {
    let mut lines = vec![
        format!(
            "status revision={} mode={} facts={} denied={} audit={}",
            snapshot.state.revision,
            mode_name(snapshot.state.mode),
            snapshot.state.facts,
            snapshot.state.denied,
            snapshot.state.audit
        ),
        format!(
            "status runtime provider_id={} provider_model={} provider_state={} provider_detail={} memory_enabled={} memory_state={} tool_enabled={} tool_state={}",
            snapshot.runtime.provider_id,
            snapshot.runtime.provider_model,
            snapshot.runtime.provider_state,
            snapshot.runtime.provider_detail,
            snapshot.runtime.memory_enabled,
            snapshot.runtime.memory_state,
            snapshot.runtime.tool_enabled,
            snapshot.runtime.tool_state
        ),
    ];
    if let Some(run) = &snapshot.runtime.latest_run {
        lines.push(format!(
            "status run run_id={} phase={} outcome={} reason={} execution_workspace={} verifier_state={} verifier_summary={} planned_steps={} step_count={}",
            run.run_id,
            run.phase,
            run.outcome,
            run.reason,
            run.execution_workspace,
            run.verifier_state,
            run.verifier_summary,
            run.planned_steps,
            run.step_ids.len()
        ));
    }
    if let Some(pending) = &snapshot.runtime.pending_run {
        lines.push(format!(
            "status pending_run run_id={} goal_file_path={} phase={} reason={} approval_state={} verifier_state={}",
            pending.run_id,
            pending.goal_file_path,
            pending.phase,
            pending.reason,
            pending.approval_state,
            pending.verifier_state
        ));
    }
    lines
}

pub fn render_doctor_lines(report: &DoctorReport) -> Vec<String> {
    let mut lines = vec![
        format!(
            "doctor ok={} profile={} provider_id={} provider_model={} provider_experimental={}",
            report.ok,
            report.profile,
            report.provider_id,
            report.provider_model,
            report.provider_experimental
        ),
        format!(
            "doctor state revision={} mode={} facts={} denied={} audit={}",
            report.state.revision,
            report.state.mode,
            report.state.facts,
            report.state.denied,
            report.state.audit
        ),
        format!(
            "doctor runtime provider_state={} memory_state={} tool_state={}",
            report.runtime.provider_state, report.runtime.memory_state, report.runtime.tool_state
        ),
        format!(
            "doctor detail provider_detail={} memory_detail={} tool_detail={}",
            report.runtime.provider_detail,
            report.runtime.memory_detail,
            report.runtime.tool_detail
        ),
        format!(
            "doctor async_host detail={}",
            report.runtime.async_host_detail
        ),
        format!(
            "doctor paths workspace={} state_path={} trace_events_path={} tool_log_path={}",
            report.paths.workspace,
            report.paths.state_path,
            report.paths.trace_events_path,
            report.paths.tool_log_path
        ),
    ];

    if let Some(pending) = &report.pending_run {
        lines.push(format!(
            "doctor pending_run run_id={} intent_id={} goal_file_path={} phase={} reason={} approval_state={} verifier_state={}",
            pending.run_id,
            pending.intent_id,
            pending.goal_file_path,
            pending.phase,
            pending.reason,
            pending.approval_state,
            pending.verifier_state
        ));
    }

    for check in &report.checks {
        lines.push(format!(
            "doctor check name={} state={} detail={}",
            check.name, check.state, check.detail
        ));
    }

    lines
}

pub fn render_replay_lines(
    latest: &TraceIntentEvent,
    summary: &ReplaySummary,
    artifact_index: &TraceArtifactIndex,
) -> Vec<String> {
    let mut lines = vec![
        format!(
            "replay intent_id={} count={} revision={} mode={} kind={} outcome={} policy={}",
            latest.intent_id,
            summary.intent_count,
            summary.latest_revision,
            summary.latest_mode,
            latest.kind,
            latest.outcome,
            latest.policy_code,
        ),
        format!(
            "replay stages provider={} memory={} tool={} report_written={}",
            latest.provider, latest.memory, latest.tool, latest.report_written,
        ),
        format!(
            "replay verification status={} summary={}",
            latest.verification.status, latest.verification.summary,
        ),
    ];

    if let Some(run) = &latest.run {
        lines.push(format!(
            "replay run run_id={} phase={} outcome={} reason={} reason_code={} reason_detail={} approval_state={} verifier_state={} elapsed_ms={} planned_steps={} summary={}",
            run.run_id,
            run.phase,
            run.outcome,
            run.reason,
            run_reason_code(&run.reason),
            run_reason_detail(&run.reason),
            run.approval_state,
            run.verifier_state,
            run.elapsed_ms,
            run.planned_steps,
            run.plan_summary
        ));
        let step_ids = if run.step_ids.is_empty() {
            String::from("none")
        } else {
            run.step_ids.join(",")
        };
        if run.repair.attempted {
            lines.push(format!(
                "replay repair attempted={} attempts={} status={} summary={} step_ids={}",
                run.repair.attempted,
                run.repair.attempts,
                run.repair.status,
                run.repair.summary,
                step_ids
            ));
        } else {
            lines.push(format!(
                "replay repair attempted={} status={} step_ids={}",
                run.repair.attempted, run.repair.status, step_ids
            ));
        }
        if let Some(rollback) = &run.rollback {
            lines.push(format!(
                "replay rollback metadata={} restore_path={} cleanup_path={} reason={}",
                rollback.metadata_path,
                rollback.restore_path,
                rollback.cleanup_path.as_deref().unwrap_or("none"),
                rollback.reason
            ));
        }
        if let Some(checkpoint) = &run.checkpoint {
            lines.push(format!(
                "replay checkpoint metadata={} restore_path={} execution_workspace={} reason={}",
                checkpoint.metadata_path,
                checkpoint.restore_path,
                checkpoint.execution_workspace,
                checkpoint.reason
            ));
        }
        for step in &run.step_journal {
            lines.push(format!(
                "replay step id={} phase={} status={} label={} evidence={} failure={}",
                step.id,
                step.phase,
                step.status,
                step.label,
                step.evidence,
                step.failure.as_deref().unwrap_or("none")
            ));
        }
    }

    lines.push(format!(
        "replay artifacts plan={} apply={} verify={} report={}",
        latest.artifacts.plan,
        latest.artifacts.apply,
        latest.artifacts.verify,
        latest.artifacts.report,
    ));
    if !latest.tool_outputs.is_empty() {
        lines.push(format!(
            "replay verifier_evidence count={} latest={}",
            latest.tool_outputs.len(),
            latest
                .tool_outputs
                .last()
                .map(String::as_str)
                .unwrap_or("none")
        ));
    }
    lines.push(format!(
        "replay artifact_index count={} latest_report={}",
        artifact_index.entries.len(),
        artifact_index
            .entries
            .last()
            .map(|entry| entry.report.as_str())
            .unwrap_or("none")
    ));

    if !latest.patch_artifacts.is_empty() {
        let changed_paths = latest
            .patch_artifacts
            .iter()
            .map(|patch| patch.target_path.as_str())
            .collect::<Vec<_>>();
        lines.push(format!(
            "replay changed_paths count={} paths={}",
            changed_paths.len(),
            changed_paths.join(",")
        ));
    }
    for patch in &latest.patch_artifacts {
        lines.push(format!(
            "replay patch target={} op={} artifact={} before={} after={}",
            patch.target_path,
            patch.operation,
            patch.artifact_path,
            patch.before_digest.as_deref().unwrap_or("none"),
            patch.after_digest.as_deref().unwrap_or("none"),
        ));
        if let Some(excerpt) = &patch.before_excerpt {
            lines.push(format!("replay patch before_excerpt={excerpt}"));
        }
        if let Some(excerpt) = &patch.after_excerpt {
            lines.push(format!("replay patch after_excerpt={excerpt}"));
        }
        if let Some(diff) = &patch.unified_diff {
            lines.push(format!("replay patch unified_diff={diff}"));
        }
    }
    if let Some(failure) = &latest.first_failure {
        lines.push(format!(
            "replay failure stage={} message={}",
            failure.stage, failure.message
        ));
    }
    lines.push(format!(
        "replay summary failed_intents={} false_success_intents={} false_done_intents={}",
        summary.failed_intents, summary.false_success_intents, summary.false_done_intents
    ));

    lines
}
