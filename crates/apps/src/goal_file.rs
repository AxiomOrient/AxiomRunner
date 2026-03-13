use axonrunner_core::{
    DoneCondition, RunApprovalMode, RunBudget, RunConstraint, RunGoal, VerificationCheck,
};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct GoalFileInput {
    summary: String,
    workspace_root: String,
    #[serde(default)]
    constraints: Vec<ConstraintInput>,
    done_conditions: Vec<DoneConditionInput>,
    verification_checks: Vec<VerificationCheckInput>,
    budget: BudgetInput,
    approval_mode: String,
}

#[derive(Debug, Deserialize)]
struct ConstraintInput {
    label: String,
    detail: String,
}

#[derive(Debug, Deserialize)]
struct DoneConditionInput {
    label: String,
    evidence: String,
}

#[derive(Debug, Deserialize)]
struct VerificationCheckInput {
    label: String,
    detail: String,
}

#[derive(Debug, Deserialize)]
struct BudgetInput {
    max_steps: u64,
    max_minutes: u64,
    max_tokens: u64,
}

pub fn parse_goal_file(path: &str) -> Result<RunGoal, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("read goal file '{path}' failed: {error}"))?;
    let input: GoalFileInput = serde_json::from_str(&raw)
        .map_err(|error| format!("parse goal file '{path}' failed: {error}"))?;

    let approval_mode = match input.approval_mode.trim().to_ascii_lowercase().as_str() {
        "never" => RunApprovalMode::Never,
        "on-risk" => RunApprovalMode::OnRisk,
        "always" => RunApprovalMode::Always,
        other => {
            return Err(format!(
                "invalid goal file '{path}': unsupported approval_mode '{other}'"
            ))
        }
    };

    let goal = RunGoal {
        summary: input.summary,
        workspace_root: input.workspace_root,
        constraints: input
            .constraints
            .into_iter()
            .map(|constraint| RunConstraint {
                label: constraint.label,
                detail: constraint.detail,
            })
            .collect(),
        done_conditions: input
            .done_conditions
            .into_iter()
            .map(|condition| DoneCondition {
                label: condition.label,
                evidence: condition.evidence,
            })
            .collect(),
        verification_checks: input
            .verification_checks
            .into_iter()
            .map(|check| VerificationCheck {
                label: check.label,
                detail: check.detail,
            })
            .collect(),
        budget: RunBudget::bounded(
            input.budget.max_steps,
            input.budget.max_minutes,
            input.budget.max_tokens,
        ),
        approval_mode,
    };

    goal.validate()
        .map_err(|error| format!("invalid goal file '{path}': {error:?}"))?;

    Ok(goal)
}
