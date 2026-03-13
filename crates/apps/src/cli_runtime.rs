use crate::cli_command::{CliCommand, LegacyIntentTemplate, RunTemplate, USAGE};
use crate::config_loader::AppConfig;
use crate::display::mode_name;
use crate::doctor::{build_doctor_report, render_doctor_lines};
use crate::runtime_compose::{
    RuntimeComposeConfig, RuntimeComposeExecution, RuntimeComposeState, RuntimeRunOutcome,
    RuntimeRunPhase, RuntimeRunRecord, RuntimeRunRepair, RuntimeRunVerification, run_outcome_name,
    run_phase_name,
};
use crate::state_store::{RuntimeStateSnapshot, StateStore};
use crate::status::{
    RuntimeStatusInput, StateStatusInput, StatusInput, StatusSnapshot, render_status_lines,
};
use crate::trace_store::TraceStore;
use axonrunner_core::{
    AgentState, DecisionOutcome, DomainEvent, Intent, IntentKind, PolicyCode, build_policy_audit,
    decide, evaluate_policy, project_from,
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

pub struct CliRuntime {
    state: AgentState,
    actor_id: String,
    next_intent_seq: u64,
    next_run_seq: u64,
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
        let trace_store = TraceStore::from_workspace_root(compose_config.tool_workspace.clone())?;
        Ok(Self {
            state: snapshot.state,
            actor_id,
            next_intent_seq: snapshot.next_intent_seq,
            next_run_seq: snapshot.next_run_seq,
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
        self.compose_state
            .clear()
            .map(|_| ())
            .map_err(|error| format!("runtime execution failed stage=clear error={error}"))?;
        self.persist_snapshot()
    }

    fn apply_template(&mut self, template: &RunTemplate) -> AppliedIntent {
        let intent = template.to_intent(self.next_intent_id(), Some(self.actor_id.clone()));
        self.apply_intent(intent)
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
        }
    }

    fn restore_snapshot(&mut self, snapshot: RuntimeStateSnapshot) {
        self.state = snapshot.state;
        self.next_intent_seq = snapshot.next_intent_seq;
        self.next_run_seq = snapshot.next_run_seq;
    }

    fn persist_snapshot(&self) -> Result<(), String> {
        self.state_store
            .save_snapshot(&self.runtime_snapshot())
            .map_err(|error| format!("runtime state persistence failed: {error}"))
    }
}

pub fn execute_command(
    runtime: &mut CliRuntime,
    config: &AppConfig,
    command: CliCommand,
) -> Result<(), String> {
    match command {
        CliCommand::Run(intent) => execute_intent(runtime, &intent)?,
        CliCommand::Batch {
            intents,
            reset_state,
        } => {
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
        CliCommand::Doctor { json } => print_doctor(runtime, config, json)?,
        CliCommand::Status => print_status(runtime),
        CliCommand::Health => print_health(runtime, config),
        CliCommand::Help => print_usage(),
    }

    Ok(())
}

fn print_usage() {
    println!("{USAGE}");
}

fn execute_intent(runtime: &mut CliRuntime, intent: &RunTemplate) -> Result<(), String> {
    let previous = runtime.runtime_snapshot();
    let run_id = runtime.next_run_id();
    let applied = runtime.apply_template(intent);
    let plan = runtime
        .compose_state
        .plan_template(intent, &run_id, &applied.intent_id, applied.outcome);
    let execution = runtime.persist_template_result(intent, &applied);
    let mut verification = verify_run(intent, &applied, &execution);
    let repair = if verification.status == "failed" {
        runtime
            .compose_state
            .repair_template(intent, &applied.intent_id, applied.outcome, &execution)
    } else {
        RuntimeRunRepair::skipped("verification_passed")
    };
    let execution = execution.with_repair(&repair);
    if repair.attempted {
        verification = verify_run(intent, &applied, &execution);
    }
    let mut finalized = finalize_run(runtime, plan, applied.clone(), execution, verification, repair);
    let report_result = runtime.compose_state.write_report(
        intent,
        &applied.intent_id,
        applied.outcome,
        applied.policy_code.as_str(),
        applied.effect_count,
        &finalized.execution,
        &finalized.record,
    );
    let report_error = report_result.as_ref().err().cloned();
    if let Some(error) = report_error.as_deref() {
        finalized.record.phase = RuntimeRunPhase::Failed;
        finalized.record.outcome = RuntimeRunOutcome::Failed;
        finalized.record.reason = format!("report_write_failed:{error}");
    }

    let mut patch_artifacts = finalized.execution.patch_artifacts.clone();
    if let Ok(report_patch_artifacts) = &report_result {
        patch_artifacts.extend(report_patch_artifacts.clone());
    }
    if let Err(error) = runtime.trace_store.append_intent_event(
        &runtime.actor_id,
        &applied.intent_id,
        applied.kind,
        applied.outcome,
        applied.policy_code.as_str(),
        applied.effect_count,
        &runtime.state,
        &finalized.execution,
        report_error.is_none(),
        report_error.as_deref(),
        &patch_artifacts,
        &finalized.record,
    ) {
        runtime.restore_snapshot(previous);
        return Err(format!("runtime trace error: {error}"));
    }
    if let Some(error) = report_error {
        runtime.restore_snapshot(previous);
        return Err(error);
    }
    if let Err(error) = runtime
        .compose_state
        .remember_run_summary(&finalized.record, &applied.intent_id)
    {
        runtime.restore_snapshot(previous);
        return Err(format!("runtime memory recall error: {error}"));
    }
    if let Some((stage, message)) = finalized.execution.first_failure() {
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
    runtime.persist_snapshot()?;

    match intent.legacy_intent() {
        LegacyIntentTemplate::Read { key } => {
            print_intent_result(&applied);
            print_read_value(&applied.intent_id, &runtime.state, key);
        }
        _ => print_intent_result(&applied),
    }
    print_run_result(&applied.intent_id, &finalized.record);
    Ok(())
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

fn print_status(runtime: &CliRuntime) {
    let compose = runtime.compose_state.health();
    let latest_run = runtime
        .trace_store
        .latest_event()
        .ok()
        .flatten()
        .and_then(|event| event.run);
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

fn outcome_name(outcome: DecisionOutcome) -> &'static str {
    match outcome {
        DecisionOutcome::Accepted => "accepted",
        DecisionOutcome::Rejected => "rejected",
    }
}

fn verify_run(
    intent: &RunTemplate,
    applied: &AppliedIntent,
    execution: &RuntimeComposeExecution,
) -> RuntimeRunVerification {
    let mut checks = Vec::new();

    if applied.outcome == DecisionOutcome::Rejected {
        checks.push(format!("policy_rejection={}", applied.policy_code.as_str()));
        return RuntimeRunVerification {
            status: "passed",
            summary: format!("blocked_by_policy={}", applied.policy_code.as_str()),
            checks,
        };
    }

    checks.push(format!("provider={}", step_name(&execution.provider)));
    checks.push(format!("memory={}", step_name(&execution.memory)));
    checks.push(format!("tool={}", step_name(&execution.tool)));

    if let Some((stage, message)) = execution.first_failure() {
        return RuntimeRunVerification {
            status: "failed",
            summary: format!("stage={stage},message={message}"),
            checks,
        };
    }

    if matches!(
        intent.legacy_intent(),
        LegacyIntentTemplate::Write { .. } | LegacyIntentTemplate::Remove { .. }
    ) {
        if execution.patch_artifacts.is_empty() {
            return RuntimeRunVerification {
                status: "failed",
                summary: String::from("mutable_run_missing_patch_evidence"),
                checks,
            };
        }
        if execution.tool_outputs.is_empty() {
            return RuntimeRunVerification {
                status: "failed",
                summary: String::from("mutable_run_missing_tool_output"),
                checks,
            };
        }
        checks.push(format!("changed_paths={}", execution.patch_artifacts.len()));
    }

    RuntimeRunVerification {
        status: "passed",
        summary: String::from("all_checks_passed"),
        checks,
    }
}

fn finalize_run(
    runtime: &CliRuntime,
    plan: crate::runtime_compose::RuntimeRunPlan,
    applied: AppliedIntent,
    execution: RuntimeComposeExecution,
    verification: RuntimeRunVerification,
    repair: RuntimeRunRepair,
) -> FinalizedRun {
    let compose = runtime.compose_state.health();

    let (phase, outcome, reason) = if applied.outcome == DecisionOutcome::Rejected {
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
    };

    FinalizedRun {
        execution,
        record: RuntimeRunRecord {
            plan,
            verification,
            repair,
            phase,
            outcome,
            reason,
        },
    }
}

fn step_name(step: &crate::runtime_compose::RuntimeComposeStep) -> &'static str {
    match step {
        crate::runtime_compose::RuntimeComposeStep::Skipped => "skipped",
        crate::runtime_compose::RuntimeComposeStep::Applied => "applied",
        crate::runtime_compose::RuntimeComposeStep::Failed(_) => "failed",
    }
}
