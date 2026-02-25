use crate::agent_coclai::CoclaiAgentAdapter;
use crate::contracts::{AdapterHealth, AgentAdapter, AgentRequest, AgentResponse};
use crate::error::AdapterResult;

pub const DEFAULT_AGENT_ID: &str = "coclai";
pub const ENV_AGENT_ID: &str = "AXIOM_AGENT_ID";

/// Builds an AgentAdapter by name.
/// Reads `AXIOM_AGENT_ID` env var if not specified directly.
/// Supported backends: `coclai` (production), `mock` (testing/offline).
pub fn build_contract_agent(id: &str) -> Result<Box<dyn AgentAdapter>, String> {
    let resolved = if id.trim().is_empty() {
        std::env::var(ENV_AGENT_ID)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_AGENT_ID.to_owned())
    } else {
        id.trim().to_owned()
    };

    match resolved.as_str() {
        "coclai" => CoclaiAgentAdapter::new().map(|a| Box::new(a) as Box<dyn AgentAdapter>),
        "mock" => Ok(Box::new(MockAgentAdapter)),
        other => Err(format!(
            "unknown agent backend: '{other}'. supported: coclai, mock"
        )),
    }
}

/// Echo-based agent adapter for testing and offline use.
/// Returns the prompt as the response content.
pub struct MockAgentAdapter;

impl AgentAdapter for MockAgentAdapter {
    fn id(&self) -> &str {
        "mock"
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn run(&self, request: AgentRequest) -> AdapterResult<AgentResponse> {
        Ok(AgentResponse {
            content: request.prompt,
        })
    }
}
