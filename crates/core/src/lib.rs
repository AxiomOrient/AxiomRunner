#![forbid(unsafe_code)]

pub mod audit;
pub mod decision;
pub mod effect;
pub mod event;
pub mod intent;
pub mod policy;
pub mod policy_codes;
pub mod projection;
pub mod reducer;
pub mod state;
mod validation;

pub use audit::{
    PolicyAuditPayload, PolicyAuditPayloadError, PolicyAuditRecord, build_policy_audit,
};
pub use decision::{Decision, DecisionOutcome, decide};
pub use effect::{Effect, effects_for_intent};
pub use event::DomainEvent;
pub use intent::{Intent, IntentKind};
pub use policy::{PolicyVerdict, evaluate_policy};
pub use policy_codes::PolicyCode;
pub use projection::{project, project_from};
pub use reducer::reduce;
pub use state::{AgentState, ExecutionMode};
