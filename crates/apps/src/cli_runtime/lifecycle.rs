use super::*;
use axiomrunner_core::DoneConditionEvidence;
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
    plan: &'a crate::runtime_compose::RuntimeRunPlan,
    verification: &'a RuntimeRunVerification,
    final_phase: RuntimeRunPhase,
    final_reason: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreExecutionGuard {
    pub(super) summary: String,
    pub(super) policy_code: Option<axiomrunner_core::PolicyCode>,
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
        let labels = matching
            .map(|entry| entry.label.as_str())
            .collect::<Vec<_>>();
        return Some((status, format!("{status}:{}", labels.join(","))));
    }
    None
}

pub(crate) fn apply_goal_done_conditions(
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
        Some((summary, reason_code, reason_detail)) => (
            RuntimeRunRecord {
                verification: RuntimeRunVerification {
                    status: "failed",
                    summary: summary.clone(),
                    checks,
                },
                phase: RuntimeRunPhase::Failed,
                outcome: RuntimeRunOutcome::Failed,
                reason: summary,
                reason_code,
                reason_detail,
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
) -> Option<(String, String, String)> {
    if execution.tool_outputs.is_empty() {
        return Some(crate::runtime_compose::runtime_run_reason(
            "goal_execution_missing_verifier_output",
            "none",
        ));
    }

    for condition in &goal_file.goal.done_conditions {
        match &condition.evidence {
            DoneConditionEvidence::ReportArtifactExists => {
                let ok = report_patch_artifacts
                    .iter()
                    .any(|artifact| artifact.target_path.ends_with(".report.md"));
                checks.push(format!(
                    "done_condition={} report_artifact={}",
                    condition.label,
                    if ok { "present" } else { "missing" }
                ));
                if !ok {
                    return Some(crate::runtime_compose::runtime_run_reason(
                        "done_condition_missing_report_artifact",
                        condition.label.clone(),
                    ));
                }
            }
            DoneConditionEvidence::FileExists { path } => {
                let full_path = std::path::Path::new(&goal_file.goal.workspace_root).join(path);
                let ok = full_path.is_file();
                checks.push(format!(
                    "done_condition={} file_exists={} status={}",
                    condition.label,
                    path,
                    if ok { "present" } else { "missing" }
                ));
                if !ok {
                    return Some(crate::runtime_compose::runtime_run_reason(
                        "done_condition_missing_file",
                        format!("{}:{}", condition.label, path),
                    ));
                }
            }
            DoneConditionEvidence::PathChanged { path } => {
                let ok = execution
                    .patch_artifacts
                    .iter()
                    .chain(report_patch_artifacts.iter())
                    .any(|artifact| path_matches_changed_target(&artifact.target_path, path));
                checks.push(format!(
                    "done_condition={} path_changed={} status={}",
                    condition.label,
                    path,
                    if ok { "present" } else { "missing" }
                ));
                if !ok {
                    return Some(crate::runtime_compose::runtime_run_reason(
                        "done_condition_path_not_changed",
                        format!("{}:{}", condition.label, path),
                    ));
                }
            }
            DoneConditionEvidence::CommandExitZero { command } => {
                let ok = parse_goal_verifier_evidence(&execution.tool_outputs)
                    .iter()
                    .any(|evidence| evidence.command == *command && evidence.exit_code == 0);
                checks.push(format!(
                    "done_condition={} command_exit_zero={} status={}",
                    condition.label,
                    command,
                    if ok { "present" } else { "missing" }
                ));
                if !ok {
                    return Some(crate::runtime_compose::runtime_run_reason(
                        "done_condition_command_not_verified",
                        format!("{}:{}", condition.label, command),
                    ));
                }
            }
        }
    }

    None
}

fn path_matches_changed_target(target_path: &str, expected_path: &str) -> bool {
    let target = normalize_path_segments(target_path);
    let expected = normalize_path_segments(expected_path);
    !expected.is_empty()
        && target.len() >= expected.len()
        && target.iter().zip(expected.iter()).all(|(left, right)| left == right)
}

fn normalize_path_segments(path: &str) -> Vec<String> {
    path.replace('\\', "/")
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .map(str::to_owned)
        .collect()
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

    let (phase, outcome, reason, reason_code, reason_detail) = finalize_goal_run(
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
            reason_code,
            reason_detail,
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
) -> (
    RuntimeRunPhase,
    RuntimeRunOutcome,
    String,
    String,
    String,
) {
    if let Some(reason) = goal_budget_guard_reason(goal_file, plan_ref, requested_max_tokens) {
        let (rendered, code, detail) = if let Some(detail) =
            reason.strip_prefix("budget_exhausted_before_execution_tokens:")
        {
            (
                reason.clone(),
                String::from("budget_exhausted_before_execution_tokens"),
                detail.to_owned(),
            )
        } else {
            (
                String::from("budget_exhausted_before_execution"),
                String::from("budget_exhausted_before_execution"),
                String::from("none"),
            )
        };
        (RuntimeRunPhase::Blocked, RuntimeRunOutcome::BudgetExhausted, rendered, code, detail)
    } else if goal_requires_pre_execution_approval(goal_file) && !approval_granted {
        let detail = verification
            .summary
            .strip_prefix("approval_required_before_execution:")
            .unwrap_or("none")
            .to_owned();
        let rendered = verification.summary.clone();
        let code = String::from("approval_required_before_execution");
        (
            RuntimeRunPhase::WaitingApproval,
            RuntimeRunOutcome::ApprovalRequired,
            rendered,
            code,
            detail,
        )
    } else if verification.status == "passed" {
        let (rendered, code, detail) =
            crate::runtime_compose::runtime_run_reason("verification_passed", "none");
        (RuntimeRunPhase::Completed, RuntimeRunOutcome::Success, rendered, code, detail)
    } else if matches!(
        verification.status,
        "verification_weak" | "verification_unresolved" | "pack_required"
    ) {
        let rendered = verification.summary.clone();
        let code = String::from("verification_blocked");
        let detail = verification.summary.clone();
        (RuntimeRunPhase::Blocked, RuntimeRunOutcome::Blocked, rendered, code, detail)
    } else if verification.summary.starts_with("repair_budget_exhausted") {
        let rendered = verification.summary.clone();
        let code = String::from("repair_budget_exhausted");
        let detail = verification.summary.clone();
        (RuntimeRunPhase::Blocked, RuntimeRunOutcome::BudgetExhausted, rendered, code, detail)
    } else if applied.outcome == DecisionOutcome::Rejected {
        blocked_policy_outcome(applied)
    } else if matches!(execution.first_failure(), Some(("provider", _)))
        && compose.provider.state == "blocked"
    {
        let (rendered, code, detail) =
            crate::runtime_compose::runtime_run_reason("provider_health_blocked", "none");
        (RuntimeRunPhase::Blocked, RuntimeRunOutcome::Blocked, rendered, code, detail)
    } else {
        let rendered = verification.summary.clone();
        let code = String::from("verification_failed");
        let detail = verification.summary.clone();
        (RuntimeRunPhase::Failed, RuntimeRunOutcome::Failed, rendered, code, detail)
    }
}

fn blocked_policy_outcome(
    applied: &AppliedIntent,
) -> (RuntimeRunPhase, RuntimeRunOutcome, String, String, String) {
    let rendered = format!("policy={}", applied.policy_code.as_str());
    let code = String::from("blocked_by_policy");
    let detail = applied.policy_code.as_str().to_owned();
    (RuntimeRunPhase::Blocked, RuntimeRunOutcome::Blocked, rendered, code, detail)
}

fn goal_requires_pre_execution_approval(goal_file: &crate::cli_command::GoalFileTemplate) -> bool {
    matches!(
        goal_file.goal.approval_mode,
        axiomrunner_core::RunApprovalMode::Always
    ) || crate::runtime_compose::constraint_requires_pre_execution_approval(goal_file)
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
) -> Option<PreExecutionGuard> {
    if let Some(reason) = goal_budget_guard_reason(goal_file, plan_ref, requested_max_tokens) {
        return Some(PreExecutionGuard {
            summary: reason,
            policy_code: None,
        });
    }
    let escalation_required = goal_file.goal.constraints.iter().any(|constraint| {
        matches!(
            constraint.policy_key(),
            Some(axiomrunner_core::RunConstraintPolicyKey::ApprovalEscalation)
        ) && constraint.detail.trim().eq_ignore_ascii_case("required")
    });
    if escalation_required
        && crate::runtime_compose::constraint_requires_pre_execution_approval(goal_file)
    {
        return Some(PreExecutionGuard {
            summary: String::from(
                "approval_required_before_execution:constraint_approval_escalation",
            ),
            policy_code: Some(axiomrunner_core::PolicyCode::ConstraintApprovalEscalation),
        });
    }
    if matches!(
        goal_file.goal.approval_mode,
        axiomrunner_core::RunApprovalMode::Always
    ) {
        return Some(PreExecutionGuard {
            summary: String::from("approval_required_before_execution"),
            policy_code: None,
        });
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
            reason: crate::runtime_compose::render_run_reason(
                "budget_exhausted_elapsed_minutes",
                &format!("{}>{}", record.elapsed_ms, limit_ms),
            ),
            reason_code: String::from("budget_exhausted_elapsed_minutes"),
            reason_detail: format!("{}>{}", record.elapsed_ms, limit_ms),
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
        plan,
        verification,
        final_phase,
        final_reason,
    } = input;
    let last_index = plan.steps.len().saturating_sub(1);
    plan.steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            if index == last_index {
                return RuntimeRunStepRecord {
                    id: step.id.clone(),
                    label: step.label.clone(),
                    phase: String::from(run_phase_name(final_phase)),
                    status: goal_terminal_step_status(final_phase).to_owned(),
                    evidence: final_reason.to_owned(),
                    failure: matches!(final_phase, RuntimeRunPhase::Failed)
                        .then(|| final_reason.to_owned()),
                };
            }
            if step.phase == "verifying" {
                return RuntimeRunStepRecord {
                    id: step.id.clone(),
                    label: step.label.clone(),
                    phase: step.phase.to_owned(),
                    status: if verification.status == "passed" {
                        String::from("verified")
                    } else {
                        String::from("failed")
                    },
                    evidence: verification.summary.clone(),
                    failure: (verification.status != "passed")
                        .then(|| verification.summary.clone()),
                };
            }
            RuntimeRunStepRecord {
                id: step.id.clone(),
                label: step.label.clone(),
                phase: step.phase.to_owned(),
                status: String::from("completed"),
                evidence: step.done_when.clone(),
                failure: None,
            }
        })
        .collect()
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
