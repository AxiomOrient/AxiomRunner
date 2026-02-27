use crate::intent::{Intent, IntentKind};
use crate::state::ExecutionMode;
use crate::validation::ensure_not_blank;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    PutFact { key: String, value: String },
    RemoveFact { key: String },
    SetMode(ExecutionMode),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectPayloadError {
    KeyEmpty,
    ValueEmpty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectPayload {
    PutFact { key: String, value: String },
    RemoveFact { key: String },
    SetMode(ExecutionMode),
}

pub fn effects_for_intent(intent: &Intent) -> Vec<Effect> {
    match &intent.kind {
        IntentKind::ReadFact { .. } => Vec::new(),
        IntentKind::WriteFact { key, value } => vec![Effect::PutFact {
            key: key.clone(),
            value: value.clone(),
        }],
        IntentKind::RemoveFact { key } => vec![Effect::RemoveFact { key: key.clone() }],
        IntentKind::FreezeWrites => vec![Effect::SetMode(ExecutionMode::ReadOnly)],
        IntentKind::Halt => vec![Effect::SetMode(ExecutionMode::Halted)],
    }
}

impl Effect {
    pub fn to_payload(&self) -> EffectPayload {
        match self {
            Effect::PutFact { key, value } => EffectPayload::PutFact {
                key: key.clone(),
                value: value.clone(),
            },
            Effect::RemoveFact { key } => EffectPayload::RemoveFact { key: key.clone() },
            Effect::SetMode(mode) => EffectPayload::SetMode(*mode),
        }
    }

    pub fn try_from_payload(payload: EffectPayload) -> Result<Self, EffectPayloadError> {
        match payload {
            EffectPayload::PutFact { key, value } => {
                ensure_not_blank(key.as_str(), EffectPayloadError::KeyEmpty)?;
                ensure_not_blank(value.as_str(), EffectPayloadError::ValueEmpty)?;
                Ok(Effect::PutFact { key, value })
            }
            EffectPayload::RemoveFact { key } => {
                ensure_not_blank(key.as_str(), EffectPayloadError::KeyEmpty)?;
                Ok(Effect::RemoveFact { key })
            }
            EffectPayload::SetMode(mode) => Ok(Effect::SetMode(mode)),
        }
    }
}
