use axiom_core::policy::{MAX_KEY_LEN, MAX_VALUE_LEN};
use axiom_core::{AgentState, ExecutionMode, Intent, PolicyCode, evaluate_policy};

fn assert_verdict(
    state: &AgentState,
    intent: &Intent,
    expected_allowed: bool,
    expected_code: PolicyCode,
    expected_reason: &str,
) {
    let verdict = evaluate_policy(state, intent);

    assert_eq!(verdict.allowed, expected_allowed);
    assert_eq!(verdict.code, expected_code);
    assert_eq!(verdict.reason, expected_reason);
}

#[test]
fn policy_decision_invalid_actor_none_is_rejected() {
    let state = AgentState::default();
    let intent = Intent::read("intent-read", None, "alpha");

    assert_verdict(
        &state,
        &intent,
        false,
        PolicyCode::ActorMissing,
        "actor is required",
    );
}

#[test]
fn policy_decision_invalid_actor_blank_is_rejected() {
    let state = AgentState::default();
    let intent = Intent::read("intent-read", Some("   \t".to_owned()), "alpha");

    assert_verdict(
        &state,
        &intent,
        false,
        PolicyCode::ActorMissing,
        "actor is required",
    );
}

#[test]
fn policy_decision_rule_order_actor_missing_precedes_runtime_halted() {
    let state = AgentState {
        mode: ExecutionMode::Halted,
        ..AgentState::default()
    };
    let intent = Intent::read("intent-read", None, "alpha");

    assert_verdict(
        &state,
        &intent,
        false,
        PolicyCode::ActorMissing,
        "actor is required",
    );
}

#[test]
fn policy_decision_runtime_halted_rejects_before_control_checks() {
    let state = AgentState {
        mode: ExecutionMode::Halted,
        ..AgentState::default()
    };
    let intent = Intent::freeze_writes("intent-freeze", Some("system".to_owned()));

    assert_verdict(
        &state,
        &intent,
        false,
        PolicyCode::RuntimeHalted,
        "runtime is halted",
    );
}

#[test]
fn policy_decision_read_only_blocks_fact_mutations_but_allows_reads() {
    let state = AgentState {
        mode: ExecutionMode::ReadOnly,
        ..AgentState::default()
    };

    let write_intent = Intent::write("intent-write", Some("alice".to_owned()), "alpha", "1");
    assert_verdict(
        &state,
        &write_intent,
        false,
        PolicyCode::ReadOnlyMutation,
        "fact mutations are blocked in read-only mode",
    );

    let remove_intent = Intent::remove("intent-remove", Some("alice".to_owned()), "alpha");
    assert_verdict(
        &state,
        &remove_intent,
        false,
        PolicyCode::ReadOnlyMutation,
        "fact mutations are blocked in read-only mode",
    );

    let read_intent = Intent::read("intent-read", Some("alice".to_owned()), "alpha");
    assert_verdict(&state, &read_intent, true, PolicyCode::Allowed, "allowed");
}

#[test]
fn policy_decision_control_requires_system_actor() {
    let state = AgentState::default();

    let freeze_intent = Intent::freeze_writes("intent-freeze", Some("gateway".to_owned()));
    assert_verdict(
        &state,
        &freeze_intent,
        false,
        PolicyCode::UnauthorizedControl,
        "control actions require actor `system`",
    );

    let halt_intent = Intent::halt("intent-halt", Some("gateway".to_owned()));
    assert_verdict(
        &state,
        &halt_intent,
        false,
        PolicyCode::UnauthorizedControl,
        "control actions require actor `system`",
    );
}

#[test]
fn policy_decision_control_allows_system_actor() {
    let state = AgentState::default();

    let freeze_intent = Intent::freeze_writes("intent-freeze", Some("system".to_owned()));
    assert_verdict(&state, &freeze_intent, true, PolicyCode::Allowed, "allowed");

    let halt_intent = Intent::halt("intent-halt", Some("system".to_owned()));
    assert_verdict(&state, &halt_intent, true, PolicyCode::Allowed, "allowed");
}

#[test]
fn policy_decision_key_size_boundaries_are_explicit_for_read_and_remove() {
    let state = AgentState::default();
    let max_key = "k".repeat(MAX_KEY_LEN);
    let oversized_key = "k".repeat(MAX_KEY_LEN + 1);

    let read_max = Intent::read("intent-read-max", Some("alice".to_owned()), &max_key);
    assert_verdict(&state, &read_max, true, PolicyCode::Allowed, "allowed");

    let read_oversized = Intent::read(
        "intent-read-oversized",
        Some("alice".to_owned()),
        &oversized_key,
    );
    assert_verdict(
        &state,
        &read_oversized,
        false,
        PolicyCode::PayloadTooLarge,
        "key exceeds limit",
    );

    let remove_max = Intent::remove("intent-remove-max", Some("alice".to_owned()), &max_key);
    assert_verdict(&state, &remove_max, true, PolicyCode::Allowed, "allowed");

    let remove_oversized = Intent::remove(
        "intent-remove-oversized",
        Some("alice".to_owned()),
        &oversized_key,
    );
    assert_verdict(
        &state,
        &remove_oversized,
        false,
        PolicyCode::PayloadTooLarge,
        "key exceeds limit",
    );
}

#[test]
fn policy_decision_write_payload_boundaries_are_explicit() {
    let state = AgentState::default();
    let max_key = "k".repeat(MAX_KEY_LEN);
    let oversized_key = "k".repeat(MAX_KEY_LEN + 1);
    let max_value = "v".repeat(MAX_VALUE_LEN);
    let oversized_value = "v".repeat(MAX_VALUE_LEN + 1);

    let write_max = Intent::write(
        "intent-write-max",
        Some("alice".to_owned()),
        &max_key,
        &max_value,
    );
    assert_verdict(&state, &write_max, true, PolicyCode::Allowed, "allowed");

    let write_oversized_key = Intent::write(
        "intent-write-key-oversized",
        Some("alice".to_owned()),
        &oversized_key,
        "ok",
    );
    assert_verdict(
        &state,
        &write_oversized_key,
        false,
        PolicyCode::PayloadTooLarge,
        "payload exceeds limit",
    );

    let write_oversized_value = Intent::write(
        "intent-write-value-oversized",
        Some("alice".to_owned()),
        "ok",
        &oversized_value,
    );
    assert_verdict(
        &state,
        &write_oversized_value,
        false,
        PolicyCode::PayloadTooLarge,
        "payload exceeds limit",
    );
}

#[test]
fn policy_decision_evaluation_is_pure_for_state_and_intent() {
    let state = AgentState::default()
        .with_fact("alpha", "1")
        .with_fact("beta", "2");
    let intent = Intent::write("intent-write", Some("alice".to_owned()), "alpha", "3");

    let before_state = state.clone();
    let before_intent = intent.clone();

    let verdict = evaluate_policy(&state, &intent);

    assert!(verdict.allowed);
    assert_eq!(verdict.code, PolicyCode::Allowed);
    assert_eq!(verdict.reason, "allowed");
    assert_eq!(state, before_state);
    assert_eq!(intent, before_intent);
}
