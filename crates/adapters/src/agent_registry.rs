use crate::agent_coclai::CoclaiAgentAdapter;
use crate::contracts::{AdapterHealth, AgentAdapter, AgentRequest, AgentResponse};
use crate::error::AdapterResult;

pub const DEFAULT_AGENT_ID: &str = "coclai";
pub const ENV_AGENT_ID: &str = "AXONRUNNER_AGENT_ID";
pub const ENV_ALLOW_MOCK_AGENT: &str = "AXONRUNNER_ALLOW_MOCK_AGENT";

/// Builds an AgentAdapter by name.
/// Reads `AXONRUNNER_AGENT_ID` env var if not specified directly.
/// Supported backends: `coclai` (production), `mock` (testing/offline).
pub fn build_contract_agent(id: &str) -> Result<Box<dyn AgentAdapter>, String> {
    let resolved = if id.trim().is_empty() {
        read_env_trimmed(ENV_AGENT_ID).unwrap_or_else(|| DEFAULT_AGENT_ID.to_owned())
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
        read_env_trimmed(ENV_ALLOW_MOCK_AGENT).as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn read_env_trimmed(key: &str) -> Option<String> {
    for candidate in env_candidates(key) {
        if let Ok(value) = std::env::var(candidate.as_str()) {
            let value = value.trim().to_owned();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn env_candidates(key: &str) -> [String; 2] {
    let legacy = key
        .strip_prefix("AXONRUNNER_")
        .map(|suffix| format!("AXIOM_{suffix}"))
        .unwrap_or_else(|| key.to_owned());
    [key.to_owned(), legacy]
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
