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

    if write_checkpoint {
        record.checkpoint =
            compose_state.write_checkpoint_metadata(intent_id, &record.plan.run_id)?;
    }

    let mut report_patch_artifacts = match compose_state.write_report(
        template,
        &report_input,
        &execution,
        &record,
    ) {
        Ok(artifacts) => artifacts,
        Err(error) => {
            cleanup_commit_artifacts(&[], record.checkpoint.as_ref(), record.rollback.as_ref());
            return Err(error);
        }
    };
    if apply_done_conditions {
        let (updated_record, applied) = lifecycle::apply_goal_done_conditions(
            template,
            &execution,
            &report_patch_artifacts,
            record,
        );
        record = updated_record;
        if applied {
            report_patch_artifacts = match compose_state.write_report(
                template,
                &report_input,
                &execution,
                &record,
            ) {
                Ok(artifacts) => artifacts,
                Err(error) => {
                    cleanup_commit_artifacts(
                        &report_patch_artifacts,
                        record.checkpoint.as_ref(),
                        record.rollback.as_ref(),
                    );
                    return Err(error);
                }
            };
        }
    }

    record.rollback = compose_state.write_rollback_metadata(intent_id, &execution, &record)?;
    if record.rollback.is_some() {
        report_patch_artifacts = match compose_state.write_report(
            template,
            &report_input,
            &execution,
            &record,
        ) {
            Ok(artifacts) => artifacts,
            Err(error) => {
                cleanup_commit_artifacts(
                    &report_patch_artifacts,
                    record.checkpoint.as_ref(),
                    record.rollback.as_ref(),
                );
                return Err(error);
            }
        };
    }

    let mut patch_artifacts = execution.patch_artifacts.clone();
    patch_artifacts.extend(report_patch_artifacts.clone());

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
            &report_patch_artifacts,
            record.checkpoint.as_ref(),
            record.rollback.as_ref(),
        );
        return Err(format!("runtime trace error: {error}"));
    }

    if let Some(snapshot) = snapshot
        && let Err(error) = state_store.save_snapshot(&snapshot)
    {
        cleanup_commit_artifacts(
            &report_patch_artifacts,
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
    patch_artifacts: &[RuntimeComposePatchArtifact],
    checkpoint: Option<&crate::runtime_compose::RuntimeRunCheckpointMetadata>,
    rollback: Option<&crate::runtime_compose::RuntimeRunRollbackMetadata>,
) {
    for artifact in patch_artifacts {
        let _ = fs::remove_file(&artifact.artifact_path);
    }
    if let Some(checkpoint) = checkpoint {
        let _ = fs::remove_file(&checkpoint.metadata_path);
    }
    if let Some(rollback) = rollback {
        let _ = fs::remove_file(&rollback.metadata_path);
    }
}
