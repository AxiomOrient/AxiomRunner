use crate::cli_command::{CliCommand, LegacyIntentTemplate, RunTemplate, USAGE};
use crate::config_loader::AppConfig;
use crate::display::{mode_name, outcome_name};
use crate::doctor::{build_doctor_report, render_doctor_lines};
use crate::runtime_compose::{
    ReportWriteInput, RuntimeComposeConfig, RuntimeComposeExecution, RuntimeComposeState,
    RuntimeRunOutcome, RuntimeRunPhase, RuntimeRunRecord, RuntimeRunRepair, RuntimeRunStepRecord,
    RuntimeRunVerification, run_outcome_name, run_phase_name,
};
use crate::state_store::{PendingRunSnapshot, RuntimeStateSnapshot, StateStore};
use crate::status::{
    RuntimeStatusInput, StateStatusInput, StatusInput, StatusSnapshot, render_status_lines,
};
use crate::trace_store::{TraceEventInput, TraceStore};
use crate::workspace_lock::WorkspaceLock;
use axonrunner_core::{
    AgentState, DecisionOutcome, DomainEvent, ExecutionMode, Intent, IntentKind, PolicyCode,
    build_policy_audit, decide, evaluate_policy, project_from,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedIntent {
    intent_id: String,
    kind: &'static str,
    outcome: DecisionOutcome,
    policy_code: PolicyCode,
    effect_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FinalizedRun {
    execution: RuntimeComposeExecution,
    record: RuntimeRunRecord,
}

struct FinalizeRunInput<'a> {
    template: &'a RunTemplate,
    plan: crate::runtime_compose::RuntimeRunPlan,
    applied: AppliedIntent,
    execution: RuntimeComposeExecution,
    verification: RuntimeRunVerification,
    repair: RuntimeRunRepair,
    goal_approval_granted: bool,
}

struct StepJournalInput<'a> {
    template: &'a RunTemplate,
    plan: &'a crate::runtime_compose::RuntimeRunPlan,
    applied: &'a AppliedIntent,
    execution: &'a RuntimeComposeExecution,
    verification: &'a RuntimeRunVerification,
    repair: &'a RuntimeRunRepair,
    final_phase: RuntimeRunPhase,
    final_reason: &'a str,
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

    fn reset(&mut self) -> Result<(), String> {
        self.state = AgentState::default();
        self.next_intent_seq = 0;
        self.next_run_seq = 0;
        self.pending_run = None;
        self.compose_state
            .clear()
            .map(|_| ())
            .map_err(|error| format!("runtime execution failed stage=clear error={error}"))?;
        self.persist_snapshot()
    }

    fn apply_template(&mut self, template: &RunTemplate) -> Result<AppliedIntent, String> {
        match template {
            RunTemplate::LegacyIntent(template) => {
                let intent = template.to_intent(self.next_intent_id(), Some(self.actor_id.clone()));
                Ok(self.apply_intent(intent))
            }
            RunTemplate::GoalFile(goal_file) => {
                goal_file
                    .goal
                    .validate()
                    .map_err(|error| format!("goal file validation failed: {error:?}"))?;
                let intent_id = self.next_intent_id();
                Ok(AppliedIntent {
                    intent_id,
                    kind: "goal",
                    outcome: DecisionOutcome::Accepted,
                    policy_code: PolicyCode::Allowed,
                    effect_count: 0,
                })
            }
        }
    }

    fn apply_intent(&mut self, intent: Intent) -> AppliedIntent {
        let verdict = evaluate_policy(&self.state, &intent);
        let decision = decide(&intent, &verdict);
        let audit = build_policy_audit(&self.state, &intent, &verdict);
        let effects = decision.effects.clone();

        let events = vec![
            DomainEvent::IntentAccepted {
                intent: intent.clone(),
            },
            DomainEvent::PolicyEvaluated { audit },
            DomainEvent::DecisionCalculated { decision },
            DomainEvent::EffectsApplied { effects },
        ];

        self.state = project_from(&self.state, &events);

        let decision = match &events[2] {
            DomainEvent::DecisionCalculated { decision } => decision,
            _ => unreachable!("event order is fixed"),
        };
        let audit = match &events[1] {
            DomainEvent::PolicyEvaluated { audit } => audit,
            _ => unreachable!("event order is fixed"),
        };
        let effects = match &events[3] {
            DomainEvent::EffectsApplied { effects } => effects,
            _ => unreachable!("event order is fixed"),
        };

        AppliedIntent {
            intent_id: intent.intent_id,
            kind: intent_kind_name(&intent.kind),
            outcome: decision.outcome,
            policy_code: audit.code,
            effect_count: effects.len(),
        }
    }

    fn persist_template_result(
        &mut self,
        template: &RunTemplate,
        applied: &AppliedIntent,
    ) -> crate::runtime_compose::RuntimeComposeExecution {
        self.compose_state
            .apply_template(template, &applied.intent_id, applied.outcome)
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
        CliCommand::Batch {
            intents,
            reset_state,
        } => {
            runtime.ensure_workspace_lock("batch")?;
            if reset_state {
                runtime.reset()?;
            }
            for intent in &intents {
                execute_intent(runtime, intent)?;
            }
            print_summary("batch", intents.len(), &runtime.state);
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
    let run_id = runtime.next_run_id();
    let applied = runtime.apply_template(intent)?;
    runtime.compose_state.prepare_execution_workspace(&run_id)?;
    let plan =
        runtime
            .compose_state
            .plan_template(intent, &run_id, &applied.intent_id, applied.outcome);
    let execution = runtime.persist_template_result(intent, &applied);
    let verification = verify_run(intent, &applied, &execution, &runtime.state);
    let (execution, verification, repair) =
        run_repair_loop(runtime, intent, &plan, &applied, execution, verification);
    let mut finalized = finalize_run(
        runtime,
        FinalizeRunInput {
            template: intent,
            plan: plan.clone(),
            applied: applied.clone(),
            execution,
            verification,
            repair,
            goal_approval_granted: false,
        },
    );
    let mut report_result = runtime.compose_state.write_report(
        intent,
        &report_write_input(&applied),
        &finalized.execution,
        &finalized.record,
    );
    if let (Some(goal_file), Ok(report_patch_artifacts)) = (intent.goal_file(), &report_result)
        && apply_goal_done_conditions(
            goal_file,
            &finalized.execution,
            report_patch_artifacts,
            &mut finalized.record,
        )
    {
        report_result = runtime.compose_state.write_report(
            intent,
            &report_write_input(&applied),
            &finalized.execution,
            &finalized.record,
        );
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
        effect_count: applied.effect_count,
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

    match intent {
        RunTemplate::LegacyIntent(LegacyIntentTemplate::Read { key }) => {
            print_intent_result(&applied);
            print_read_value(&applied.intent_id, &runtime.state, key);
        }
        _ => print_intent_result(&applied),
    }
    print_run_result(&applied.intent_id, &finalized.record);
    Ok(())
}

fn execute_resume(runtime: &mut CliRuntime, target: &str) -> Result<(), String> {
    let pending = pending_run_for_target(runtime, target)?.clone();
    runtime
        .compose_state
        .prepare_execution_workspace(&pending.run_id)?;
    let goal = crate::goal_file::parse_goal_file(&pending.goal_file_path)?;
    let template = RunTemplate::GoalFile(crate::cli_command::GoalFileTemplate {
        path: pending.goal_file_path.clone(),
        goal,
    });
    let plan = runtime.compose_state.plan_template(
        &template,
        &pending.run_id,
        &pending.intent_id,
        DecisionOutcome::Accepted,
    );
    let execution = runtime.compose_state.apply_template(
        &template,
        &pending.intent_id,
        DecisionOutcome::Accepted,
    );
    let resume_applied = accepted_goal_applied(&pending.intent_id);
    let verification = verify_run(&template, &resume_applied, &execution, &runtime.state);
    let (execution, verification, repair) = run_repair_loop(
        runtime,
        &template,
        &plan,
        &resume_applied,
        execution,
        verification,
    );
    let mut finalized = finalize_run(
        runtime,
        FinalizeRunInput {
            template: &template,
            plan: plan.clone(),
            applied: resume_applied.clone(),
            execution,
            verification,
            repair,
            goal_approval_granted: true,
        },
    );
    let mut patch_artifacts = runtime.compose_state.write_report(
        &template,
        &report_write_input(&resume_applied),
        &finalized.execution,
        &finalized.record,
    )?;
    if apply_goal_done_conditions(
        template.goal_file().expect("resume template is goal"),
        &finalized.execution,
        &patch_artifacts,
        &mut finalized.record,
    ) {
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
    runtime.trace_store.append_intent_event(TraceEventInput {
        actor_id: &runtime.actor_id,
        intent_id: &pending.intent_id,
        kind: "goal",
        outcome: DecisionOutcome::Accepted,
        policy_code: PolicyCode::Allowed.as_str(),
        effect_count: 0,
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
    let pending = pending_run_for_target(runtime, target)?.clone();
    let goal = crate::goal_file::parse_goal_file(&pending.goal_file_path)?;
    let template = RunTemplate::GoalFile(crate::cli_command::GoalFileTemplate {
        path: pending.goal_file_path.clone(),
        goal,
    });
    let plan = runtime.compose_state.plan_template(
        &template,
        &pending.run_id,
        &pending.intent_id,
        DecisionOutcome::Accepted,
    );
    let execution = runtime.compose_state.apply_template(
        &template,
        &pending.intent_id,
        DecisionOutcome::Accepted,
    );
    let verification = RuntimeRunVerification {
        status: "passed",
        summary: String::from("operator_abort"),
        checks: vec![String::from("abort=operator_requested")],
    };
    let repair = RuntimeRunRepair::skipped("abort");
    let record = RuntimeRunRecord {
        plan,
        step_journal: vec![RuntimeRunStepRecord {
            id: format!("{}/step-abort", pending.run_id),
            label: String::from("abort pending run"),
            phase: String::from("aborted"),
            status: String::from("aborted"),
            evidence: String::from("operator_abort"),
            failure: None,
        }],
        verification,
        repair,
        rollback: None,
        phase: RuntimeRunPhase::Aborted,
        outcome: RuntimeRunOutcome::Aborted,
        reason: String::from("operator_abort"),
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
        effect_count: 0,
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

fn run_repair_loop(
    runtime: &CliRuntime,
    intent: &RunTemplate,
    plan: &crate::runtime_compose::RuntimeRunPlan,
    applied: &AppliedIntent,
    initial_execution: RuntimeComposeExecution,
    initial_verification: RuntimeRunVerification,
) -> (
    RuntimeComposeExecution,
    RuntimeRunVerification,
    RuntimeRunRepair,
) {
    if initial_verification.status != "failed" {
        return (
            initial_execution,
            initial_verification,
            RuntimeRunRepair::skipped("verification_passed"),
        );
    }
    if !matches!(
        initial_execution.tool,
        crate::runtime_compose::RuntimeComposeStep::Failed(_)
    ) {
        return (
            initial_execution,
            initial_verification,
            RuntimeRunRepair::skipped("repair_not_applicable"),
        );
    }

    let repair_budget = repair_budget(intent, plan);
    if repair_budget == 0 {
        return (
            initial_execution,
            RuntimeRunVerification {
                status: "failed",
                summary: String::from("repair_budget_exhausted:attempts=0/0"),
                checks: initial_verification.checks.clone(),
            },
            RuntimeRunRepair {
                attempted: false,
                status: "budget_exhausted",
                summary: String::from("repair_budget_exhausted:attempts=0/0"),
                tool: crate::runtime_compose::RuntimeComposeStep::Skipped,
                tool_outputs: Vec::new(),
                patch_artifacts: Vec::new(),
            },
        );
    }

    let mut execution = initial_execution;
    let mut verification = initial_verification;
    let mut last_repair = RuntimeRunRepair::skipped("repair_not_attempted");

    for attempt in 1..=repair_budget {
        let repair = runtime.compose_state.repair_template(
            intent,
            &applied.intent_id,
            applied.outcome,
            &execution,
        );
        let next_execution = execution.with_repair(&repair);
        let next_verification = verify_run(intent, applied, &next_execution, &runtime.state);

        if next_verification.status == "passed" {
            let status = if repair.attempted {
                "repaired"
            } else {
                repair.status
            };
            let summary = if repair.attempted {
                format!("repair_completed:attempts={attempt}/{repair_budget}")
            } else {
                format!("repair_not_needed_after_retry:attempts={attempt}/{repair_budget}")
            };
            return (
                next_execution,
                next_verification,
                RuntimeRunRepair {
                    attempted: repair.attempted,
                    status,
                    summary,
                    tool: repair.tool,
                    tool_outputs: repair.tool_outputs,
                    patch_artifacts: repair.patch_artifacts,
                },
            );
        }

        execution = next_execution;
        verification = next_verification;
        last_repair = RuntimeRunRepair {
            attempted: repair.attempted,
            status: if attempt == repair_budget {
                "budget_exhausted"
            } else {
                repair.status
            },
            summary: if attempt == repair_budget {
                format!("repair_budget_exhausted:attempts={attempt}/{repair_budget}")
            } else {
                format!(
                    "repair_retry_failed:attempts={attempt}/{repair_budget}:{}",
                    repair.summary
                )
            },
            tool: repair.tool,
            tool_outputs: repair.tool_outputs,
            patch_artifacts: repair.patch_artifacts,
        };
    }

    let final_summary = last_repair.summary.clone();
    (
        execution,
        RuntimeRunVerification {
            status: "failed",
            summary: final_summary,
            checks: verification.checks,
        },
        last_repair,
    )
}

fn repair_budget(intent: &RunTemplate, plan: &crate::runtime_compose::RuntimeRunPlan) -> usize {
    match intent.goal_file() {
        Some(goal_file) => goal_file
            .goal
            .budget
            .max_steps
            .saturating_sub(plan.planned_steps as u64) as usize,
        None => 1,
    }
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

fn pending_run_snapshot(
    intent: &RunTemplate,
    applied: &AppliedIntent,
    record: &RuntimeRunRecord,
) -> Option<PendingRunSnapshot> {
    if !matches!(record.outcome, RuntimeRunOutcome::ApprovalRequired) {
        return None;
    }
    let goal_file = intent.goal_file()?;
    Some(PendingRunSnapshot {
        run_id: record.plan.run_id.clone(),
        intent_id: applied.intent_id.clone(),
        goal_file_path: goal_file.path.clone(),
        phase: run_phase_name(record.phase).to_owned(),
        reason: record.reason.clone(),
    })
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

fn print_read_value(intent_id: &str, state: &AgentState, key: &str) {
    let value = state.facts.get(key).map(String::as_str).unwrap_or("<none>");
    println!(
        "query intent_id={} key={} value={} revision={}",
        intent_id, key, value, state.revision
    );
}

fn print_summary(label: &str, intent_count: usize, state: &AgentState) {
    println!(
        "{} completed count={} revision={} mode={} facts={} denied={} audit={}",
        label,
        intent_count,
        state.revision,
        mode_name(state.mode),
        state.facts.len(),
        state.denied_count,
        state.audit_count
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
        state: StateStatusInput {
            revision: runtime.state.revision,
            mode: runtime.state.mode,
            facts: runtime.state.facts.len(),
            denied: runtime.state.denied_count,
            audit: runtime.state.audit_count,
        },
        runtime: RuntimeStatusInput {
            provider_id: compose.provider_id,
            provider_model: compose.provider_model,
            provider_state: compose.provider.state.to_string(),
            provider_detail: compose.provider.detail,
            memory_enabled: compose.memory.enabled,
            memory_state: compose.memory.state.to_string(),
            tool_enabled: compose.tool.enabled,
            tool_state: compose.tool.state.to_string(),
            latest_run,
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

fn intent_kind_name(kind: &IntentKind) -> &'static str {
    match kind {
        IntentKind::ReadFact { .. } => "read",
        IntentKind::WriteFact { .. } => "write",
        IntentKind::RemoveFact { .. } => "remove",
        IntentKind::FreezeWrites => "freeze",
        IntentKind::Halt => "halt",
    }
}

fn verify_run(
    intent: &RunTemplate,
    applied: &AppliedIntent,
    execution: &RuntimeComposeExecution,
    state: &AgentState,
) -> RuntimeRunVerification {
    let mut checks = Vec::new();

    if let Some(goal_file) = intent.goal_file() {
        return verify_goal_run(goal_file, execution, &mut checks);
    }

    verify_legacy_run(intent, applied, execution, state, &mut checks)
}

fn verify_goal_run(
    goal_file: &crate::cli_command::GoalFileTemplate,
    execution: &RuntimeComposeExecution,
    checks: &mut Vec<String>,
) -> RuntimeRunVerification {
    checks.push(format!("goal_file={}", goal_file.path));
    checks.push(format!("workspace_root={}", goal_file.goal.workspace_root));
    checks.push(format!(
        "done_conditions={}",
        goal_file.goal.done_conditions.len()
    ));
    checks.push(format!(
        "verification_checks={}",
        goal_file.goal.verification_checks.len()
    ));

    checks.push(format!("provider={}", step_name(&execution.provider)));
    checks.push(format!("memory={}", step_name(&execution.memory)));
    checks.push(format!("tool={}", step_name(&execution.tool)));

    if let Some((stage, message)) = execution.first_failure() {
        return RuntimeRunVerification {
            status: "failed",
            summary: format!("stage={stage},message={message}"),
            checks: checks.clone(),
        };
    }

    if execution.tool_outputs.is_empty() {
        return RuntimeRunVerification {
            status: "failed",
            summary: String::from("goal_execution_missing_verifier_output"),
            checks: checks.clone(),
        };
    }

    match goal_file.goal.validate() {
        Ok(()) => RuntimeRunVerification {
            status: "passed",
            summary: String::from("goal_execution_verified"),
            checks: checks.clone(),
        },
        Err(error) => RuntimeRunVerification {
            status: "failed",
            summary: format!("goal_contract_invalid:{error:?}"),
            checks: checks.clone(),
        },
    }
}

fn verify_legacy_run(
    intent: &RunTemplate,
    applied: &AppliedIntent,
    execution: &RuntimeComposeExecution,
    state: &AgentState,
    checks: &mut Vec<String>,
) -> RuntimeRunVerification {
    if applied.outcome == DecisionOutcome::Rejected {
        return blocked_policy_verification(applied, checks);
    }

    checks.push(format!("provider={}", step_name(&execution.provider)));
    checks.push(format!("memory={}", step_name(&execution.memory)));
    checks.push(format!("tool={}", step_name(&execution.tool)));

    if let Some((stage, message)) = execution.first_failure() {
        return RuntimeRunVerification {
            status: "failed",
            summary: format!("stage={stage},message={message}"),
            checks: checks.clone(),
        };
    }

    if matches!(
        intent,
        RunTemplate::LegacyIntent(LegacyIntentTemplate::Write { .. })
            | RunTemplate::LegacyIntent(LegacyIntentTemplate::Remove { .. })
    ) && let Some(failure) = verify_mutation_contract(intent, execution, state, checks)
    {
        return RuntimeRunVerification {
            status: "failed",
            summary: failure,
            checks: checks.clone(),
        };
    }

    if let Some(failure) = verify_control_contract(intent, state, checks) {
        return RuntimeRunVerification {
            status: "failed",
            summary: failure,
            checks: checks.clone(),
        };
    }

    RuntimeRunVerification {
        status: "passed",
        summary: String::from("all_checks_passed"),
        checks: checks.clone(),
    }
}

fn blocked_policy_verification(
    applied: &AppliedIntent,
    checks: &mut Vec<String>,
) -> RuntimeRunVerification {
    checks.push(format!("policy_rejection={}", applied.policy_code.as_str()));
    RuntimeRunVerification {
        status: "passed",
        summary: format!("blocked_by_policy={}", applied.policy_code.as_str()),
        checks: checks.clone(),
    }
}

fn apply_goal_done_conditions(
    goal_file: &crate::cli_command::GoalFileTemplate,
    execution: &RuntimeComposeExecution,
    report_patch_artifacts: &[crate::runtime_compose::RuntimeComposePatchArtifact],
    record: &mut RuntimeRunRecord,
) -> bool {
    if !matches!(record.outcome, RuntimeRunOutcome::Success) {
        return false;
    }

    let mut checks = std::mem::take(&mut record.verification.checks);
    let failure =
        goal_done_condition_failure(goal_file, execution, report_patch_artifacts, &mut checks);
    record.verification.checks = checks;

    match failure {
        Some(summary) => {
            record.verification.status = "failed";
            record.verification.summary = summary.clone();
            record.phase = RuntimeRunPhase::Failed;
            record.outcome = RuntimeRunOutcome::Failed;
            record.reason = summary;
            true
        }
        None => {
            record.verification.summary = String::from("goal_done_conditions_verified");
            true
        }
    }
}

fn goal_done_condition_failure(
    goal_file: &crate::cli_command::GoalFileTemplate,
    execution: &RuntimeComposeExecution,
    report_patch_artifacts: &[crate::runtime_compose::RuntimeComposePatchArtifact],
    checks: &mut Vec<String>,
) -> Option<String> {
    if execution.tool_outputs.is_empty() {
        return Some(String::from("goal_execution_missing_verifier_output"));
    }

    for condition in &goal_file.goal.done_conditions {
        match condition.evidence.as_str() {
            "report artifact exists" => {
                let ok = report_patch_artifacts
                    .iter()
                    .any(|artifact| artifact.target_path.ends_with(".report.md"));
                checks.push(format!(
                    "done_condition={} report_artifact={}",
                    condition.label,
                    if ok { "present" } else { "missing" }
                ));
                if !ok {
                    return Some(format!(
                        "done_condition_missing_report_artifact:{}",
                        condition.label
                    ));
                }
            }
            other => {
                return Some(format!(
                    "unsupported_done_condition_evidence:{}:{}",
                    condition.label, other
                ));
            }
        }
    }

    None
}

fn verify_mutation_contract(
    intent: &RunTemplate,
    execution: &RuntimeComposeExecution,
    state: &AgentState,
    checks: &mut Vec<String>,
) -> Option<String> {
    if execution.patch_artifacts.is_empty() {
        return Some(String::from("mutable_run_missing_changed_paths"));
    }
    if execution.tool_outputs.is_empty() {
        return Some(String::from("mutable_run_missing_tool_output"));
    }

    checks.push(format!("changed_paths={}", execution.patch_artifacts.len()));

    match intent {
        RunTemplate::LegacyIntent(LegacyIntentTemplate::Write { key, value }) => {
            checks.push(format!("state_fact={key}:{value}"));
            match state.facts.get(key) {
                Some(actual) if actual == value => None,
                Some(actual) => Some(format!("state_fact_mismatch:{key}:{actual}")),
                None => Some(format!("state_fact_missing:{key}")),
            }
        }
        RunTemplate::LegacyIntent(LegacyIntentTemplate::Remove { key }) => {
            checks.push(format!("state_fact_absent={key}"));
            state
                .facts
                .contains_key(key)
                .then(|| format!("state_fact_still_present:{key}"))
        }
        _ => None,
    }
}

fn verify_control_contract(
    intent: &RunTemplate,
    state: &AgentState,
    checks: &mut Vec<String>,
) -> Option<String> {
    match intent {
        RunTemplate::LegacyIntent(LegacyIntentTemplate::Freeze) => {
            checks.push(format!("mode={}", mode_name(state.mode)));
            (state.mode != ExecutionMode::ReadOnly)
                .then(|| String::from("mode_not_read_only_after_freeze"))
        }
        RunTemplate::LegacyIntent(LegacyIntentTemplate::Halt) => {
            checks.push(format!("mode={}", mode_name(state.mode)));
            (state.mode != ExecutionMode::Halted)
                .then(|| String::from("mode_not_halted_after_halt"))
        }
        _ => None,
    }
}

fn finalize_run(runtime: &CliRuntime, input: FinalizeRunInput<'_>) -> FinalizedRun {
    let FinalizeRunInput {
        template,
        plan,
        applied,
        execution,
        verification,
        repair,
        goal_approval_granted,
    } = input;
    let compose = runtime.compose_state.health();

    let (phase, outcome, reason) = if let Some(goal_file) = template.goal_file() {
        finalize_goal_run(
            goal_file,
            &plan,
            &applied,
            &execution,
            &verification,
            &compose,
            goal_approval_granted,
        )
    } else {
        finalize_legacy_run(&applied, &execution, &verification, &compose)
    };

    let step_journal = build_step_journal(StepJournalInput {
        template,
        plan: &plan,
        applied: &applied,
        execution: &execution,
        verification: &verification,
        repair: &repair,
        final_phase: phase,
        final_reason: &reason,
    });

    FinalizedRun {
        execution,
        record: RuntimeRunRecord {
            plan,
            step_journal,
            verification,
            repair,
            rollback: None,
            phase,
            outcome,
            reason,
        },
    }
}

fn finalize_goal_run(
    goal_file: &crate::cli_command::GoalFileTemplate,
    plan_ref: &crate::runtime_compose::RuntimeRunPlan,
    applied: &AppliedIntent,
    execution: &RuntimeComposeExecution,
    verification: &RuntimeRunVerification,
    compose: &crate::runtime_compose::RuntimeComposeHealth,
    approval_granted: bool,
) -> (RuntimeRunPhase, RuntimeRunOutcome, String) {
    if verification.status == "passed" {
        if goal_file.goal.budget.max_steps < plan_ref.planned_steps as u64 {
            (
                RuntimeRunPhase::Blocked,
                RuntimeRunOutcome::BudgetExhausted,
                String::from("budget_exhausted_before_execution"),
            )
        } else if goal_requires_pre_execution_approval(&goal_file.goal) && !approval_granted {
            (
                RuntimeRunPhase::WaitingApproval,
                RuntimeRunOutcome::ApprovalRequired,
                String::from("approval_required_before_execution"),
            )
        } else {
            (
                RuntimeRunPhase::Completed,
                RuntimeRunOutcome::Success,
                String::from("verification_passed"),
            )
        }
    } else if verification.summary.starts_with("repair_budget_exhausted") {
        (
            RuntimeRunPhase::Blocked,
            RuntimeRunOutcome::BudgetExhausted,
            verification.summary.clone(),
        )
    } else if applied.outcome == DecisionOutcome::Rejected {
        blocked_policy_outcome(applied)
    } else if matches!(execution.first_failure(), Some(("provider", _)))
        && compose.provider.state == "blocked"
    {
        (
            RuntimeRunPhase::Blocked,
            RuntimeRunOutcome::Blocked,
            String::from("provider_health_blocked"),
        )
    } else {
        (
            RuntimeRunPhase::Failed,
            RuntimeRunOutcome::Failed,
            verification.summary.clone(),
        )
    }
}

fn finalize_legacy_run(
    applied: &AppliedIntent,
    execution: &RuntimeComposeExecution,
    verification: &RuntimeRunVerification,
    compose: &crate::runtime_compose::RuntimeComposeHealth,
) -> (RuntimeRunPhase, RuntimeRunOutcome, String) {
    if applied.outcome == DecisionOutcome::Rejected {
        blocked_policy_outcome(applied)
    } else if verification.summary.starts_with("repair_budget_exhausted") {
        (
            RuntimeRunPhase::Blocked,
            RuntimeRunOutcome::BudgetExhausted,
            verification.summary.clone(),
        )
    } else if verification.status == "passed" {
        (
            RuntimeRunPhase::Completed,
            RuntimeRunOutcome::Success,
            String::from("verification_passed"),
        )
    } else if matches!(execution.first_failure(), Some(("provider", _)))
        && compose.provider.state == "blocked"
    {
        (
            RuntimeRunPhase::Blocked,
            RuntimeRunOutcome::Blocked,
            verification.summary.clone(),
        )
    } else {
        (
            RuntimeRunPhase::Failed,
            RuntimeRunOutcome::Failed,
            verification.summary.clone(),
        )
    }
}

fn blocked_policy_outcome(applied: &AppliedIntent) -> (RuntimeRunPhase, RuntimeRunOutcome, String) {
    if applied.policy_code == PolicyCode::UnauthorizedControl {
        (
            RuntimeRunPhase::WaitingApproval,
            RuntimeRunOutcome::ApprovalRequired,
            format!("policy={}", applied.policy_code.as_str()),
        )
    } else {
        (
            RuntimeRunPhase::Blocked,
            RuntimeRunOutcome::Blocked,
            format!("policy={}", applied.policy_code.as_str()),
        )
    }
}

fn goal_requires_pre_execution_approval(goal: &axonrunner_core::RunGoal) -> bool {
    matches!(
        goal.approval_mode,
        axonrunner_core::RunApprovalMode::Always | axonrunner_core::RunApprovalMode::OnRisk
    )
}

fn build_step_journal(input: StepJournalInput<'_>) -> Vec<RuntimeRunStepRecord> {
    let StepJournalInput {
        template,
        plan,
        applied,
        execution,
        verification,
        repair,
        final_phase,
        final_reason,
    } = input;
    let goal_file = match template {
        RunTemplate::GoalFile(goal_file) => goal_file,
        RunTemplate::LegacyIntent(_) => {
            return build_legacy_step_journal(StepJournalInput {
                template,
                plan,
                applied,
                execution,
                verification,
                repair,
                final_phase,
                final_reason,
            });
        }
    };

    vec![
        RuntimeRunStepRecord {
            id: plan.steps[0].id.clone(),
            label: plan.steps[0].label.clone(),
            phase: plan.steps[0].phase.to_owned(),
            status: String::from("completed"),
            evidence: format!("goal_file={}", goal_file.path),
            failure: None,
        },
        RuntimeRunStepRecord {
            id: plan.steps[1].id.clone(),
            label: plan.steps[1].label.clone(),
            phase: plan.steps[1].phase.to_owned(),
            status: if verification.status == "passed" {
                String::from("verified")
            } else {
                String::from("failed")
            },
            evidence: verification.summary.clone(),
            failure: (verification.status != "passed").then(|| verification.summary.clone()),
        },
        RuntimeRunStepRecord {
            id: plan.steps[2].id.clone(),
            label: plan.steps[2].label.clone(),
            phase: String::from(run_phase_name(final_phase)),
            status: goal_terminal_step_status(final_phase).to_owned(),
            evidence: final_reason.to_owned(),
            failure: matches!(final_phase, RuntimeRunPhase::Failed)
                .then(|| final_reason.to_owned()),
        },
    ]
}

fn goal_terminal_step_status(final_phase: RuntimeRunPhase) -> &'static str {
    match final_phase {
        RuntimeRunPhase::Completed => "completed",
        RuntimeRunPhase::WaitingApproval | RuntimeRunPhase::Blocked => "blocked",
        RuntimeRunPhase::Failed => "failed",
        RuntimeRunPhase::Aborted => "aborted",
        RuntimeRunPhase::Planning
        | RuntimeRunPhase::ExecutingStep
        | RuntimeRunPhase::Verifying
        | RuntimeRunPhase::Repairing => "completed",
    }
}

fn build_legacy_step_journal(input: StepJournalInput<'_>) -> Vec<RuntimeRunStepRecord> {
    let StepJournalInput {
        template,
        plan,
        applied,
        execution,
        verification,
        repair,
        final_phase,
        final_reason,
    } = input;
    let mut steps = Vec::new();

    steps.push(RuntimeRunStepRecord {
        id: plan.steps[0].id.clone(),
        label: plan.steps[0].label.clone(),
        phase: plan.steps[0].phase.to_owned(),
        status: String::from("completed"),
        evidence: format!("intent_id={}", applied.intent_id),
        failure: None,
    });

    match template {
        RunTemplate::LegacyIntent(LegacyIntentTemplate::Write { .. })
        | RunTemplate::LegacyIntent(LegacyIntentTemplate::Remove { .. }) => {
            steps.push(step_record_from_compose_step(
                &plan.steps[1].id,
                &plan.steps[1].label,
                plan.steps[1].phase,
                &execution.provider,
                execution
                    .provider_output
                    .clone()
                    .unwrap_or_else(|| String::from("provider_output=<none>")),
            ));
            steps.push(RuntimeRunStepRecord {
                id: plan.steps[2].id.clone(),
                label: plan.steps[2].label.clone(),
                phase: plan.steps[2].phase.to_owned(),
                status: mutation_status(execution, repair),
                evidence: mutation_evidence(execution, repair),
                failure: mutation_failure(execution, repair),
            });
            if repair.attempted {
                steps.push(RuntimeRunStepRecord {
                    id: format!("{}/step-repair-1", plan.run_id),
                    label: String::from("repair failed mutation"),
                    phase: String::from("repairing"),
                    status: repair.status.to_owned(),
                    evidence: repair.summary.clone(),
                    failure: matches!(
                        repair.tool,
                        crate::runtime_compose::RuntimeComposeStep::Failed(_)
                    )
                    .then(|| repair.summary.clone()),
                });
            }
        }
        RunTemplate::LegacyIntent(LegacyIntentTemplate::Read { .. }) => {}
        RunTemplate::LegacyIntent(LegacyIntentTemplate::Freeze)
        | RunTemplate::LegacyIntent(LegacyIntentTemplate::Halt) => {
            steps.push(RuntimeRunStepRecord {
                id: plan.steps[1].id.clone(),
                label: plan.steps[1].label.clone(),
                phase: String::from(run_phase_name(final_phase)),
                status: if applied.outcome == DecisionOutcome::Accepted {
                    String::from("completed")
                } else {
                    String::from("blocked")
                },
                evidence: final_reason.to_owned(),
                failure: None,
            });
        }
        RunTemplate::GoalFile(_) => {}
    }

    steps.push(RuntimeRunStepRecord {
        id: plan
            .steps
            .last()
            .map(|step| step.id.clone())
            .unwrap_or_else(|| format!("{}/step-final-verifying", plan.run_id)),
        label: plan
            .steps
            .last()
            .map(|step| step.label.clone())
            .unwrap_or_else(|| String::from("verify run")),
        phase: String::from("verifying"),
        status: if verification.status == "passed" {
            String::from("verified")
        } else {
            String::from("failed")
        },
        evidence: verification.summary.clone(),
        failure: (verification.status != "passed").then(|| verification.summary.clone()),
    });

    steps
}

fn step_record_from_compose_step(
    step_id: &str,
    label: &str,
    phase: &str,
    step: &crate::runtime_compose::RuntimeComposeStep,
    evidence: String,
) -> RuntimeRunStepRecord {
    let (status, failure) = match step {
        crate::runtime_compose::RuntimeComposeStep::Applied => (String::from("completed"), None),
        crate::runtime_compose::RuntimeComposeStep::Skipped => (String::from("skipped"), None),
        crate::runtime_compose::RuntimeComposeStep::Failed(message) => {
            (String::from("failed"), Some(message.clone()))
        }
    };

    RuntimeRunStepRecord {
        id: step_id.to_owned(),
        label: label.to_owned(),
        phase: phase.to_owned(),
        status,
        evidence,
        failure,
    }
}

fn mutation_status(execution: &RuntimeComposeExecution, repair: &RuntimeRunRepair) -> String {
    if repair.attempted && repair.status == "repaired" {
        return String::from("repaired");
    }
    match (&execution.memory, &execution.tool) {
        (crate::runtime_compose::RuntimeComposeStep::Failed(_), _)
        | (_, crate::runtime_compose::RuntimeComposeStep::Failed(_)) => String::from("failed"),
        (
            crate::runtime_compose::RuntimeComposeStep::Skipped,
            crate::runtime_compose::RuntimeComposeStep::Skipped,
        ) => String::from("skipped"),
        _ => String::from("completed"),
    }
}

fn mutation_evidence(execution: &RuntimeComposeExecution, repair: &RuntimeRunRepair) -> String {
    if repair.attempted {
        return repair.summary.clone();
    }
    format!(
        "memory={} tool={} changed_paths={}",
        step_name(&execution.memory),
        step_name(&execution.tool),
        execution.patch_artifacts.len()
    )
}

fn mutation_failure(
    execution: &RuntimeComposeExecution,
    repair: &RuntimeRunRepair,
) -> Option<String> {
    if repair.attempted && repair.status != "repaired" {
        return Some(repair.summary.clone());
    }
    match (&execution.memory, &execution.tool) {
        (crate::runtime_compose::RuntimeComposeStep::Failed(message), _) => Some(message.clone()),
        (_, crate::runtime_compose::RuntimeComposeStep::Failed(message)) => Some(message.clone()),
        _ => None,
    }
}

fn step_name(step: &crate::runtime_compose::RuntimeComposeStep) -> &'static str {
    match step {
        crate::runtime_compose::RuntimeComposeStep::Skipped => "skipped",
        crate::runtime_compose::RuntimeComposeStep::Applied => "applied",
        crate::runtime_compose::RuntimeComposeStep::Failed(_) => "failed",
    }
}
