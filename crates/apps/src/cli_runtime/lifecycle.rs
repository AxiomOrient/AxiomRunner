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
    verification: &'a RuntimeRunVerification,
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
        let repair = runtime.compose_state.repair_template(
            intent,
            &applied.intent_id,
            applied.outcome,
            &execution,
        );
        let next_execution = execution.with_repair(&repair);
        let next_verification = verify_run(intent, &next_execution);

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
    intent
        .goal
        .budget
        .max_steps
        .saturating_sub(plan.planned_steps as u64) as usize
}

pub(super) fn verify_run(
    intent: &RunTemplate,
    execution: &RuntimeComposeExecution,
) -> RuntimeRunVerification {
    verify_goal_run(intent, execution)
}

fn verify_goal_run(
    goal_file: &crate::cli_command::GoalFileTemplate,
    execution: &RuntimeComposeExecution,
) -> RuntimeRunVerification {
    let mut checks: Vec<String> = [
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

    let verifier_evidence = parse_goal_verifier_evidence(&execution.tool_outputs);
    if verifier_evidence.is_empty() {
        return RuntimeRunVerification {
            status: "failed",
            summary: String::from("verification_evidence_unreadable"),
            checks,
        };
    }
    checks.extend(verifier_evidence.iter().map(|evidence| {
        format!(
            "verifier={} strength={} exit_code={} command={} artifact={} expectation={}",
            evidence.label,
            evidence.strength,
            evidence.exit_code,
            evidence.command,
            evidence.artifact_path,
            evidence.expectation
        )
    }));
    if let Some((status, summary)) = classify_goal_verifier_strength(&verifier_evidence) {
        return RuntimeRunVerification {
            status,
            summary,
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct GoalVerifierEvidence {
    label: String,
    strength: String,
    exit_code: i64,
    command: String,
    artifact_path: String,
    expectation: String,
}

fn parse_goal_verifier_evidence(tool_outputs: &[String]) -> Vec<GoalVerifierEvidence> {
    tool_outputs
        .iter()
        .filter_map(|output| serde_json::from_str(output).ok())
        .collect()
}

fn classify_goal_verifier_strength(
    evidence: &[GoalVerifierEvidence],
) -> Option<(&'static str, String)> {
    for (strength, status) in [
        ("pack_required", "pack_required"),
        ("unresolved", "verification_unresolved"),
        ("weak", "verification_weak"),
    ] {
        let mut matching = evidence
            .iter()
            .filter(|entry| entry.strength == strength)
            .peekable();
        if matching.peek().is_none() {
            continue;
        }
        let labels = matching.map(|entry| entry.label.as_str()).collect::<Vec<_>>();
        return Some((status, format!("{status}:{}", labels.join(","))));
    }
    None
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

    let (phase, outcome, reason) = finalize_goal_run(
        template,
        &plan,
        &applied,
        &execution,
        &verification,
        &compose,
        goal_approval_granted,
        requested_max_tokens,
    );

    let step_journal = build_step_journal(StepJournalInput {
        template,
        plan: &plan,
        verification: &verification,
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
    } else if matches!(
        verification.status,
        "verification_weak" | "verification_unresolved" | "pack_required"
    ) {
        (
            RuntimeRunPhase::Blocked,
            RuntimeRunOutcome::Blocked,
            verification.summary.clone(),
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

fn blocked_policy_outcome(applied: &AppliedIntent) -> (RuntimeRunPhase, RuntimeRunOutcome, String) {
    (
        RuntimeRunPhase::Blocked,
        RuntimeRunOutcome::Blocked,
        format!("policy={}", applied.policy_code.as_str()),
    )
}

fn goal_requires_pre_execution_approval(goal: &axiomrunner_core::RunGoal) -> bool {
    matches!(
        goal.approval_mode,
        axiomrunner_core::RunApprovalMode::Always | axiomrunner_core::RunApprovalMode::OnRisk
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
    let limit_ms = template.goal.budget.max_minutes.saturating_mul(60_000);
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
        verification,
        final_phase,
        final_reason,
    } = input;
    debug_assert!(
        plan.steps.len() >= 3,
        "goal plan must have ≥3 steps, got {}",
        plan.steps.len()
    );

    vec![
        RuntimeRunStepRecord {
            id: plan.steps[0].id.clone(),
            label: plan.steps[0].label.clone(),
            phase: plan.steps[0].phase.to_owned(),
            status: String::from("completed"),
            evidence: format!("goal_file={}", template.path),
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

fn execution_step_checks(execution: &RuntimeComposeExecution) -> [String; 3] {
    [
        format!("provider={}", step_name(&execution.provider)),
        format!("memory={}", step_name(&execution.memory)),
        format!("tool={}", step_name(&execution.tool)),
    ]
}
