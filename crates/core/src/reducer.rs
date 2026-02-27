use crate::effect::Effect;
use crate::event::DomainEvent;
use crate::state::AgentState;

pub fn reduce(state: &AgentState, event: &DomainEvent) -> AgentState {
    let mut next = state.clone();
    next.revision = next.revision.saturating_add(1);

    match event {
        DomainEvent::IntentAccepted { intent } => {
            next.last_intent_id = Some(intent.intent_id.clone());
            next.last_actor_id = intent.actor_id.clone();
        }
        DomainEvent::PolicyEvaluated { audit } => {
            next.last_policy_code = Some(audit.code);
            next.audit_count = next.audit_count.saturating_add(1);
            if !audit.allowed {
                next.denied_count = next.denied_count.saturating_add(1);
            }
        }
        DomainEvent::DecisionCalculated { decision } => {
            next.last_decision = Some(decision.outcome);
        }
        DomainEvent::EffectsApplied { effects } => {
            for effect in effects {
                match effect {
                    Effect::PutFact { key, value } => {
                        next.facts.insert(key.clone(), value.clone());
                    }
                    Effect::RemoveFact { key } => {
                        next.facts.remove(key);
                    }
                    Effect::SetMode(mode) => {
                        next.mode = *mode;
                    }
                }
            }
        }
    }

    next
}
