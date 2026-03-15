use crate::cli_command::{CliCommand, RunTemplate, USAGE};
use crate::config_loader::AppConfig;
use crate::display::{mode_name, outcome_name};
use crate::doctor::{build_doctor_report, render_doctor_lines};
use crate::runtime_compose::{
    APPROVAL_STATE_REQUIRED, RUN_REASON_OPERATOR_ABORT, ReportWriteInput, RuntimeComposeConfig,
    RuntimeComposeExecution, RuntimeComposeState, RuntimeRunOutcome, RuntimeRunPhase,
    RuntimeRunRecord, RuntimeRunRepair, RuntimeRunStepRecord, RuntimeRunVerification,
    run_outcome_name, run_phase_name,
};
use crate::state_store::{PendingRunSnapshot, RuntimeStateSnapshot, StateStore};
use crate::status::{
    RuntimeStatusInput, StateStatusInput, StatusInput, StatusSnapshot, render_status_lines,
};
use crate::trace_store::{TraceEventInput, TraceStore};
use crate::workspace_lock::WorkspaceLock;
use axiomrunner_core::{AgentState, DecisionOutcome, PolicyCode};
use std::time::Instant;

mod lifecycle;

const RESUME_PENDING_APPROVAL_ONLY: &str = "resume only supports pending goal-file approval runs";
const ABORT_PENDING_CONTROL_ONLY: &str = "abort only supports pending goal-file control runs";

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedIntent {
    intent_id: String,
    kind: &'static str,
    outcome: DecisionOutcome,
    policy_code: PolicyCode,
    effect_count: usize,
}

fn report_write_input(applied: &AppliedIntent) -> ReportWriteInput<'_> {
    ReportWriteInput {
        intent_id: &applied.intent_id,
        outcome: applied.outcome,
        policy_code: applied.policy_code.as_str(),
        effect_count: applied.effect_count,
    }
}

fn accepted_goal_applied(intent_id: &str) -> AppliedIntent {
    AppliedIntent {
        intent_id: intent_id.to_owned(),
        kind: "goal",
        outcome: DecisionOutcome::Accepted,
        policy_code: PolicyCode::Allowed,
        effect_count: 0,
    }
}

pub struct CliRuntime {
    state: AgentState,
    actor_id: String,
    next_intent_seq: u64,
    next_run_seq: u64,
    pending_run: Option<PendingRunSnapshot>,
    workspace_lock: Option<WorkspaceLock>,
    compose_state: RuntimeComposeState,
    state_store: StateStore,
    trace_store: TraceStore,
}

impl CliRuntime {
    pub fn new(actor_id: String, config: &AppConfig) -> Result<Self, String> {
        Self::new_with_compose(
            actor_id,
            RuntimeComposeConfig::from_app_config(config),
            config,
        )
    }

    fn new_with_compose(
        actor_id: String,
        compose_config: RuntimeComposeConfig,
        app_config: &AppConfig,
    ) -> Result<Self, String> {
        let state_store = StateStore::from_app_config(app_config)?;
        let snapshot = state_store.load_snapshot()?;
        let trace_store = TraceStore::from_workspace_root(
            compose_config
                .artifact_workspace
                .clone()
                .or_else(|| compose_config.tool_workspace.clone()),
        )?;
        Ok(Self {
            state: snapshot.state,
            actor_id,
            next_intent_seq: snapshot.next_intent_seq,
            next_run_seq: snapshot.next_run_seq,
            pending_run: snapshot.pending_run,
            workspace_lock: None,
            compose_state: RuntimeComposeState::new(compose_config)?,
            state_store,
            trace_store,
        })
    }

    pub fn shutdown(&self) -> Result<(), String> {
        self.compose_state.shutdown()
    }

