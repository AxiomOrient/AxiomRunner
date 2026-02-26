use axiom_core::PolicyCode;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterErrorKind {
    InvalidInput,
    NotFound,
    Unavailable,
    Failed,
    PolicyViolation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterError {
    InvalidInput {
        field: &'static str,
        reason: &'static str,
    },
    NotFound {
        resource: &'static str,
        id: String,
    },
    Unavailable {
        component: &'static str,
        reason: &'static str,
        retry: RetryClass,
    },
    Failed {
        operation: &'static str,
        reason: String,
        retry: RetryClass,
    },
    PolicyViolation {
        code: PolicyCode,
        reason: &'static str,
    },
}

impl AdapterError {
    pub const fn invalid_input(field: &'static str, reason: &'static str) -> Self {
        Self::InvalidInput { field, reason }
    }

    pub fn not_found(resource: &'static str, id: impl Into<String>) -> Self {
        Self::NotFound {
            resource,
            id: id.into(),
        }
    }

    pub const fn unavailable(component: &'static str, reason: &'static str) -> Self {
        Self::Unavailable {
            component,
            reason,
            retry: RetryClass::Retryable,
        }
    }

    pub const fn unavailable_with_class(
        component: &'static str,
        reason: &'static str,
        retry: RetryClass,
    ) -> Self {
        Self::Unavailable {
            component,
            reason,
            retry,
        }
    }

    pub fn failed(operation: &'static str, reason: impl Into<String>, retry: RetryClass) -> Self {
        Self::Failed {
            operation,
            reason: reason.into(),
            retry,
        }
    }

    pub const fn policy_violation(code: PolicyCode, reason: &'static str) -> Self {
        Self::PolicyViolation { code, reason }
    }

    pub fn kind(&self) -> AdapterErrorKind {
        match self {
            AdapterError::InvalidInput { .. } => AdapterErrorKind::InvalidInput,
            AdapterError::NotFound { .. } => AdapterErrorKind::NotFound,
            AdapterError::Unavailable { .. } => AdapterErrorKind::Unavailable,
            AdapterError::Failed { .. } => AdapterErrorKind::Failed,
            AdapterError::PolicyViolation { .. } => AdapterErrorKind::PolicyViolation,
        }
    }

    pub fn retry_class(&self) -> RetryClass {
        match self {
            AdapterError::InvalidInput { .. } => RetryClass::NonRetryable,
            AdapterError::NotFound { .. } => RetryClass::NonRetryable,
            AdapterError::Unavailable { retry, .. } => *retry,
            AdapterError::Failed { retry, .. } => *retry,
            AdapterError::PolicyViolation { .. } => RetryClass::PolicyDenied,
        }
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdapterError::InvalidInput { field, reason } => {
                write!(f, "invalid adapter input `{field}`: {reason}")
            }
            AdapterError::NotFound { resource, id } => {
                write!(f, "adapter resource `{resource}` not found: {id}")
            }
            AdapterError::Unavailable {
                component,
                reason,
                retry,
            } => write!(
                f,
                "adapter component `{component}` unavailable ({}): {reason}",
                retry.as_str()
            ),
            AdapterError::Failed {
                operation,
                reason,
                retry,
            } => write!(
                f,
                "adapter operation `{operation}` failed ({}): {reason}",
                retry.as_str()
            ),
            AdapterError::PolicyViolation { code, reason } => {
                write!(f, "policy violation ({:?}): {reason}", code)
            }
        }
    }
}

impl std::error::Error for AdapterError {}

pub type AdapterResult<T> = Result<T, AdapterError>;

/// Classify a reqwest error into a `&'static str` label suitable for
/// AdapterError messages. Using `&'static str` ensures that no URL
/// or token information from the `reqwest::Error` can leak into logs.
pub(crate) fn classify_reqwest_error(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connection failed"
    } else if e.is_status() {
        "unexpected status"
    } else {
        "request failed"
    }
}
