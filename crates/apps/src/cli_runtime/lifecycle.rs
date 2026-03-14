use super::*;
use crate::runtime_compose::step_name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FinalizedRun {
    pub(super) execution: RuntimeComposeExecution,
    pub(super) record: RuntimeRunRecord,
}

pub(super) struct FinalizeRunInput<'a> {
    pub(super) template: &'a RunTemplate,
    pub(super) plan: crate::runtime_compose::RuntimeRunPlan,
    pub(super) applied: AppliedIntent,
    pub(super) execution: RuntimeComposeExecution,
    pub(super) verification: RuntimeRunVerification,
    pub(super) repair: RuntimeRunRepair,
    pub(super) goal_approval_granted: bool,
    pub(super) elapsed_ms: u64,
    pub(super) requested_max_tokens: usize,
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

pub(super) fn run_repair_loop(
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
                checks: initial_verification.checks,
            },
            RuntimeRunRepair {
                attempted: false,
                attempts: 0,
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

    for attempt in 1..=repair_budget {
        let repair = runtime
            .compose_state
            .repair_template(intent, &applied.intent_id, applied.outcome, &execution);
        let next_execution = execution.with_repair(&repair);
        let next_verification = verify_run(intent, applied, &next_execution, &runtime.state);

        if next_verification.status == "passed" {
            let summary = if attempt == 1 {
                format!("repair_completed:attempts={attempt}/{repair_budget}")
            } else {
                format!("repair_not_needed_after_retry:attempts={attempt}/{repair_budget}")
            };
            return (
                next_execution,
                next_verification,
                RuntimeRunRepair {
                    attempted: true,
                    attempts: attempt,
                    status: "repaired",
                    summary,
                    ..repair
                },
            );
        }

        execution = next_execution;
        verification = next_verification;

        let exhausted = attempt == repair_budget;
        let summary = if exhausted {
            format!("repair_budget_exhausted:attempts={attempt}/{repair_budget}")
        } else {
            format!(
                "repair_retry_failed:attempts={attempt}/{repair_budget}:{}",
                verification.summary
            )
        };

        if exhausted {
            return (
                execution,
                RuntimeRunVerification {
                    status: "failed",
                    summary: summary.clone(),
                    checks: verification.checks.clone(),
                },
                RuntimeRunRepair {
                    attempted: true,
                    attempts: attempt,
                    status: "budget_exhausted",
                    summary,
                    tool: repair.tool,
                    tool_outputs: repair.tool_outputs,
                    patch_artifacts: repair.patch_artifacts,
                },
            );
        }
    }

    (
        execution,
        verification,
        RuntimeRunRepair::skipped("repair_loop_unreachable"),
    )
}

fn repair_budget(intent: &RunTemplate, plan: &crate::runtime_compose::RuntimeRunPlan) -> usize {
    match intent {
        RunTemplate::GoalFile(goal_file) => goal_file
            .goal
            .budget
            .max_steps
            .saturating_sub(plan.planned_steps as u64) as usize,
        RunTemplate::LegacyIntent(_) => 1,
    }
}

pub(super) fn verify_run(
    intent: &RunTemplate,
    applied: &AppliedIntent,
    execution: &RuntimeComposeExecution,
    state: &AgentState,
) -> RuntimeRunVerification {
    if let Some(goal_file) = intent.goal_file() {
        return verify_goal_run(goal_file, execution);
    }
    verify_legacy_run(intent, applied, execution, state)
}

fn verify_goal_run(
    goal_file: &crate::cli_command::GoalFileTemplate,
    execution: &RuntimeComposeExecution,
) -> RuntimeRunVerification {
    let checks: Vec<String> = [
        format!("goal_file={}", goal_file.path),
        format!("workspace_root={}", goal_file.goal.workspace_root),
        format!("done_conditions={}", goal_file.goal.done_conditions.len()),
        format!(
            "verification_checks={}",
            goal_file.goal.verification_checks.len()
        ),
    ]
    .into_iter()
    .chain(execution_step_checks(execution))
    .collect();

    if let Some((stage, message)) = execution.first_failure() {
        return RuntimeRunVerification {
            status: "failed",
            summary: format!("stage={stage},message={message}"),
            checks,
        };
    }

    if execution.tool_outputs.is_empty() {
        return RuntimeRunVerification {
            status: "failed",
            summary: String::from("goal_execution_missing_verifier_output"),
            checks,
        };
    }

    match goal_file.goal.validate() {
        Ok(()) => RuntimeRunVerification {
            status: "passed",
            summary: String::from("goal_execution_verified"),
            checks,
        },
        Err(error) => RuntimeRunVerification {
            status: "failed",
            summary: format!("goal_contract_invalid:{error:?}"),
            checks,
        },
    }
}

fn verify_legacy_run(
    intent: &RunTemplate,
    applied: &AppliedIntent,
    execution: &RuntimeComposeExecution,
    state: &AgentState,
) -> RuntimeRunVerification {
    if applied.outcome == DecisionOutcome::Rejected {
        let checks = vec![format!(
            "policy_rejection={}",
            applied.policy_code.as_str()
        )];
        return RuntimeRunVerification {
            status: "passed",
            summary: format!("blocked_by_policy={}", applied.policy_code.as_str()),
            checks,
        };
    }

    let mut checks: Vec<String> = execution_step_checks(execution).into();

    if let Some((stage, message)) = execution.first_failure() {
        return RuntimeRunVerification {
            status: "failed",
            summary: format!("stage={stage},message={message}"),
            checks,
        };
    }

    if matches!(
        intent,
        RunTemplate::LegacyIntent(LegacyIntentTemplate::Write { .. })
            | RunTemplate::LegacyIntent(LegacyIntentTemplate::Remove { .. })
    ) && let Some(failure) = verify_mutation_contract(intent, execution, state, &mut checks)
    {
        return RuntimeRunVerification {
            status: "failed",
            summary: failure,
            checks,
        };
    }

    if let Some(failure) = verify_control_contract(intent, state, &mut checks) {
        return RuntimeRunVerification {
            status: "failed",
            summary: failure,
            checks,
        };
    }

    RuntimeRunVerification {
        status: "passed",
        summary: String::from("all_checks_passed"),
        checks,
    }
}