    fn apply_template(&mut self, template: &RunTemplate) -> Result<AppliedIntent, String> {
        template
            .goal
            .validate()
            .map_err(|error| format!("goal file validation failed: {error:?}"))?;
        let policy_violation = self.compose_state.constraint_policy_violation(template);
        let intent_id = self.next_intent_id();
        let (outcome, policy_code) = match policy_violation {
            Some(violation) => (DecisionOutcome::Rejected, violation.code),
            None => (DecisionOutcome::Accepted, PolicyCode::Allowed),
        };
        self.state = self.state.record_intent(
            intent_id.clone(),
            self.actor_id.clone(),
            outcome,
            policy_code,
        );
        Ok(AppliedIntent {
            intent_id,
            kind: "goal",
            outcome,
            policy_code,
            effect_count: 0,
        })
    }

    fn persist_template_result(
        &mut self,
        template: &RunTemplate,
        applied: &AppliedIntent,
    ) -> crate::runtime_compose::RuntimeComposeExecution {
        self.compose_state.apply_template(template, applied.outcome)
    }

    fn next_intent_id(&mut self) -> String {
        self.next_intent_seq = self.next_intent_seq.saturating_add(1);
        format!("cli-{}", self.next_intent_seq)
    }

    fn next_run_id(&mut self) -> String {
        self.next_run_seq = self.next_run_seq.saturating_add(1);
        format!("run-{}", self.next_run_seq)
    }

    fn runtime_snapshot(&self) -> RuntimeStateSnapshot {
        RuntimeStateSnapshot {
            state: self.state.clone(),
            next_intent_seq: self.next_intent_seq,
            next_run_seq: self.next_run_seq,
            pending_run: self.pending_run.clone(),
        }
    }

    fn restore_snapshot(&mut self, snapshot: RuntimeStateSnapshot) {
        self.state = snapshot.state;
        self.next_intent_seq = snapshot.next_intent_seq;
        self.next_run_seq = snapshot.next_run_seq;
        self.pending_run = snapshot.pending_run;
    }

    fn persist_snapshot(&self) -> Result<(), String> {
        debug_assert!(
            self.state.invariants_hold(),
            "AgentState invariant violated: revision={}",
            self.state.revision
        );
        self.state_store
            .save_snapshot(&self.runtime_snapshot())
            .map_err(|error| format!("runtime state persistence failed: {error}"))
    }

    fn ensure_workspace_lock(&mut self, command_name: &str) -> Result<(), String> {
        if self.workspace_lock.is_some() {
            return Ok(());
        }
        let workspace_root = self
            .compose_state
            .workspace_root()
            .ok_or_else(|| String::from("runtime tool workspace is not configured"))?;
        let lock = WorkspaceLock::acquire(workspace_root, command_name)?;
        self.workspace_lock = Some(lock);
        Ok(())
    }

    fn release_workspace_lock(&mut self) {
        self.workspace_lock = None;
    }
}

pub fn execute_command(
    runtime: &mut CliRuntime,
    config: &AppConfig,
    command: CliCommand,
) -> Result<(), String> {
    match command {
        CliCommand::Run(intent) => {
            runtime.ensure_workspace_lock("run")?;
            execute_intent(runtime, &intent)?
        }
        CliCommand::Replay { .. } => {
            return Err(String::from(
                "replay command should be handled before runtime execution",
            ));
        }
        CliCommand::Resume { target } => {
            runtime.ensure_workspace_lock("resume")?;
            execute_resume(runtime, &target)?
        }
        CliCommand::Abort { target } => {
            runtime.ensure_workspace_lock("abort")?;
            execute_abort(runtime, &target)?
        }
        CliCommand::Doctor { json } => print_doctor(runtime, config, json)?,
        CliCommand::Status { target } => print_status(runtime, target.as_deref()),
        CliCommand::Health => print_health(runtime, config),
        CliCommand::Help => print_usage(),
    }

    runtime.release_workspace_lock();
    Ok(())
}

fn print_usage() {
    println!("{USAGE}");
}

