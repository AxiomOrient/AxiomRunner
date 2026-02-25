use axiom_core::decision::{DecisionOutcomePayload, DecisionPayload, DecisionPayloadError};
use axiom_core::effect::{EffectPayload, EffectPayloadError};
use axiom_core::event::{DomainEventPayload, DomainEventPayloadError, PolicyAuditPayload};
use axiom_core::intent::{IntentKindPayload, IntentPayload, IntentPayloadError};
use axiom_core::{
    Decision, DecisionOutcome, DomainEvent, Effect, ExecutionMode, Intent, PolicyAuditRecord,
    PolicyCode,
};

#[test]
fn intent_payload_round_trip() {
    let intents = vec![
        Intent::read("i-read", Some("alice".to_owned()), "alpha"),
        Intent::write("i-write", Some("alice".to_owned()), "alpha", "42"),
        Intent::remove("i-remove", Some("alice".to_owned()), "alpha"),
        Intent::freeze_writes("i-freeze", Some("system".to_owned())),
        Intent::halt("i-halt", Some("system".to_owned())),
    ];

    for intent in intents {
        let payload = intent.to_payload();
        let decoded = Intent::try_from_payload(payload).expect("payload should decode");
        assert_eq!(decoded, intent);
    }
}

#[test]
fn intent_payload_rejects_invalid_fields() {
    let invalid_intent_id = Intent::try_from_payload(IntentPayload {
        intent_id: "  ".to_owned(),
        actor_id: Some("alice".to_owned()),
        kind: IntentKindPayload::ReadFact {
            key: "alpha".to_owned(),
        },
    });
    assert_eq!(invalid_intent_id, Err(IntentPayloadError::IntentIdEmpty));

    let invalid_actor_id = Intent::try_from_payload(IntentPayload {
        intent_id: "i-1".to_owned(),
        actor_id: Some("\n".to_owned()),
        kind: IntentKindPayload::ReadFact {
            key: "alpha".to_owned(),
        },
    });
    assert_eq!(invalid_actor_id, Err(IntentPayloadError::ActorIdEmpty));

    let invalid_write_value = Intent::try_from_payload(IntentPayload {
        intent_id: "i-2".to_owned(),
        actor_id: Some("alice".to_owned()),
        kind: IntentKindPayload::WriteFact {
            key: "alpha".to_owned(),
            value: " ".to_owned(),
        },
    });
    assert_eq!(invalid_write_value, Err(IntentPayloadError::ValueEmpty));
}

#[test]
fn effect_payload_round_trip() {
    let effects = vec![
        Effect::PutFact {
            key: "alpha".to_owned(),
            value: "1".to_owned(),
        },
        Effect::RemoveFact {
            key: "beta".to_owned(),
        },
        Effect::SetMode(ExecutionMode::ReadOnly),
        Effect::SetMode(ExecutionMode::Halted),
    ];

    for effect in effects {
        let payload = effect.to_payload();
        let decoded = Effect::try_from_payload(payload).expect("payload should decode");
        assert_eq!(decoded, effect);
    }
}

#[test]
fn effect_payload_rejects_invalid_fields() {
    let invalid_put = Effect::try_from_payload(EffectPayload::PutFact {
        key: "alpha".to_owned(),
        value: "\t".to_owned(),
    });
    assert_eq!(invalid_put, Err(EffectPayloadError::ValueEmpty));

    let invalid_remove = Effect::try_from_payload(EffectPayload::RemoveFact {
        key: " ".to_owned(),
    });
    assert_eq!(invalid_remove, Err(EffectPayloadError::KeyEmpty));
}

#[test]
fn decision_payload_round_trip() {
    let decision = Decision {
        intent_id: "i-decision".to_owned(),
        outcome: DecisionOutcome::Accepted,
        reason: "accepted_with_effect".to_owned(),
        effects: vec![Effect::PutFact {
            key: "alpha".to_owned(),
            value: "1".to_owned(),
        }],
    };

    let payload = decision.to_payload();
    let decoded = Decision::try_from_payload(payload).expect("payload should decode");
    assert_eq!(decoded, decision);
}

