use axiom_core::{
    AgentState, Decision, DecisionOutcome, DomainEvent, Effect, ExecutionMode, Intent,
    PolicyAuditRecord, PolicyCode, reduce,
};

#[test]
fn reducer_cases_intent_accepted_updates_identifiers_without_mutating_input() {
    let before = AgentState {
        revision: 7,
        ..AgentState::default()
    };
    let event = DomainEvent::IntentAccepted {
        intent: Intent::write("intent-1", Some("alice".to_owned()), "alpha", "1"),
    };

    let after = reduce(&before, &event);

    assert_eq!(after.revision, 8);
    assert_eq!(after.last_intent_id.as_deref(), Some("intent-1"));
    assert_eq!(after.last_actor_id.as_deref(), Some("alice"));
    assert_eq!(after.facts, before.facts);
    assert_eq!(before.revision, 7);
    assert_eq!(before.last_intent_id, None);
    assert_eq!(before.last_actor_id, None);
}

#[test]
fn reducer_cases_policy_evaluated_allowed_updates_audit_without_denied_increment() {
    let before = AgentState {
        revision: 3,
        audit_count: 4,
        denied_count: 2,
        last_policy_code: PolicyCode::ActorMissing,
        ..AgentState::default()
    };
    let event = DomainEvent::PolicyEvaluated {
        audit: PolicyAuditRecord {
            intent_id: "intent-2".to_owned(),
            actor_id: Some("system".to_owned()),
            state_revision: 3,
            allowed: true,
            code: PolicyCode::Allowed,
            reason: "allowed".to_owned(),
        },
    };

    let after = reduce(&before, &event);

    assert_eq!(after.revision, 4);
    assert_eq!(after.audit_count, 5);
    assert_eq!(after.denied_count, 2);
    assert_eq!(after.last_policy_code, PolicyCode::Allowed);
    assert_eq!(before.audit_count, 4);
    assert_eq!(before.denied_count, 2);
    assert_eq!(before.last_policy_code, PolicyCode::ActorMissing);
}

#[test]
fn reducer_cases_policy_evaluated_denied_increments_denied_counter() {
    let before = AgentState {
        revision: 11,
        audit_count: 1,
        denied_count: 0,
        ..AgentState::default()
    };
    let event = DomainEvent::PolicyEvaluated {
        audit: PolicyAuditRecord {
            intent_id: "intent-3".to_owned(),
            actor_id: Some("alice".to_owned()),
            state_revision: 11,
            allowed: false,
            code: PolicyCode::ReadOnlyMutation,
            reason: "read-only mode".to_owned(),
        },
    };

    let after = reduce(&before, &event);

    assert_eq!(after.revision, 12);
    assert_eq!(after.audit_count, 2);
    assert_eq!(after.denied_count, 1);
    assert_eq!(after.last_policy_code, PolicyCode::ReadOnlyMutation);
}

#[test]
fn reducer_cases_decision_calculated_updates_last_decision() {
    let before = AgentState {
        revision: 5,
        ..AgentState::default()
    };
    let event = DomainEvent::DecisionCalculated {
        decision: Decision {
            intent_id: "intent-4".to_owned(),
            outcome: DecisionOutcome::Rejected,
            reason: "policy denied".to_owned(),
            effects: Vec::new(),
        },
    };

    let after = reduce(&before, &event);

    assert_eq!(after.revision, 6);
    assert_eq!(after.last_decision, Some(DecisionOutcome::Rejected));
    assert_eq!(before.last_decision, None);
}

#[test]
fn reducer_cases_effects_applied_are_processed_in_order_and_keep_input_pure() {
    let before = AgentState::default()
        .with_fact("alpha", "1")
        .with_fact("beta", "2");
    let event = DomainEvent::EffectsApplied {
        effects: vec![
            Effect::PutFact {
                key: "alpha".to_owned(),
                value: "first".to_owned(),
            },
            Effect::RemoveFact {
                key: "beta".to_owned(),
            },
            Effect::SetMode(ExecutionMode::ReadOnly),
            Effect::PutFact {
                key: "alpha".to_owned(),
                value: "final".to_owned(),
            },
            Effect::PutFact {
                key: "gamma".to_owned(),
                value: "3".to_owned(),
            },
        ],
    };

    let after = reduce(&before, &event);

    assert_eq!(after.revision, 1);
    assert_eq!(after.mode, ExecutionMode::ReadOnly);
    assert_eq!(after.facts.get("alpha").map(String::as_str), Some("final"));
    assert_eq!(after.facts.get("beta"), None);
    assert_eq!(after.facts.get("gamma").map(String::as_str), Some("3"));
    assert_eq!(before.revision, 0);
    assert_eq!(before.mode, ExecutionMode::Active);
    assert_eq!(before.facts.get("alpha").map(String::as_str), Some("1"));
    assert_eq!(before.facts.get("beta").map(String::as_str), Some("2"));
    assert_eq!(before.facts.get("gamma"), None);
}

#[test]
fn reducer_cases_policy_counters_and_revision_use_saturating_math() {
    let before = AgentState {
        revision: u64::MAX,
        audit_count: u64::MAX,
        denied_count: u64::MAX,
        ..AgentState::default()
    };
    let event = DomainEvent::PolicyEvaluated {
        audit: PolicyAuditRecord {
            intent_id: "intent-5".to_owned(),
            actor_id: Some("system".to_owned()),
            state_revision: u64::MAX,
            allowed: false,
            code: PolicyCode::RuntimeHalted,
            reason: "halted".to_owned(),
        },
    };

    let after = reduce(&before, &event);

    assert_eq!(after.revision, u64::MAX);
    assert_eq!(after.audit_count, u64::MAX);
    assert_eq!(after.denied_count, u64::MAX);
    assert_eq!(after.last_policy_code, PolicyCode::RuntimeHalted);
}
