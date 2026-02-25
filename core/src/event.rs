use crate::audit::PolicyAuditRecord;
use crate::decision::{Decision, DecisionPayload, DecisionPayloadError};
use crate::effect::{Effect, EffectPayload, EffectPayloadError};
use crate::intent::{Intent, IntentPayload, IntentPayloadError};
use crate::policy_codes::PolicyCode;
use crate::validation::ensure_not_blank;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    IntentAccepted { intent: Intent },
    PolicyEvaluated { audit: PolicyAuditRecord },
    DecisionCalculated { decision: Decision },
    EffectsApplied { effects: Vec<Effect> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyAuditPayload {
    pub intent_id: String,
    pub actor_id: Option<String>,
    pub state_revision: u64,
    pub allowed: bool,
    pub code: PolicyCode,
    pub reason: String,
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
    InvalidDecision(DecisionPayloadError),
    InvalidEffect(EffectPayloadError),
    AuditIntentIdEmpty,
    AuditActorIdEmpty,
    AuditReasonEmpty,
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
                audit: PolicyAuditRecord::try_from_payload(audit)?,
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

impl PolicyAuditRecord {
    fn to_payload(&self) -> PolicyAuditPayload {
        PolicyAuditPayload {
            intent_id: self.intent_id.clone(),
            actor_id: self.actor_id.clone(),
            state_revision: self.state_revision,
            allowed: self.allowed,
            code: self.code,
            reason: self.reason.clone(),
        }
    }

    fn try_from_payload(payload: PolicyAuditPayload) -> Result<Self, DomainEventPayloadError> {
        ensure_not_blank(
            payload.intent_id.as_str(),
            DomainEventPayloadError::AuditIntentIdEmpty,
        )?;
        if let Some(actor_id) = payload.actor_id.as_deref() {
            ensure_not_blank(actor_id, DomainEventPayloadError::AuditActorIdEmpty)?;
        }
        ensure_not_blank(
            payload.reason.as_str(),
            DomainEventPayloadError::AuditReasonEmpty,
        )?;

        Ok(Self {
            intent_id: payload.intent_id,
            actor_id: payload.actor_id,
            state_revision: payload.state_revision,
            allowed: payload.allowed,
            code: payload.code,
            reason: payload.reason,
        })
    }
}
