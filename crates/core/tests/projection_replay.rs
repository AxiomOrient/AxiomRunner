use axonrunner_core::{
    AgentState, Decision, DecisionOutcome, DomainEvent, Effect, ExecutionMode, Intent,
    PolicyAuditRecord, PolicyCode, project, project_from, reduce,
};

fn replay_event_stream() -> Vec<DomainEvent> {
    vec![
        DomainEvent::IntentAccepted {
            intent: Intent::write("intent-1", Some("alice".to_owned()), "alpha", "42"),
        },
        DomainEvent::PolicyEvaluated {
            audit: PolicyAuditRecord {
                intent_id: "intent-1".to_owned(),
                actor_id: Some("alice".to_owned()),
                state_revision: 0,
                allowed: true,
                code: PolicyCode::Allowed,
                reason: "allowed".to_owned(),
            },
        },
        DomainEvent::DecisionCalculated {
            decision: Decision {
                intent_id: "intent-1".to_owned(),
                outcome: DecisionOutcome::Accepted,
                reason: "accepted_with_effect".to_owned(),
                effects: vec![Effect::PutFact {
                    key: "alpha".to_owned(),
                    value: "42".to_owned(),
                }],
            },
        },
        DomainEvent::EffectsApplied {
            effects: vec![Effect::PutFact {
                key: "alpha".to_owned(),
                value: "42".to_owned(),
            }],
        },
        DomainEvent::IntentAccepted {
            intent: Intent::freeze_writes("intent-2", Some("system".to_owned())),
        },
        DomainEvent::PolicyEvaluated {
            audit: PolicyAuditRecord {
                intent_id: "intent-2".to_owned(),
                actor_id: Some("system".to_owned()),
                state_revision: 4,
                allowed: true,
                code: PolicyCode::Allowed,
                reason: "allowed".to_owned(),
            },
        },
        DomainEvent::DecisionCalculated {
            decision: Decision {
                intent_id: "intent-2".to_owned(),
                outcome: DecisionOutcome::Accepted,
                reason: "accepted_with_effect".to_owned(),
                effects: vec![Effect::SetMode(ExecutionMode::ReadOnly)],
            },
        },
        DomainEvent::EffectsApplied {
            effects: vec![Effect::SetMode(ExecutionMode::ReadOnly)],
        },
        DomainEvent::IntentAccepted {
            intent: Intent::remove("intent-3", Some("alice".to_owned()), "alpha"),
        },
        DomainEvent::PolicyEvaluated {
            audit: PolicyAuditRecord {
                intent_id: "intent-3".to_owned(),
                actor_id: Some("alice".to_owned()),
                state_revision: 8,
                allowed: false,
                code: PolicyCode::ReadOnlyMutation,
                reason: "fact mutations are blocked in read-only mode".to_owned(),
            },
        },
        DomainEvent::DecisionCalculated {
            decision: Decision {
                intent_id: "intent-3".to_owned(),
                outcome: DecisionOutcome::Rejected,
                reason: "fact mutations are blocked in read-only mode".to_owned(),
                effects: Vec::new(),
            },
        },
        DomainEvent::EffectsApplied {
            effects: Vec::new(),
        },
    ]
}

#[test]
fn projection_replay_is_deterministic_for_identical_event_streams() {
    let initial = AgentState::default().with_fact("seed", "baseline");
    let events = replay_event_stream();

    let first = project_from(&initial, &events);
    let second = project_from(&initial, &events);

    assert_eq!(first, second);
    assert_eq!(first.revision, initial.revision + events.len() as u64);
    assert_eq!(first.mode, ExecutionMode::ReadOnly);
    assert_eq!(first.facts.get("alpha").map(String::as_str), Some("42"));
    assert_eq!(
        first.facts.get("seed").map(String::as_str),
        Some("baseline")
    );
}

#[test]
fn projection_replay_projection_matches_manual_reduction_fold() {
    let initial = AgentState::default().with_fact("seed", "baseline");
    let events = replay_event_stream();

    let projected = project_from(&initial, &events);
    let folded = events
        .iter()
        .fold(initial.clone(), |state, event| reduce(&state, event));

    assert_eq!(projected, folded);
    assert_eq!(
        project(&events),
        project_from(&AgentState::default(), &events)
    );
}

#[test]
fn projection_replay_is_pure_for_state_and_event_inputs() {
    let initial = AgentState::default().with_fact("seed", "baseline");
    let events = replay_event_stream();
    let initial_snapshot = initial.clone();
    let events_snapshot = events.clone();

    let projected = project_from(&initial, &events);

    assert_eq!(initial, initial_snapshot);
    assert_eq!(events, events_snapshot);
    assert_ne!(projected, initial);
}

#[test]
fn golden_projection_replay_preserves_readonly_control_state_contract() {
    let projected = project_from(&AgentState::default(), &replay_event_stream());

    assert_eq!(projected.mode, ExecutionMode::ReadOnly);
    assert_eq!(projected.last_intent_id.as_deref(), Some("intent-3"));
    assert_eq!(projected.last_actor_id.as_deref(), Some("alice"));
    assert_eq!(projected.denied_count, 1);
    assert_eq!(projected.audit_count, 3);
    assert_eq!(projected.facts.get("alpha").map(String::as_str), Some("42"));
}
