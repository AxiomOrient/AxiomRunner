use crate::validation::ensure_not_blank;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunApprovalMode {
    Never,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentSurface {
    LegacyFact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentKind {
    ReadFact { key: String },
    WriteFact { key: String, value: String },
    RemoveFact { key: String },
    FreezeWrites,
    Halt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    pub intent_id: String,
    pub actor_id: Option<String>,
    pub kind: IntentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentPayloadError {
    IntentIdEmpty,
    ActorIdEmpty,
    KeyEmpty,
    ValueEmpty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentKindPayload {
    ReadFact { key: String },
    WriteFact { key: String, value: String },
    RemoveFact { key: String },
    FreezeWrites,
    Halt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentPayload {
    pub intent_id: String,
    pub actor_id: Option<String>,
    pub kind: IntentKindPayload,
}

impl Intent {
    pub fn read(
        intent_id: impl Into<String>,
        actor_id: Option<String>,
        key: impl Into<String>,
    ) -> Self {
        Self {
            intent_id: intent_id.into(),
            actor_id,
            kind: IntentKind::ReadFact { key: key.into() },
        }
    }

    pub fn write(
        intent_id: impl Into<String>,
        actor_id: Option<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            intent_id: intent_id.into(),
            actor_id,
            kind: IntentKind::WriteFact {
                key: key.into(),
                value: value.into(),
            },
        }
    }

    pub fn remove(
        intent_id: impl Into<String>,
        actor_id: Option<String>,
        key: impl Into<String>,
    ) -> Self {
        Self {
            intent_id: intent_id.into(),
            actor_id,
            kind: IntentKind::RemoveFact { key: key.into() },
        }
    }

    pub fn freeze_writes(intent_id: impl Into<String>, actor_id: Option<String>) -> Self {
        Self {
            intent_id: intent_id.into(),
            actor_id,
            kind: IntentKind::FreezeWrites,
        }
    }

    pub fn halt(intent_id: impl Into<String>, actor_id: Option<String>) -> Self {
        Self {
            intent_id: intent_id.into(),
            actor_id,
            kind: IntentKind::Halt,
        }
    }

    pub fn mutates_facts(&self) -> bool {
        matches!(
            self.kind,
            IntentKind::WriteFact { .. } | IntentKind::RemoveFact { .. }
        )
    }

    pub fn is_control_action(&self) -> bool {
        matches!(self.kind, IntentKind::FreezeWrites | IntentKind::Halt)
    }

    pub fn surface(&self) -> IntentSurface {
        IntentSurface::LegacyFact
    }

    pub fn to_payload(&self) -> IntentPayload {
        IntentPayload {
            intent_id: self.intent_id.clone(),
            actor_id: self.actor_id.clone(),
            kind: self.kind.to_payload(),
        }
    }

    pub fn try_from_payload(payload: IntentPayload) -> Result<Self, IntentPayloadError> {
        ensure_not_blank(
            payload.intent_id.as_str(),
            IntentPayloadError::IntentIdEmpty,
        )?;

        if let Some(actor_id) = payload.actor_id.as_deref() {
            ensure_not_blank(actor_id, IntentPayloadError::ActorIdEmpty)?;
        }

        Ok(Self {
            intent_id: payload.intent_id,
            actor_id: payload.actor_id,
            kind: IntentKind::try_from_payload(payload.kind)?,
        })
    }
}

impl IntentKind {
    fn to_payload(&self) -> IntentKindPayload {
        match self {
            IntentKind::ReadFact { key } => IntentKindPayload::ReadFact { key: key.clone() },
            IntentKind::WriteFact { key, value } => IntentKindPayload::WriteFact {
                key: key.clone(),
                value: value.clone(),
            },
            IntentKind::RemoveFact { key } => IntentKindPayload::RemoveFact { key: key.clone() },
            IntentKind::FreezeWrites => IntentKindPayload::FreezeWrites,
            IntentKind::Halt => IntentKindPayload::Halt,
        }
    }

    fn try_from_payload(payload: IntentKindPayload) -> Result<Self, IntentPayloadError> {
        match payload {
            IntentKindPayload::ReadFact { key } => {
                ensure_not_blank(key.as_str(), IntentPayloadError::KeyEmpty)?;
                Ok(IntentKind::ReadFact { key })
            }
            IntentKindPayload::WriteFact { key, value } => {
                ensure_not_blank(key.as_str(), IntentPayloadError::KeyEmpty)?;
                ensure_not_blank(value.as_str(), IntentPayloadError::ValueEmpty)?;
                Ok(IntentKind::WriteFact { key, value })
            }
            IntentKindPayload::RemoveFact { key } => {
                ensure_not_blank(key.as_str(), IntentPayloadError::KeyEmpty)?;
                Ok(IntentKind::RemoveFact { key })
            }
            IntentKindPayload::FreezeWrites => Ok(IntentKind::FreezeWrites),
            IntentKindPayload::Halt => Ok(IntentKind::Halt),
        }
    }
}