fn execute_intent(runtime: &mut CliRuntime, intent: &RunTemplate) -> Result<(), String> {
    let previous = runtime.runtime_snapshot();
    let started_at = Instant::now();
    let run_id = runtime.next_run_id();
    let applied = runtime.apply_template(intent)?;
    let plan = runtime
        .compose_state
        .plan_template(intent, &run_id, &applied.intent_id);
    let pre_execution_guard =
        lifecycle::goal_pre_execution_guard(intent, &plan, runtime.compose_state.max_tokens()).map(
            |summary| {
                (
                    runtime.compose_state.idle_execution(),
                    RuntimeRunVerification {
                        status: "skipped",
                        summary,
                        checks: vec![
                            format!("goal_file={}", intent.path),
                            format!("workspace_root={}", intent.goal.workspace_root),
                            format!("done_conditions={}", intent.goal.done_conditions.len()),
                            format!(
                                "verification_checks={}",
                                intent.goal.verification_checks.len()
                            ),
                        ],
                    },
                    RuntimeRunRepair::skipped("pre_execution_guard"),
                )
            },
        );
    let (execution, verification, repair, checkpoint) = if let Some(guard) = pre_execution_guard {
        (guard.0, guard.1, guard.2, None)
    } else {
        runtime.compose_state.prepare_execution_workspace(&run_id)?;
        let checkpoint = runtime
            .compose_state
            .write_checkpoint_metadata(&applied.intent_id, &run_id)?;
        let execution = runtime.persist_template_result(intent, &applied);
        let verification = lifecycle::verify_run(intent, &execution);
        let (execution, verification, repair) =
            lifecycle::run_repair_loop(runtime, intent, &plan, &applied, execution, verification);
        (execution, verification, repair, checkpoint)
    };
    let mut finalized = lifecycle::finalize_run(
        runtime.compose_state.health(),
        lifecycle::FinalizeRunInput {
            template: intent,
            plan: plan.clone(),
            applied: applied.clone(),
            execution,
            verification,
            repair,
            goal_approval_granted: false,
            elapsed_ms: lifecycle::elapsed_ms(started_at),
            requested_max_tokens: runtime.compose_state.max_tokens(),
        },
    );
    finalized.record.checkpoint = checkpoint;
    finalized.record = lifecycle::apply_goal_elapsed_budget(intent, finalized.record);
    let mut report_result = runtime.compose_state.write_report(
        intent,
        &report_write_input(&applied),
        &finalized.execution,
        &finalized.record,
    );
    if let Ok(report_patch_artifacts) = &report_result {
        let (updated_record, conditions_applied) = lifecycle::apply_goal_done_conditions(
            intent,
            &finalized.execution,
            report_patch_artifacts,
            finalized.record,
        );
        finalized.record = updated_record;
        if conditions_applied {
            report_result = runtime.compose_state.write_report(
                intent,
                &report_write_input(&applied),
                &finalized.execution,
                &finalized.record,
            );
        }
    }
    let report_error = report_result.as_ref().err().cloned();
    if let Some(error) = report_error.as_deref() {
        finalized.record.phase = RuntimeRunPhase::Failed;
        finalized.record.outcome = RuntimeRunOutcome::Failed;
        finalized.record.reason = format!("report_write_failed:{error}");
    }
    finalized.record.rollback = runtime.compose_state.write_rollback_metadata(
        &applied.intent_id,
        &finalized.execution,
        &finalized.record,
    )?;
    if finalized.record.rollback.is_some() {
        report_result = runtime.compose_state.write_report(
            intent,
            &report_write_input(&applied),
            &finalized.execution,
            &finalized.record,
        );
    }

    let mut patch_artifacts = finalized.execution.patch_artifacts.clone();
    if let Ok(report_patch_artifacts) = &report_result {
        patch_artifacts.extend(report_patch_artifacts.clone());
    }
    if let Err(error) = runtime.trace_store.append_intent_event(TraceEventInput {
        actor_id: &runtime.actor_id,
        intent_id: &applied.intent_id,
        kind: applied.kind,
        outcome: applied.outcome,
        policy_code: applied.policy_code.as_str(),
        effect_count: patch_artifacts.len(),
        state: &runtime.state,
        execution: &finalized.execution,
        report_written: report_error.is_none(),
        report_error: report_error.as_deref(),
        patch_artifacts: &patch_artifacts,
        run: &finalized.record,
    }) {
        runtime.restore_snapshot(previous);
        return Err(format!("runtime trace error: {error}"));
    }
    if let Some(error) = report_error {
        runtime.restore_snapshot(previous);
        return Err(error);
    }
    if let Err(error) = runtime.compose_state.remember_run_summary(
        &finalized.record,
        &finalized.execution,
        &applied.intent_id,
    ) {
        runtime.restore_snapshot(previous);
        return Err(format!("runtime memory recall error: {error}"));
    }
    if let Some((stage, message)) = finalized.execution.first_failure()
        && finalized.record.outcome != RuntimeRunOutcome::BudgetExhausted
    {
        runtime.restore_snapshot(previous);
        return Err(format!(
            "runtime execution failed intent_id={} stage={} error={}",
            applied.intent_id, stage, message
        ));
    }
    if matches!(
        finalized.record.outcome,
        RuntimeRunOutcome::Failed | RuntimeRunOutcome::Aborted
    ) {
        runtime.restore_snapshot(previous);
        return Err(format!(
            "runtime execution failed intent_id={} stage=run error={}",
            applied.intent_id, finalized.record.reason
        ));
    }
    runtime.pending_run = pending_run_snapshot(intent, &applied, &finalized.record);
    runtime.persist_snapshot()?;

    print_intent_result(&applied);
    print_run_result(&applied.intent_id, &finalized.record);
    Ok(())
}

