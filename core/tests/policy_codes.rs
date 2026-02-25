use axiom_core::PolicyCode;
use axiom_core::policy_codes::POLICY_REJECTION_CODES;

fn rejection_occurrences(code: PolicyCode) -> usize {
    POLICY_REJECTION_CODES
        .iter()
        .filter(|candidate| **candidate == code)
        .count()
}

#[test]
fn policy_codes_variants_have_expected_strings_and_rejection_state() {
    let cases = [
        (PolicyCode::Allowed, "allowed", false),
        (PolicyCode::ActorMissing, "actor_missing", true),
        (PolicyCode::RuntimeHalted, "runtime_halted", true),
        (PolicyCode::ReadOnlyMutation, "readonly_mutation", true),
        (
            PolicyCode::UnauthorizedControl,
            "unauthorized_control",
            true,
        ),
        (PolicyCode::PayloadTooLarge, "payload_too_large", true),
    ];

    for (code, expected_str, expected_rejection) in cases {
        assert_eq!(code.as_str(), expected_str);
        assert_eq!(code.is_rejection(), expected_rejection);
    }
}

#[test]
fn policy_codes_rejection_catalog_has_one_entry_per_rejection_variant() {
    assert_eq!(POLICY_REJECTION_CODES.len(), 5);

    assert_eq!(rejection_occurrences(PolicyCode::Allowed), 0);
    assert_eq!(rejection_occurrences(PolicyCode::ActorMissing), 1);
    assert_eq!(rejection_occurrences(PolicyCode::RuntimeHalted), 1);
    assert_eq!(rejection_occurrences(PolicyCode::ReadOnlyMutation), 1);
    assert_eq!(rejection_occurrences(PolicyCode::UnauthorizedControl), 1);
    assert_eq!(rejection_occurrences(PolicyCode::PayloadTooLarge), 1);
}
