use crate::audit::{PolicyAuditPayload, PolicyAuditPayloadError, PolicyAuditRecord};
use crate::decision::{Decision, DecisionPayload, DecisionPayloadError};
use crate::effect::{Effect, EffectPayload, EffectPayloadError};
use crate::intent::{Intent, IntentPayload, IntentPayloadError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    IntentAccepted { intent: Intent },
    PolicyEvaluated { audit: PolicyAuditRecord },
    DecisionCalculated { decision: Decision },
    EffectsApplied { effects: Vec<Effect> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEventPayload {
    IntentAccepted { intent: IntentPayload },
    PolicyEvaluated { audit: PolicyAuditPayload },
    DecisionCalculated { decision: DecisionPayload },
    EffectsApplied { effects: Vec<EffectPayload> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEventPayloadError {
    InvalidIntent(IntentPayloadError),
    InvalidAudit(PolicyAuditPayloadError),
    InvalidDecision(DecisionPayloadError),
    InvalidEffect(EffectPayloadError),
}

impl DomainEvent {
    pub fn to_payload(&self) -> DomainEventPayload {
        match self {
            DomainEvent::IntentAccepted { intent } => DomainEventPayload::IntentAccepted {
                intent: intent.to_payload(),
            },
            DomainEvent::PolicyEvaluated { audit } => DomainEventPayload::PolicyEvaluated {
                audit: audit.to_payload(),
            },
            DomainEvent::DecisionCalculated { decision } => {
                DomainEventPayload::DecisionCalculated {
                    decision: decision.to_payload(),
                }
            }
            DomainEvent::EffectsApplied { effects } => DomainEventPayload::EffectsApplied {
                effects: effects.iter().map(Effect::to_payload).collect(),
            },
        }
    }

    pub fn try_from_payload(payload: DomainEventPayload) -> Result<Self, DomainEventPayloadError> {
        match payload {
            DomainEventPayload::IntentAccepted { intent } => Ok(DomainEvent::IntentAccepted {
                intent: Intent::try_from_payload(intent)
                    .map_err(DomainEventPayloadError::InvalidIntent)?,
            }),
            DomainEventPayload::PolicyEvaluated { audit } => Ok(DomainEvent::PolicyEvaluated {
                audit: PolicyAuditRecord::try_from_payload(audit)
                    .map_err(DomainEventPayloadError::InvalidAudit)?,
            }),
            DomainEventPayload::DecisionCalculated { decision } => {
                Ok(DomainEvent::DecisionCalculated {
                    decision: Decision::try_from_payload(decision)
                        .map_err(DomainEventPayloadError::InvalidDecision)?,
                })
            }
            DomainEventPayload::EffectsApplied { effects } => {
                let mut parsed_effects = Vec::with_capacity(effects.len());
                for effect in effects {
                    parsed_effects.push(
                        Effect::try_from_payload(effect)
                            .map_err(DomainEventPayloadError::InvalidEffect)?,
                    );
                }
                Ok(DomainEvent::EffectsApplied {
                    effects: parsed_effects,
                })
            }
        }
    }
}