fn execute_resume(runtime: &mut CliRuntime, target: &str) -> Result<(), String> {
    let started_at = Instant::now();
    let pending = pending_resume_for_target(runtime, target)?.clone();
    runtime
        .compose_state
        .prepare_execution_workspace(&pending.run_id)?;
    let checkpoint = runtime
        .compose_state
        .write_checkpoint_metadata(&pending.intent_id, &pending.run_id)?;
    let template = load_pending_goal_template(&pending.goal_file_path)?;
    let plan = runtime
        .compose_state
        .plan_template(&template, &pending.run_id, &pending.intent_id);
    let execution = runtime
        .compose_state
        .apply_template(&template, DecisionOutcome::Accepted);
    let resume_applied = accepted_goal_applied(&pending.intent_id);
    let verification = lifecycle::verify_run(&template, &execution);
    let (execution, verification, repair) = lifecycle::run_repair_loop(
        runtime,
        &template,
        &plan,
        &resume_applied,
        execution,
        verification,
    );
    let mut finalized = lifecycle::finalize_run(
        runtime.compose_state.health(),
        lifecycle::FinalizeRunInput {
            template: &template,
            plan: plan.clone(),
            applied: resume_applied.clone(),
            execution,
            verification,
            repair,
            goal_approval_granted: true,
            elapsed_ms: lifecycle::elapsed_ms(started_at),
            requested_max_tokens: runtime.compose_state.max_tokens(),
        },
    );
    finalized.record.checkpoint = checkpoint;
    finalized.record = lifecycle::apply_goal_elapsed_budget(&template, finalized.record);
    let mut patch_artifacts = runtime.compose_state.write_report(
        &template,
        &report_write_input(&resume_applied),
        &finalized.execution,
        &finalized.record,
    )?;
    let (updated_record, conditions_applied) = lifecycle::apply_goal_done_conditions(
        &template,
        &finalized.execution,
        &patch_artifacts,
        finalized.record,
    );
    finalized.record = updated_record;
    if conditions_applied {
        patch_artifacts = runtime.compose_state.write_report(
            &template,
            &report_write_input(&resume_applied),
            &finalized.execution,
            &finalized.record,
        )?;
    }
    finalized.record.rollback = runtime.compose_state.write_rollback_metadata(
        &pending.intent_id,
        &finalized.execution,
        &finalized.record,
    )?;
    patch_artifacts = rewrite_report_for_rollback(
        &runtime.compose_state,
        &template,
        &report_write_input(&resume_applied),
        &finalized.execution,
        &finalized.record,
        patch_artifacts,
    )?;
    runtime.trace_store.append_intent_event(TraceEventInput {
        actor_id: &runtime.actor_id,
        intent_id: &pending.intent_id,
        kind: "goal",
        outcome: DecisionOutcome::Accepted,
        policy_code: PolicyCode::Allowed.as_str(),
        effect_count: patch_artifacts.len(),
        state: &runtime.state,
        execution: &finalized.execution,
        report_written: true,
        report_error: None,
        patch_artifacts: &patch_artifacts,
        run: &finalized.record,
    })?;
    runtime.pending_run = None;
    runtime.persist_snapshot()?;
    println!(
        "resume run_id={} phase={} outcome={} reason={}",
        pending.run_id,
        run_phase_name(finalized.record.phase),
        run_outcome_name(finalized.record.outcome),
        finalized.record.reason
    );
    Ok(())
}

