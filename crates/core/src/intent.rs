use crate::validation::ensure_not_blank;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunApprovalMode {
    Never,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunConstraintPolicyKey {
    PathScope,
    DestructiveCommandClass,
    ExternalCommandClass,
    ApprovalEscalation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunConstraintMode {
    Advisory,
    EnforcedSubset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoneCondition {
    pub label: String,
    pub evidence: DoneConditionEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRelativePath {
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoneConditionEvidence {
    ReportArtifactExists,
    FileExists { path: WorkspaceRelativePath },
    PathChanged { path: WorkspaceRelativePath },
    CommandExitZero { command: String },
}

impl DoneConditionEvidence {
    pub fn parse(raw: &str) -> Result<Self, DoneConditionEvidenceParseError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(DoneConditionEvidenceParseError::Empty);
        }
        if trimmed == "report_artifact_exists" {
            return Ok(Self::ReportArtifactExists);
        }

        let (kind, detail) = trimmed.split_once(':').ok_or_else(|| {
            DoneConditionEvidenceParseError::Unsupported {
                raw: trimmed.to_owned(),
            }
        })?;
        let value = detail.trim();
        if value.is_empty() {
            return Err(DoneConditionEvidenceParseError::MissingValue {
                kind: kind.to_owned(),
            });
        }

        match kind {
            "file_exists" => Ok(Self::FileExists {
                path: WorkspaceRelativePath::parse(value)
                    .map_err(DoneConditionEvidenceParseError::InvalidWorkspacePath)?,
            }),
            "path_changed" => Ok(Self::PathChanged {
                path: WorkspaceRelativePath::parse(value)
                    .map_err(DoneConditionEvidenceParseError::InvalidWorkspacePath)?,
            }),
            "command_exit_zero" => Ok(Self::CommandExitZero {
                command: value.to_owned(),
            }),
            _ => Err(DoneConditionEvidenceParseError::Unsupported {
                raw: trimmed.to_owned(),
            }),
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Self::ReportArtifactExists => String::from("report_artifact_exists"),
            Self::FileExists { path } => format!("file_exists:{path}"),
            Self::PathChanged { path } => format!("path_changed:{path}"),
            Self::CommandExitZero { command } => format!("command_exit_zero:{command}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoneConditionEvidenceParseError {
    Empty,
    MissingValue { kind: String },
    Unsupported { raw: String },
    InvalidWorkspacePath(WorkspaceRelativePathError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceRelativePathError {
    Empty,
    Absolute,
    ParentTraversal,
    InvalidSegment,
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
    /// Goal-level constraints for this run. The current runtime distinguishes:
    /// - enforced subset labels: `path_scope`, `destructive_commands`,
    ///   `external_commands`, `approval_escalation`
    /// - all other labels: advisory-only
    ///
    /// CP-001 defines the subset and vocabulary. CP-002/CP-003 connect those labels
    /// to actual policy and approval behavior.
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
    ConstraintLabelEmpty {
        index: usize,
    },
    ConstraintDetailEmpty {
        index: usize,
    },
    DoneConditionsEmpty,
    DoneConditionLabelEmpty {
        index: usize,
    },
    DoneConditionEvidenceInvalid {
        index: usize,
        error: DoneConditionEvidenceParseError,
    },
    VerificationChecksEmpty,
    VerificationCheckLabelEmpty {
        index: usize,
    },
    VerificationCheckDetailEmpty {
        index: usize,
    },
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
            if let Err(error) = DoneConditionEvidence::parse(&done_condition.evidence.as_str()) {
                return Err(RunGoalValidationError::DoneConditionEvidenceInvalid { index, error });
            }
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

impl WorkspaceRelativePath {
    pub fn parse(raw: &str) -> Result<Self, WorkspaceRelativePathError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(WorkspaceRelativePathError::Empty);
        }
        if Path::new(trimmed).is_absolute() || looks_like_windows_absolute(trimmed) {
            return Err(WorkspaceRelativePathError::Absolute);
        }

        let normalized_input = trimmed.replace('\\', "/");
        let mut normalized = Vec::new();
        for segment in normalized_input.split('/') {
            if segment.is_empty() || segment == "." {
                continue;
            }
            if segment == ".." {
                return Err(WorkspaceRelativePathError::ParentTraversal);
            }
            if segment.contains('\0') {
                return Err(WorkspaceRelativePathError::InvalidSegment);
            }
            normalized.push(segment);
        }

        if normalized.is_empty() {
            return Err(WorkspaceRelativePathError::InvalidSegment);
        }

        Ok(Self {
            value: normalized.join("/"),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for WorkspaceRelativePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

fn looks_like_windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

impl RunConstraint {
    pub fn policy_key(&self) -> Option<RunConstraintPolicyKey> {
        match self.label.trim().to_ascii_lowercase().as_str() {
            "path_scope" => Some(RunConstraintPolicyKey::PathScope),
            "destructive_commands" => Some(RunConstraintPolicyKey::DestructiveCommandClass),
            "external_commands" => Some(RunConstraintPolicyKey::ExternalCommandClass),
            "approval_escalation" => Some(RunConstraintPolicyKey::ApprovalEscalation),
            _ => None,
        }
    }

    pub fn mode(&self) -> RunConstraintMode {
        match self.policy_key() {
            Some(_) => RunConstraintMode::EnforcedSubset,
            None => RunConstraintMode::Advisory,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DoneConditionEvidence, DoneConditionEvidenceParseError, RunConstraint, RunConstraintMode,
        RunConstraintPolicyKey, WorkspaceRelativePath, WorkspaceRelativePathError,
    };

    #[test]
    fn run_constraint_recognizes_enforced_subset_labels() {
        let cases = [
            ("path_scope", RunConstraintPolicyKey::PathScope),
            (
                "destructive_commands",
                RunConstraintPolicyKey::DestructiveCommandClass,
            ),
            (
                "external_commands",
                RunConstraintPolicyKey::ExternalCommandClass,
            ),
            (
                "approval_escalation",
                RunConstraintPolicyKey::ApprovalEscalation,
            ),
        ];

        for (label, expected) in cases {
            let constraint = RunConstraint {
                label: label.to_owned(),
                detail: String::from("value"),
            };
            assert_eq!(constraint.policy_key(), Some(expected));
            assert_eq!(constraint.mode(), RunConstraintMode::EnforcedSubset);
        }
    }

    #[test]
    fn run_constraint_keeps_unknown_labels_advisory() {
        let constraint = RunConstraint {
            label: String::from("do_not_touch_prod"),
            detail: String::from("configs"),
        };

        assert_eq!(constraint.policy_key(), None);
        assert_eq!(constraint.mode(), RunConstraintMode::Advisory);
    }

    #[test]
    fn done_condition_evidence_parses_v1_vocabulary() {
        assert_eq!(
            DoneConditionEvidence::parse("report_artifact_exists"),
            Ok(DoneConditionEvidence::ReportArtifactExists)
        );
        assert_eq!(
            DoneConditionEvidence::parse("file_exists:Cargo.toml"),
            Ok(DoneConditionEvidence::FileExists {
                path: WorkspaceRelativePath::parse("Cargo.toml")
                    .expect("relative path should parse")
            })
        );
        assert_eq!(
            DoneConditionEvidence::parse("path_changed:src"),
            Ok(DoneConditionEvidence::PathChanged {
                path: WorkspaceRelativePath::parse("src").expect("relative path should parse")
            })
        );
        assert_eq!(
            DoneConditionEvidence::parse("command_exit_zero:cargo test -q"),
            Ok(DoneConditionEvidence::CommandExitZero {
                command: String::from("cargo test -q")
            })
        );
    }

    #[test]
    fn done_condition_evidence_rejects_unsupported_or_blank_values() {
        assert_eq!(
            DoneConditionEvidence::parse(""),
            Err(DoneConditionEvidenceParseError::Empty)
        );
        assert_eq!(
            DoneConditionEvidence::parse("report artifact exists"),
            Err(DoneConditionEvidenceParseError::Unsupported {
                raw: String::from("report artifact exists")
            })
        );
        assert_eq!(
            DoneConditionEvidence::parse("file_exists:"),
            Err(DoneConditionEvidenceParseError::MissingValue {
                kind: String::from("file_exists")
            })
        );
        assert_eq!(
            DoneConditionEvidence::parse("file_exists:/tmp/outside"),
            Err(DoneConditionEvidenceParseError::InvalidWorkspacePath(
                WorkspaceRelativePathError::Absolute
            ))
        );
        assert_eq!(
            DoneConditionEvidence::parse("path_changed:../outside"),
            Err(DoneConditionEvidenceParseError::InvalidWorkspacePath(
                WorkspaceRelativePathError::ParentTraversal
            ))
        );
    }
}
