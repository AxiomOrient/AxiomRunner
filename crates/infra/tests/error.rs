use axiom_infra::{InfraError, RetryClass};

#[test]
fn retryable_errors_map_to_retryable_class() {
    let unavailable = InfraError::Unavailable {
        dependency: "sqlite",
        reason: "connection refused",
    };
    let timeout = InfraError::Timeout {
        operation: "write_batch",
        reason: "deadline exceeded",
    };

    assert_eq!(unavailable.retry_class(), RetryClass::Retryable);
    assert_eq!(timeout.classify(), RetryClass::Retryable);
    assert_eq!(RetryClass::Retryable.as_str(), "retryable");
}

#[test]
fn invalid_input_is_non_retryable() {
    let error = InfraError::InvalidInput {
        field: "path",
        reason: "must be absolute",
    };

    assert_eq!(error.retry_class(), RetryClass::NonRetryable);
    assert_eq!(
        error.to_string(),
        "invalid infra input field `path`: must be absolute"
    );
}

#[test]
fn policy_violation_is_policy_denied() {
    let error = InfraError::PolicyViolation {
        code: "INFRA_DENY_WRITE",
        reason: "write path outside workspace",
    };

    assert_eq!(error.retry_class(), RetryClass::PolicyDenied);
    assert_eq!(RetryClass::PolicyDenied.as_str(), "policy_denied");
    assert_eq!(
        error.to_string(),
        "policy violation (INFRA_DENY_WRITE): write path outside workspace"
    );
}
