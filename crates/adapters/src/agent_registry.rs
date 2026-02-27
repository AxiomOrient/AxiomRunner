use crate::agent_coclai::CoclaiAgentAdapter;
use crate::contracts::{AdapterHealth, AgentAdapter, AgentRequest, AgentResponse};
use crate::error::AdapterResult;

pub const DEFAULT_AGENT_ID: &str = "coclai";
pub const ENV_AGENT_ID: &str = "AXIOM_AGENT_ID";
pub const ENV_ALLOW_MOCK_AGENT: &str = "AXIOM_ALLOW_MOCK_AGENT";

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
        "mock" => {
            if allow_mock_agent() {
                Ok(Box::new(MockAgentAdapter))
            } else {
                Err(format!(
                    "agent backend 'mock' is blocked by default. set {ENV_ALLOW_MOCK_AGENT}=1 to enable it explicitly"
                ))
            }
        }
        other => Err(format!(
            "unknown agent backend: '{other}'. supported: coclai, mock"
        )),
    }
}

fn allow_mock_agent() -> bool {
    matches!(
        std::env::var(ENV_ALLOW_MOCK_AGENT).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
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
