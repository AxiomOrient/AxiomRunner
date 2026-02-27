use crate::effect::{Effect, EffectPayload, EffectPayloadError, effects_for_intent};
use crate::intent::Intent;
use crate::policy::PolicyVerdict;
use crate::validation::ensure_not_blank;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionOutcome {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionOutcomePayload {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub intent_id: String,
    pub outcome: DecisionOutcome,
    pub reason: String,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionPayload {
    pub intent_id: String,
    pub outcome: DecisionOutcomePayload,
    pub reason: String,
    pub effects: Vec<EffectPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionPayloadError {
    IntentIdEmpty,
    ReasonEmpty,
    InvalidEffect(EffectPayloadError),
}

pub fn decide(intent: &Intent, verdict: &PolicyVerdict) -> Decision {
    if !verdict.allowed {
        return Decision {
            intent_id: intent.intent_id.clone(),
            outcome: DecisionOutcome::Rejected,
            reason: verdict.reason.to_owned(),
            effects: Vec::new(),
        };
    }

    let effects = effects_for_intent(intent);
    let reason = if effects.is_empty() {
        "accepted_no_effect"
    } else {
        "accepted_with_effect"
    };

    Decision {
        intent_id: intent.intent_id.clone(),
        outcome: DecisionOutcome::Accepted,
        reason: reason.to_owned(),
        effects,
    }
}

impl Decision {
    pub fn to_payload(&self) -> DecisionPayload {
        DecisionPayload {
            intent_id: self.intent_id.clone(),
            outcome: self.outcome.to_payload(),
            reason: self.reason.clone(),
            effects: self.effects.iter().map(Effect::to_payload).collect(),
        }
    }

    pub fn try_from_payload(payload: DecisionPayload) -> Result<Self, DecisionPayloadError> {
        ensure_not_blank(
            payload.intent_id.as_str(),
            DecisionPayloadError::IntentIdEmpty,
        )?;
        ensure_not_blank(payload.reason.as_str(), DecisionPayloadError::ReasonEmpty)?;

        let mut effects = Vec::with_capacity(payload.effects.len());
        for effect in payload.effects {
            effects.push(
                Effect::try_from_payload(effect).map_err(DecisionPayloadError::InvalidEffect)?,
            );
        }

        Ok(Self {
            intent_id: payload.intent_id,
            outcome: DecisionOutcome::from_payload(payload.outcome),
            reason: payload.reason,
            effects,
        })
    }
}

impl DecisionOutcome {
    fn to_payload(self) -> DecisionOutcomePayload {
        match self {
            DecisionOutcome::Accepted => DecisionOutcomePayload::Accepted,
            DecisionOutcome::Rejected => DecisionOutcomePayload::Rejected,
        }
    }

    fn from_payload(payload: DecisionOutcomePayload) -> Self {
        match payload {
            DecisionOutcomePayload::Accepted => DecisionOutcome::Accepted,
            DecisionOutcomePayload::Rejected => DecisionOutcome::Rejected,
        }
    }
}
