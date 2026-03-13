use crate::display::mode_name;
use axonrunner_core::ExecutionMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateStatusInput {
    pub revision: u64,
    pub mode: ExecutionMode,
    pub facts: usize,
    pub denied: u64,
    pub audit: u64,
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
    pub facts: usize,
    pub denied: u64,
    pub audit: u64,
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
}

impl From<StatusInput> for StatusSnapshot {
    fn from(input: StatusInput) -> Self {
        Self {
            state: StateSnapshot {
                revision: input.state.revision,
                mode: input.state.mode,
                facts: input.state.facts,
                denied: input.state.denied,
                audit: input.state.audit,
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
            },
        }
    }
}

pub fn render_status_lines(snapshot: &StatusSnapshot) -> Vec<String> {
    vec![
        format!(
            "status revision={} mode={} facts={} denied={} audit={}",
            snapshot.state.revision,
            mode_name(snapshot.state.mode),
            snapshot.state.facts,
            snapshot.state.denied,
            snapshot.state.audit
        ),
        format!(
            "status runtime provider_id={} provider_model={} provider_state={} provider_detail={} memory_enabled={} memory_state={} tool_enabled={} tool_state={}",
            snapshot.runtime.provider_id,
            snapshot.runtime.provider_model,
            snapshot.runtime.provider_state,
            snapshot.runtime.provider_detail,
            snapshot.runtime.memory_enabled,
            snapshot.runtime.memory_state,
            snapshot.runtime.tool_enabled,
            snapshot.runtime.tool_state
        ),
    ]
}