#[test]
fn decision_payload_rejects_invalid_fields() {
    let invalid_reason = Decision::try_from_payload(DecisionPayload {
        intent_id: "i-1".to_owned(),
        outcome: DecisionOutcomePayload::Rejected,
        reason: " ".to_owned(),
        effects: Vec::new(),
    });
    assert_eq!(invalid_reason, Err(DecisionPayloadError::ReasonEmpty));

    let invalid_effect = Decision::try_from_payload(DecisionPayload {
        intent_id: "i-2".to_owned(),
        outcome: DecisionOutcomePayload::Accepted,
        reason: "accepted_with_effect".to_owned(),
        effects: vec![EffectPayload::RemoveFact {
            key: "\n".to_owned(),
        }],
    });
    assert_eq!(
        invalid_effect,
        Err(DecisionPayloadError::InvalidEffect(
            EffectPayloadError::KeyEmpty
        ))
    );
}

#[test]
fn domain_event_payload_round_trip() {
    let events = vec![
        DomainEvent::IntentAccepted {
            intent: Intent::read("i-read", Some("alice".to_owned()), "alpha"),
        },
        DomainEvent::PolicyEvaluated {
            audit: sample_audit(),
        },
        DomainEvent::DecisionCalculated {
            decision: sample_decision(),
        },
        DomainEvent::EffectsApplied {
            effects: vec![Effect::SetMode(ExecutionMode::ReadOnly)],
        },
    ];

    for event in events {
        let payload = event.to_payload();
        let decoded = DomainEvent::try_from_payload(payload).expect("payload should decode");
        assert_eq!(decoded, event);
    }
}

#[test]
fn domain_event_payload_rejects_invalid_fields() {
    let invalid_intent_event = DomainEvent::try_from_payload(DomainEventPayload::IntentAccepted {
        intent: IntentPayload {
            intent_id: "\t".to_owned(),
            actor_id: Some("alice".to_owned()),
            kind: IntentKindPayload::ReadFact {
                key: "alpha".to_owned(),
            },
        },
    });
    assert_eq!(
        invalid_intent_event,
        Err(DomainEventPayloadError::InvalidIntent(
            IntentPayloadError::IntentIdEmpty
        ))
    );

    let invalid_audit_event = DomainEvent::try_from_payload(DomainEventPayload::PolicyEvaluated {
        audit: PolicyAuditPayload {
            intent_id: "i-audit".to_owned(),
            actor_id: Some("gateway".to_owned()),
            state_revision: 3,
            allowed: false,
            code: PolicyCode::UnauthorizedControl,
            reason: " ".to_owned(),
        },
    });
    assert_eq!(
        invalid_audit_event,
        Err(DomainEventPayloadError::AuditReasonEmpty)
    );

    let invalid_decision_event =
        DomainEvent::try_from_payload(DomainEventPayload::DecisionCalculated {
            decision: DecisionPayload {
                intent_id: "i-decision".to_owned(),
                outcome: DecisionOutcomePayload::Accepted,
                reason: "accepted_with_effect".to_owned(),
                effects: vec![EffectPayload::PutFact {
                    key: " ".to_owned(),
                    value: "1".to_owned(),
                }],
            },
        });
    assert_eq!(
        invalid_decision_event,
        Err(DomainEventPayloadError::InvalidDecision(
            DecisionPayloadError::InvalidEffect(EffectPayloadError::KeyEmpty)
        ))
    );

    let invalid_effects_event = DomainEvent::try_from_payload(DomainEventPayload::EffectsApplied {
        effects: vec![EffectPayload::RemoveFact {
            key: "\n".to_owned(),
        }],
    });
    assert_eq!(
        invalid_effects_event,
        Err(DomainEventPayloadError::InvalidEffect(
            EffectPayloadError::KeyEmpty
        ))
    );
}

fn sample_audit() -> PolicyAuditRecord {
    PolicyAuditRecord {
        intent_id: "i-audit".to_owned(),
        actor_id: Some("gateway".to_owned()),
        state_revision: 3,
        allowed: false,
        code: PolicyCode::UnauthorizedControl,
        reason: "control actions require actor `system`".to_owned(),
    }
}

fn sample_decision() -> Decision {
    Decision {
        intent_id: "i-decision".to_owned(),
        outcome: DecisionOutcome::Rejected,
        reason: "policy denied".to_owned(),
        effects: vec![Effect::SetMode(ExecutionMode::Active)],
    }
}