pub(super) fn apply_goal_done_conditions(
    goal_file: &crate::cli_command::GoalFileTemplate,
    execution: &RuntimeComposeExecution,
    report_patch_artifacts: &[crate::runtime_compose::RuntimeComposePatchArtifact],
    record: RuntimeRunRecord,
) -> (RuntimeRunRecord, bool) {
    if !matches!(record.outcome, RuntimeRunOutcome::Success) {
        return (record, false);
    }

    let mut checks = record.verification.checks.clone();
    let failure =
        goal_done_condition_failure(goal_file, execution, report_patch_artifacts, &mut checks);

    match failure {
        Some(summary) => (
            RuntimeRunRecord {
                verification: RuntimeRunVerification {
                    status: "failed",
                    summary: summary.clone(),
                    checks,
                },
                phase: RuntimeRunPhase::Failed,
                outcome: RuntimeRunOutcome::Failed,
                reason: summary,
                ..record
            },
            true,
        ),
        None => (
            RuntimeRunRecord {
                verification: RuntimeRunVerification {
                    status: record.verification.status,
                    summary: String::from("goal_done_conditions_verified"),
                    checks,
                },
                ..record
            },
            true,
        ),
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

pub(super) fn finalize_run(
    compose: crate::runtime_compose::RuntimeComposeHealth,
    input: FinalizeRunInput<'_>,
) -> FinalizedRun {
    let FinalizeRunInput {
        template,
        plan,
        applied,
        execution,
        verification,
        repair,
        goal_approval_granted,
        elapsed_ms,
        requested_max_tokens,
    } = input;

    let (phase, outcome, reason) = if let Some(goal_file) = template.goal_file() {
        finalize_goal_run(
            goal_file,
            &plan,
            &applied,
            &execution,
            &verification,
            &compose,
            goal_approval_granted,
            requested_max_tokens,
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
            checkpoint: None,
            rollback: None,
            elapsed_ms,
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
    requested_max_tokens: usize,
) -> (RuntimeRunPhase, RuntimeRunOutcome, String) {
    if let Some(reason) = goal_budget_guard_reason(goal_file, plan_ref, requested_max_tokens) {
        (
            RuntimeRunPhase::Blocked,
            RuntimeRunOutcome::BudgetExhausted,
            reason,
        )
    } else if goal_requires_pre_execution_approval(&goal_file.goal) && !approval_granted {
        (
            RuntimeRunPhase::WaitingApproval,
            RuntimeRunOutcome::ApprovalRequired,
            String::from("approval_required_before_execution"),
        )
    } else if verification.status == "passed" {
        (
            RuntimeRunPhase::Completed,
            RuntimeRunOutcome::Success,
            String::from("verification_passed"),
        )
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

fn goal_budget_guard_reason(
    goal_file: &crate::cli_command::GoalFileTemplate,
    plan_ref: &crate::runtime_compose::RuntimeRunPlan,
    requested_max_tokens: usize,
) -> Option<String> {
    if goal_file.goal.budget.max_steps < plan_ref.planned_steps as u64 {
        return Some(String::from("budget_exhausted_before_execution"));
    }
    if goal_file.goal.budget.max_tokens < requested_max_tokens as u64 {
        return Some(format!(
            "budget_exhausted_before_execution_tokens:{}>{}",
            requested_max_tokens, goal_file.goal.budget.max_tokens
        ));
    }
    None
}

pub(super) fn goal_pre_execution_guard(
    goal_file: &crate::cli_command::GoalFileTemplate,
    plan_ref: &crate::runtime_compose::RuntimeRunPlan,
    requested_max_tokens: usize,
) -> Option<String> {
    if let Some(reason) = goal_budget_guard_reason(goal_file, plan_ref, requested_max_tokens) {
        return Some(reason);
    }
    if goal_requires_pre_execution_approval(&goal_file.goal) {
        return Some(String::from("approval_required_before_execution"));
    }
    None
}

pub(super) fn apply_goal_elapsed_budget(
    template: &RunTemplate,
    record: RuntimeRunRecord,
) -> RuntimeRunRecord {
    let Some(goal_file) = template.goal_file() else {
        return record;
    };
    let limit_ms = goal_file.goal.budget.max_minutes.saturating_mul(60_000);
    if record.elapsed_ms > limit_ms
        && !matches!(
            record.outcome,
            RuntimeRunOutcome::ApprovalRequired | RuntimeRunOutcome::Aborted
        )
    {
        RuntimeRunRecord {
            phase: RuntimeRunPhase::Blocked,
            outcome: RuntimeRunOutcome::BudgetExhausted,
            reason: format!(
                "budget_exhausted_elapsed_minutes:{}>{}",
                record.elapsed_ms, limit_ms
            ),
            ..record
        }
    } else {
        record
    }
}

pub(super) fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
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

fn execution_step_checks(execution: &RuntimeComposeExecution) -> [String; 3] {
    [
        format!("provider={}", step_name(&execution.provider)),
        format!("memory={}", step_name(&execution.memory)),
        format!("tool={}", step_name(&execution.tool)),
    ]
}
