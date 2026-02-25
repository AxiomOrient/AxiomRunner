use crate::policy_codes::PolicyCode;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    InvalidIntent {
        field: &'static str,
        reason: &'static str,
    },
    PolicyViolation {
        code: PolicyCode,
        reason: &'static str,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::InvalidIntent { field, reason } => {
                write!(f, "invalid intent field `{field}`: {reason}")
            }
            CoreError::PolicyViolation { code, reason } => {
                write!(f, "policy violation ({:?}): {reason}", code)
            }
        }
    }
}

impl std::error::Error for CoreError {}

pub type CoreResult<T> = Result<T, CoreError>;