fn execute_abort(runtime: &mut CliRuntime, target: &str) -> Result<(), String> {
    let started_at = Instant::now();
    let pending = pending_abort_for_target(runtime, target)?.clone();
    let template = load_pending_goal_template(&pending.goal_file_path)?;
    let plan = runtime
        .compose_state
        .plan_template(&template, &pending.run_id, &pending.intent_id);
    let execution = runtime.compose_state.idle_execution();
    let verification = RuntimeRunVerification {
        status: "passed",
        summary: String::from(RUN_REASON_OPERATOR_ABORT),
        checks: vec![String::from("abort=operator_requested")],
    };
    let repair = RuntimeRunRepair::skipped("abort");
    let record = RuntimeRunRecord {
        plan,
        step_journal: vec![RuntimeRunStepRecord {
            id: format!("{}/step-abort", pending.run_id),
            label: String::from("abort pending run"),
            phase: run_phase_name(RuntimeRunPhase::Aborted).to_owned(),
            status: run_phase_name(RuntimeRunPhase::Aborted).to_owned(),
            evidence: String::from(RUN_REASON_OPERATOR_ABORT),
            failure: None,
        }],
        verification,
        repair,
        checkpoint: None,
        rollback: None,
        elapsed_ms: lifecycle::elapsed_ms(started_at),
        phase: RuntimeRunPhase::Aborted,
        outcome: RuntimeRunOutcome::Aborted,
        reason: String::from(RUN_REASON_OPERATOR_ABORT),
    };
    let abort_applied = accepted_goal_applied(&pending.intent_id);
    let patch_artifacts = runtime.compose_state.write_report(
        &template,
        &report_write_input(&abort_applied),
        &execution,
        &record,
    )?;
    runtime.trace_store.append_intent_event(TraceEventInput {
        actor_id: &runtime.actor_id,
        intent_id: &pending.intent_id,
        kind: "goal",
        outcome: DecisionOutcome::Accepted,
        policy_code: PolicyCode::Allowed.as_str(),
        effect_count: patch_artifacts.len(),
        state: &runtime.state,
        execution: &execution,
        report_written: true,
        report_error: None,
        patch_artifacts: &patch_artifacts,
        run: &record,
    })?;
    runtime.pending_run = None;
    runtime.persist_snapshot()?;
    println!(
        "abort run_id={} phase={} outcome={} reason={}",
        pending.run_id,
        run_phase_name(record.phase),
        run_outcome_name(record.outcome),
        record.reason
    );
    Ok(())
}

fn pending_run_for_target<'a>(
    runtime: &'a CliRuntime,
    target: &str,
) -> Result<&'a PendingRunSnapshot, String> {
    let Some(pending) = runtime.pending_run.as_ref() else {
        return Err(String::from("no pending run is available"));
    };
    if target == "latest" || target == pending.run_id {
        return Ok(pending);
    }
    Err(format!("pending run not found: {target}"))
}

fn pending_resume_for_target<'a>(
    runtime: &'a CliRuntime,
    target: &str,
) -> Result<&'a PendingRunSnapshot, String> {
    let pending = pending_run_for_target(runtime, target).map_err(|_| {
        format!("{RESUME_PENDING_APPROVAL_ONLY}; no pending approval run is available")
    })?;
    if pending.phase != run_phase_name(RuntimeRunPhase::WaitingApproval)
        || pending.approval_state != APPROVAL_STATE_REQUIRED
    {
        return Err(format!(
            "{RESUME_PENDING_APPROVAL_ONLY}; found phase={} approval_state={}",
            pending.phase, pending.approval_state
        ));
    }
    Ok(pending)
}

