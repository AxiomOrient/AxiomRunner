use axonrunner_core::{
    AgentState, DecisionOutcome, DomainEvent, ExecutionMode, Intent, PolicyCode,
    build_policy_audit, decide, evaluate_policy, project, project_from, reduce,
};

fn pipeline_events(state: &AgentState, intent: Intent) -> Vec<DomainEvent> {
    let verdict = evaluate_policy(state, &intent);
    let decision = decide(&intent, &verdict);
    let effects = decision.effects.clone();
    let audit = build_policy_audit(state, &intent, &verdict);

    vec![
        DomainEvent::IntentAccepted { intent },
        DomainEvent::PolicyEvaluated { audit },
        DomainEvent::DecisionCalculated { decision },
        DomainEvent::EffectsApplied { effects },
    ]
}

#[test]
fn policy_denial_audit_contains_required_fields() {
    let state = AgentState {
        revision: 7,
        ..AgentState::default()
    };
    let intent = Intent::freeze_writes("i-deny", Some("gateway".to_owned()));

    let verdict = evaluate_policy(&state, &intent);
    assert!(!verdict.allowed);
    assert_eq!(verdict.code, PolicyCode::UnauthorizedControl);

    let audit = build_policy_audit(&state, &intent, &verdict);
    assert_eq!(audit.intent_id, "i-deny");
    assert_eq!(audit.actor_id.as_deref(), Some("gateway"));
    assert_eq!(audit.state_revision, 7);
    assert!(!audit.allowed);
    assert_eq!(audit.code, PolicyCode::UnauthorizedControl);
    assert_eq!(audit.reason, "control actions require actor `system`");
}

#[test]
fn policy_allow_audit_contains_required_fields() {
    let state = AgentState {
        revision: 3,
        ..AgentState::default()
    };
    let intent = Intent::write("i-allow", Some("alice".to_owned()), "alpha", "1");

    let verdict = evaluate_policy(&state, &intent);
    assert!(verdict.allowed);
    assert_eq!(verdict.code, PolicyCode::Allowed);

    let audit = build_policy_audit(&state, &intent, &verdict);
    assert_eq!(audit.intent_id, "i-allow");
    assert_eq!(audit.actor_id.as_deref(), Some("alice"));
    assert_eq!(audit.state_revision, 3);
    assert!(audit.allowed);
    assert_eq!(audit.code, PolicyCode::Allowed);
    assert_eq!(audit.reason, "allowed");
}

#[test]
fn projection_applies_effects_without_mutating_input_state() {
    let initial = AgentState::default();
    let intent = Intent::write("i-2", Some("alice".to_owned()), "alpha", "42");
    let events = pipeline_events(&initial, intent);

    let next = project(&events);

    assert_eq!(next.facts.get("alpha"), Some(&"42".to_owned()));
    assert_eq!(next.revision, 4);
    assert_eq!(next.last_decision, Some(DecisionOutcome::Accepted));
    assert_eq!(initial.facts.get("alpha"), None);
    assert_eq!(initial.revision, 0);
}

#[test]
fn readonly_mode_blocks_fact_mutation() {
    let state = AgentState {
        mode: ExecutionMode::ReadOnly,
        ..AgentState::default()
    };
    let intent = Intent::write("i-3", Some("alice".to_owned()), "beta", "99");
    let events = pipeline_events(&state, intent);

    let next = project_from(&state, &events);

    assert_eq!(next.facts.get("beta"), None);
    assert_eq!(next.last_policy_code, Some(PolicyCode::ReadOnlyMutation));
    assert_eq!(next.last_decision, Some(DecisionOutcome::Rejected));
    assert_eq!(next.denied_count, 1);
    assert_eq!(next.audit_count, 1);
}

#[test]
fn projection_matches_manual_reduction_fold() {
    let initial = AgentState::default();

    let mut events = pipeline_events(
        &initial,
        Intent::write("i-4", Some("alice".to_owned()), "key", "value"),
    );
    events.extend(pipeline_events(
        &initial,
        Intent::remove("i-5", Some("alice".to_owned()), "key"),
    ));

    let projected = project(&events);
    let folded = events
        .iter()
        .fold(initial.clone(), |state, event| reduce(&state, event));

    assert_eq!(projected, folded);
}
