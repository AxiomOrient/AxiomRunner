use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryClass {
    Retryable,
    NonRetryable,
    PolicyDenied,
}

impl RetryClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            RetryClass::Retryable => "retryable",
            RetryClass::NonRetryable => "non_retryable",
            RetryClass::PolicyDenied => "policy_denied",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfraError {
    InvalidInput {
        field: &'static str,
        reason: &'static str,
    },
    Unavailable {
        dependency: &'static str,
        reason: &'static str,
    },
    Timeout {
        operation: &'static str,
        reason: &'static str,
    },
    PolicyViolation {
        code: &'static str,
        reason: &'static str,
    },
}

impl InfraError {
    pub const fn retry_class(&self) -> RetryClass {
        match self {
            InfraError::Unavailable { .. } | InfraError::Timeout { .. } => RetryClass::Retryable,
            InfraError::InvalidInput { .. } => RetryClass::NonRetryable,
            InfraError::PolicyViolation { .. } => RetryClass::PolicyDenied,
        }
    }

    pub const fn classify(&self) -> RetryClass {
        self.retry_class()
    }
}

impl fmt::Display for InfraError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InfraError::InvalidInput { field, reason } => {
                write!(f, "invalid infra input field `{field}`: {reason}")
            }
            InfraError::Unavailable { dependency, reason } => {
                write!(f, "infra dependency `{dependency}` unavailable: {reason}")
            }
            InfraError::Timeout { operation, reason } => {
                write!(f, "infra operation `{operation}` timed out: {reason}")
            }
            InfraError::PolicyViolation { code, reason } => {
                write!(f, "policy violation ({code}): {reason}")
            }
        }
    }
}

impl std::error::Error for InfraError {}

pub type InfraResult<T> = Result<T, InfraError>;