fn pending_abort_for_target<'a>(
    runtime: &'a CliRuntime,
    target: &str,
) -> Result<&'a PendingRunSnapshot, String> {
    let pending = pending_run_for_target(runtime, target).map_err(|_| {
        format!("{ABORT_PENDING_CONTROL_ONLY}; no pending control run is available")
    })?;
    if pending.phase != run_phase_name(RuntimeRunPhase::WaitingApproval)
        || pending.approval_state != APPROVAL_STATE_REQUIRED
    {
        return Err(format!(
            "{ABORT_PENDING_CONTROL_ONLY}; found phase={} approval_state={}",
            pending.phase, pending.approval_state
        ));
    }
    Ok(pending)
}

fn pending_run_snapshot(
    intent: &RunTemplate,
    applied: &AppliedIntent,
    record: &RuntimeRunRecord,
) -> Option<PendingRunSnapshot> {
    if !matches!(record.outcome, RuntimeRunOutcome::ApprovalRequired) {
        return None;
    }
    Some(PendingRunSnapshot {
        run_id: record.plan.run_id.clone(),
        intent_id: applied.intent_id.clone(),
        goal_file_path: intent.path.clone(),
        phase: run_phase_name(record.phase).to_owned(),
        reason: record.reason.clone(),
        approval_state: String::from(APPROVAL_STATE_REQUIRED),
        verifier_state: record.verification.status.to_owned(),
    })
}

fn load_pending_goal_template(goal_file_path: &str) -> Result<RunTemplate, String> {
    crate::goal_file::parse_goal_file_template(goal_file_path)
}

fn rewrite_report_for_rollback(
    compose_state: &crate::runtime_compose::RuntimeComposeState,
    template: &RunTemplate,
    input: &ReportWriteInput<'_>,
    execution: &RuntimeComposeExecution,
    record: &RuntimeRunRecord,
    patch_artifacts: Vec<crate::runtime_compose::RuntimeComposePatchArtifact>,
) -> Result<Vec<crate::runtime_compose::RuntimeComposePatchArtifact>, String> {
    if record.rollback.is_some() {
        return compose_state.write_report(template, input, execution, record);
    }
    Ok(patch_artifacts)
}

fn print_intent_result(applied: &AppliedIntent) {
    println!(
        "intent id={} kind={} outcome={} policy={} effects={}",
        applied.intent_id,
        applied.kind,
        outcome_name(applied.outcome),
        applied.policy_code.as_str(),
        applied.effect_count
    );
}

fn print_run_result(intent_id: &str, record: &RuntimeRunRecord) {
    println!(
        "run intent_id={} phase={} outcome={} reason={}",
        intent_id,
        run_phase_name(record.phase),
        run_outcome_name(record.outcome),
        record.reason
    );
}

fn print_status(runtime: &CliRuntime, target: Option<&str>) {
    let compose = runtime.compose_state.health();
    let latest_run = match target {
        Some("latest") | None => runtime
            .trace_store
            .latest_event()
            .ok()
            .flatten()
            .and_then(|event| event.run),
        Some(target) => runtime
            .trace_store
            .latest_event_for_run(target)
            .ok()
            .flatten()
            .and_then(|event| event.run),
    };
    let snapshot = StatusSnapshot::from(StatusInput {
        runtime: {
            let latest_artifact = latest_run.as_ref().and_then(|run| {
                runtime
                    .trace_store
                    .artifact_index_for_run(&run.run_id)
                    .ok()
                    .flatten()
            });
            RuntimeStatusInput {
                provider_id: compose.provider_id,
                provider_model: compose.provider_model,
                provider_state: compose.provider.state.to_string(),
                provider_detail: compose.provider.detail,
                memory_enabled: compose.memory.enabled,
                memory_state: compose.memory.state.to_string(),
                tool_enabled: compose.tool.enabled,
                tool_state: compose.tool.state.to_string(),
                latest_run,
                latest_artifact,
                pending_run: runtime.pending_run.clone(),
            }
        },
        state: StateStatusInput {
            revision: runtime.state.revision,
            mode: runtime.state.mode,
            last_intent_id: runtime.state.last_intent_id.clone(),
            last_decision: runtime
                .state
                .last_decision
                .map(|value| value.as_str().to_owned()),
            last_policy_code: runtime
                .state
                .last_policy_code
                .map(|value| value.as_str().to_owned()),
        },
    });

    for line in render_status_lines(&snapshot) {
        println!("{line}");
    }
}

