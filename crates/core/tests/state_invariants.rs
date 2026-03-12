use axonrunner_core::{
    AgentState, Decision, DecisionOutcome, DomainEvent, Effect, ExecutionMode, Intent,
    PolicyAuditRecord, PolicyCode, project_from, reduce,
};

#[derive(Clone, Copy)]
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.state
    }

    fn next_usize(&mut self, upper: usize) -> usize {
        (self.next_u64() as usize) % upper
    }

    fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }
}

fn next_token(rng: &mut Lcg, prefix: &str) -> String {
    format!("{prefix}-{}", rng.next_u64())
}

fn random_actor(rng: &mut Lcg) -> Option<String> {
    match rng.next_usize(4) {
        0 => None,
        1 => Some(String::new()),
        2 => Some("alice".to_owned()),
        _ => Some("system".to_owned()),
    }
}

fn random_mode(rng: &mut Lcg) -> ExecutionMode {
    match rng.next_usize(3) {
        0 => ExecutionMode::Active,
        1 => ExecutionMode::ReadOnly,
        _ => ExecutionMode::Halted,
    }
}

fn random_intent(rng: &mut Lcg) -> Intent {
    let intent_id = next_token(rng, "intent");
    let actor = random_actor(rng);

    match rng.next_usize(5) {
        0 => Intent::read(intent_id, actor, next_token(rng, "key")),
        1 => Intent::write(
            intent_id,
            actor,
            next_token(rng, "key"),
            next_token(rng, "value"),
        ),
        2 => Intent::remove(intent_id, actor, next_token(rng, "key")),
        3 => Intent::freeze_writes(intent_id, actor),
        _ => Intent::halt(intent_id, actor),
    }
}

fn random_policy_code(rng: &mut Lcg, allowed: bool) -> PolicyCode {
    if allowed {
        return PolicyCode::Allowed;
    }

    match rng.next_usize(5) {
        0 => PolicyCode::ActorMissing,
        1 => PolicyCode::RuntimeHalted,
        2 => PolicyCode::ReadOnlyMutation,
        3 => PolicyCode::UnauthorizedControl,
        _ => PolicyCode::PayloadTooLarge,
    }
}

fn random_effects(rng: &mut Lcg) -> Vec<Effect> {
    let len = 1 + rng.next_usize(4);
    let mut effects = Vec::with_capacity(len);
    for _ in 0..len {
        let effect = match rng.next_usize(3) {
            0 => Effect::PutFact {
                key: next_token(rng, "key"),
                value: next_token(rng, "value"),
            },
            1 => Effect::RemoveFact {
                key: next_token(rng, "key"),
            },
            _ => Effect::SetMode(random_mode(rng)),
        };
        effects.push(effect);
    }
    effects
}

fn random_event(rng: &mut Lcg) -> DomainEvent {
    match rng.next_usize(4) {
        0 => DomainEvent::IntentAccepted {
            intent: random_intent(rng),
        },
        1 => {
            let allowed = rng.next_bool();
            DomainEvent::PolicyEvaluated {
                audit: PolicyAuditRecord {
                    intent_id: next_token(rng, "intent"),
                    actor_id: random_actor(rng),
                    state_revision: rng.next_u64(),
                    allowed,
                    code: random_policy_code(rng, allowed),
                    reason: next_token(rng, "reason"),
                },
            }
        }
        2 => DomainEvent::DecisionCalculated {
            decision: Decision {
                intent_id: next_token(rng, "intent"),
                outcome: if rng.next_bool() {
                    DecisionOutcome::Accepted
                } else {
                    DecisionOutcome::Rejected
                },
                reason: next_token(rng, "reason"),
                effects: Vec::new(),
            },
        },
        _ => DomainEvent::EffectsApplied {
            effects: random_effects(rng),
        },
    }
}

fn generated_event_stream(seed: u64) -> Vec<DomainEvent> {
    let mut rng = Lcg::new(seed);
    let len = 1 + rng.next_usize(128);
    let mut events = Vec::with_capacity(len);

    for _ in 0..len {
        events.push(random_event(&mut rng));
    }

    events
}

#[test]
fn state_invariants_flag_invalid_counter_ordering() {
    let state = AgentState {
        denied_count: 1,
        ..AgentState::default()
    };
    assert!(!state.invariants_hold());

    let state = AgentState {
        revision: 1,
        audit_count: 2,
        ..AgentState::default()
    };
    assert!(!state.invariants_hold());
}

#[test]
fn state_invariants_hold_for_generated_reduction_prefixes() {
    for seed in 1_u64..=256 {
        let events = generated_event_stream(seed);
        let mut state = AgentState::default();

        for (index, event) in events.iter().enumerate() {
            state = reduce(&state, event);
            assert!(
                state.invariants_hold(),
                "seed={seed}, event_index={index}, event={event:?}, state={state:?}"
            );
        }
    }
}

#[test]
fn state_invariants_hold_for_projected_generated_streams() {
    for seed in 257_u64..=512 {
        let events = generated_event_stream(seed);
        let projected = project_from(&AgentState::default(), &events);

        let expected_audit = events
            .iter()
            .filter(|event| matches!(event, DomainEvent::PolicyEvaluated { .. }))
            .count() as u64;

        let expected_denied = events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    DomainEvent::PolicyEvaluated {
                        audit: PolicyAuditRecord { allowed: false, .. }
                    }
                )
            })
            .count() as u64;

        assert!(
            projected.invariants_hold(),
            "seed={seed}, projected_state={projected:?}"
        );
        assert_eq!(projected.revision, events.len() as u64);
        assert_eq!(projected.audit_count, expected_audit);
        assert_eq!(projected.denied_count, expected_denied);
    }
}
