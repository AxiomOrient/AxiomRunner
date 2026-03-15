use axiomrunner_adapters::{AdapterError, AdapterErrorKind, RetryClass};
use axiomrunner_core::PolicyCode;

#[test]
fn adapter_error_classification_is_explicit_and_data_first() {
    let invalid = AdapterError::invalid_input("prompt", "must not be empty");
    assert_eq!(invalid.kind(), AdapterErrorKind::InvalidInput);
    assert_eq!(invalid.retry_class(), RetryClass::NonRetryable);

    let missing = AdapterError::not_found("tool", "echo");
    assert_eq!(missing.kind(), AdapterErrorKind::NotFound);
    assert_eq!(missing.retry_class(), RetryClass::NonRetryable);

    let unavailable = AdapterError::unavailable("runtime", "is not running");
    assert_eq!(unavailable.kind(), AdapterErrorKind::Unavailable);
    assert_eq!(unavailable.retry_class(), RetryClass::Retryable);

    let failed = AdapterError::failed(
        "provider.complete",
        "upstream connection reset",
        RetryClass::Retryable,
    );
    assert_eq!(failed.kind(), AdapterErrorKind::Failed);
    assert_eq!(failed.retry_class(), RetryClass::Retryable);
}

#[test]
fn adapter_error_policy_violation_maps_to_policy_denied_retry_class() {
    let error =
        AdapterError::policy_violation(PolicyCode::PayloadTooLarge, "payload exceeds limit");

    assert_eq!(error.kind(), AdapterErrorKind::PolicyViolation);
    assert_eq!(error.retry_class(), RetryClass::PolicyDenied);
    assert_eq!(
        format!("{error}"),
        "policy violation (PayloadTooLarge): payload exceeds limit"
    );
}