fn print_health(runtime: &CliRuntime, config: &AppConfig) {
    let compose = runtime.compose_state.health();
    let ok = compose.provider.state == "ready"
        && compose.memory.state != "failed"
        && compose.tool.state != "failed";
    println!(
        "health ok={} profile={} mode={} revision={} provider_id={} provider_state={} memory_state={} tool_state={}",
        ok,
        config.profile,
        mode_name(runtime.state.mode),
        runtime.state.revision,
        compose.provider_id,
        compose.provider.state,
        compose.memory.state,
        compose.tool.state,
    );
    println!(
        "health detail provider_detail={} memory_detail={} tool_detail={}",
        compose.provider.detail, compose.memory.detail, compose.tool.detail,
    );
}

fn print_doctor(runtime: &CliRuntime, config: &AppConfig, json: bool) -> Result<(), String> {
    let compose = runtime.compose_state.health();
    let report = build_doctor_report(
        config,
        &runtime.state,
        &compose,
        &runtime.trace_store,
        runtime.state_store.path(),
        runtime.pending_run.as_ref(),
    );

    if json {
        let rendered = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("render doctor json failed: {error}"))?;
        println!("{rendered}");
        return Ok(());
    }

    for line in render_doctor_lines(&report) {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axiomrunner_core::{DoneCondition, RunApprovalMode, RunBudget, RunGoal, VerificationCheck};

    fn sample_goal_template(minutes: u64) -> RunTemplate {
        crate::cli_command::GoalFileTemplate {
            path: String::from("/tmp/goal.json"),
            goal: RunGoal {
                summary: String::from("goal"),
                workspace_root: String::from("/workspace"),
                constraints: Vec::new(),
                done_conditions: vec![DoneCondition {
                    label: String::from("report"),
                    evidence: String::from("report exists"),
                }],
                verification_checks: vec![VerificationCheck {
                    label: String::from("unit"),
                    detail: String::from("cargo test"),
                }],
                budget: RunBudget::bounded(5, minutes, 8000),
                approval_mode: RunApprovalMode::Never,
            },
            workflow_pack: None,
        }
    }

    #[test]
    fn elapsed_minute_budget_blocks_goal_runs() {
        let template = sample_goal_template(1);
        let record = RuntimeRunRecord {
            plan: crate::runtime_compose::RuntimeRunPlan {
                run_id: String::from("run-1"),
                goal: String::from("goal"),
                summary: String::from("summary"),
                done_when: String::from("done"),
                planned_steps: 4,
                steps: Vec::new(),
            },
            step_journal: Vec::new(),
            verification: RuntimeRunVerification {
                status: "passed",
                summary: String::from("goal_execution_verified"),
                checks: Vec::new(),
            },
            repair: RuntimeRunRepair::skipped("verification_passed"),
            checkpoint: None,
            rollback: None,
            elapsed_ms: 60_001,
            phase: RuntimeRunPhase::Completed,
            outcome: RuntimeRunOutcome::Success,
            reason: String::from("verification_passed"),
        };

        let record = lifecycle::apply_goal_elapsed_budget(&template, record);

        assert_eq!(record.phase, RuntimeRunPhase::Blocked);
        assert_eq!(record.outcome, RuntimeRunOutcome::BudgetExhausted);
        assert!(
            record
                .reason
                .starts_with("budget_exhausted_elapsed_minutes:")
        );
    }
}
