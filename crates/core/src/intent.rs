use crate::validation::ensure_not_blank;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunApprovalMode {
    Never,
    /// Human approval required when the goal is classified as risky.
    /// **Current behavior**: identical to `Always` — risk classification is not yet
    /// implemented. A future release will introduce mutation-count and tool-scope
    /// heuristics to distinguish `OnRisk` from `Always`.
    OnRisk,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunBudget {
    pub max_steps: u64,
    pub max_minutes: u64,
    pub max_tokens: u64,
}

impl RunBudget {
    pub fn bounded(max_steps: u64, max_minutes: u64, max_tokens: u64) -> Self {
        Self {
            max_steps,
            max_minutes,
            max_tokens,
        }
    }

    pub fn validate(&self) -> Result<(), RunGoalValidationError> {
        if self.max_steps == 0 {
            return Err(RunGoalValidationError::BudgetStepsZero);
        }
        if self.max_minutes == 0 {
            return Err(RunGoalValidationError::BudgetMinutesZero);
        }
        if self.max_tokens == 0 {
            return Err(RunGoalValidationError::BudgetTokensZero);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConstraint {
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoneCondition {
    pub label: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationCheck {
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunGoal {
    pub summary: String,
    pub workspace_root: String,
    /// Advisory constraints for this goal run (e.g., "do not modify production configs").
    /// Constraints are validated (non-empty labels/details) and recorded in the plan
    /// artifact, but are **not enforced at runtime**. A future release will evaluate
    /// constraints against the tool allowlist and operator policy.
    pub constraints: Vec<RunConstraint>,
    pub done_conditions: Vec<DoneCondition>,
    pub verification_checks: Vec<VerificationCheck>,
    pub budget: RunBudget,
    pub approval_mode: RunApprovalMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunGoalValidationError {
    SummaryEmpty,
    WorkspaceRootEmpty,
    ConstraintLabelEmpty { index: usize },
    ConstraintDetailEmpty { index: usize },
    DoneConditionsEmpty,
    DoneConditionLabelEmpty { index: usize },
    DoneConditionEvidenceEmpty { index: usize },
    VerificationChecksEmpty,
    VerificationCheckLabelEmpty { index: usize },
    VerificationCheckDetailEmpty { index: usize },
    BudgetStepsZero,
    BudgetMinutesZero,
    BudgetTokensZero,
}

impl RunGoal {
    pub fn validate(&self) -> Result<(), RunGoalValidationError> {
        ensure_not_blank(self.summary.as_str(), RunGoalValidationError::SummaryEmpty)?;
        ensure_not_blank(
            self.workspace_root.as_str(),
            RunGoalValidationError::WorkspaceRootEmpty,
        )?;

        for (index, constraint) in self.constraints.iter().enumerate() {
            ensure_not_blank(
                constraint.label.as_str(),
                RunGoalValidationError::ConstraintLabelEmpty { index },
            )?;
            ensure_not_blank(
                constraint.detail.as_str(),
                RunGoalValidationError::ConstraintDetailEmpty { index },
            )?;
        }

        if self.done_conditions.is_empty() {
            return Err(RunGoalValidationError::DoneConditionsEmpty);
        }
        for (index, done_condition) in self.done_conditions.iter().enumerate() {
            ensure_not_blank(
                done_condition.label.as_str(),
                RunGoalValidationError::DoneConditionLabelEmpty { index },
            )?;
            ensure_not_blank(
                done_condition.evidence.as_str(),
                RunGoalValidationError::DoneConditionEvidenceEmpty { index },
            )?;
        }

        if self.verification_checks.is_empty() {
            return Err(RunGoalValidationError::VerificationChecksEmpty);
        }
        for (index, verification_check) in self.verification_checks.iter().enumerate() {
            ensure_not_blank(
                verification_check.label.as_str(),
                RunGoalValidationError::VerificationCheckLabelEmpty { index },
            )?;
            ensure_not_blank(
                verification_check.detail.as_str(),
                RunGoalValidationError::VerificationCheckDetailEmpty { index },
            )?;
        }

        self.budget.validate()?;

        Ok(())
    }
}
