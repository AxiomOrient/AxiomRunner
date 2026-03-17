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
use crate::storage::state::{PendingRunSnapshot, RuntimeStateSnapshot, StateStore};
use crate::status::{
    RuntimeStatusInput, StateStatusInput, StatusInput, StatusSnapshot, render_status_lines,
};
use crate::storage::trace::TraceStore;
use crate::workspace_lock::WorkspaceLock;
use axiomrunner_core::{AgentState, DecisionOutcome, PolicyCode, RunConstraint, RunConstraintMode};
use std::time::Instant;

pub(crate) mod lifecycle;
mod run_session;

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
            run_session::execute_intent(runtime, &intent)?
        }
        CliCommand::Replay { .. } => {
            return Err(String::from(
                "replay command should be handled before runtime execution",
            ));
        }
        CliCommand::Resume { target } => {
            runtime.ensure_workspace_lock("resume")?;
            run_session::execute_resume(runtime, &target)?
        }
        CliCommand::Abort { target } => {
            runtime.ensure_workspace_lock("abort")?;
            run_session::execute_abort(runtime, &target)?
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
                    evidence: axiomrunner_core::DoneConditionEvidence::ReportArtifactExists,
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
                workflow_pack: String::from("goal-default-v1"),
                verifier_flow: String::from("generic"),
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
            reason_code: String::from("verification_passed"),
            reason_detail: String::from("none"),
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
