use crate::state_store::PendingRunSnapshot;
use crate::trace_store::TraceRunSummary;
use axonrunner_core::ExecutionMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateStatusInput {
    pub revision: u64,
    pub mode: ExecutionMode,
    pub last_intent_id: Option<String>,
    pub last_decision: Option<String>,
    pub last_policy_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStatusInput {
    pub provider_id: String,
    pub provider_model: String,
    pub provider_state: String,
    pub provider_detail: String,
    pub memory_enabled: bool,
    pub memory_state: String,
    pub tool_enabled: bool,
    pub tool_state: String,
    pub latest_run: Option<TraceRunSummary>,
    pub pending_run: Option<PendingRunSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusInput {
    pub state: StateStatusInput,
    pub runtime: RuntimeStatusInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub state: StateSnapshot,
    pub runtime: RuntimeSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSnapshot {
    pub revision: u64,
    pub mode: ExecutionMode,
    pub last_intent_id: Option<String>,
    pub last_decision: Option<String>,
    pub last_policy_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    pub provider_id: String,
    pub provider_model: String,
    pub provider_state: String,
    pub provider_detail: String,
    pub memory_enabled: bool,
    pub memory_state: String,
    pub tool_enabled: bool,
    pub tool_state: String,
    pub latest_run: Option<TraceRunSummary>,
    pub pending_run: Option<PendingRunSnapshot>,
}

impl From<StatusInput> for StatusSnapshot {
    fn from(input: StatusInput) -> Self {
        Self {
            state: StateSnapshot {
                revision: input.state.revision,
                mode: input.state.mode,
                last_intent_id: input.state.last_intent_id,
                last_decision: input.state.last_decision,
                last_policy_code: input.state.last_policy_code,
            },
            runtime: RuntimeSnapshot {
                provider_id: input.runtime.provider_id,
                provider_model: input.runtime.provider_model,
                provider_state: input.runtime.provider_state,
                provider_detail: input.runtime.provider_detail,
                memory_enabled: input.runtime.memory_enabled,
                memory_state: input.runtime.memory_state,
                tool_enabled: input.runtime.tool_enabled,
                tool_state: input.runtime.tool_state,
                latest_run: input.runtime.latest_run,
                pending_run: input.runtime.pending_run,
            },
        }
    }
}

pub fn render_status_lines(snapshot: &StatusSnapshot) -> Vec<String> {
    crate::operator_render::render_status_lines(snapshot)
}
