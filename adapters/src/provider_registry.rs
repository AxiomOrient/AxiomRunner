use crate::contracts::{AdapterHealth, ProviderAdapter, ProviderRequest, ProviderResponse};
use crate::error::AdapterResult;
use crate::provider_openai::OpenAiCompatProvider;

pub const DEFAULT_PROVIDER_ID: &str = "mock-local";

/// Returns the canonical provider ID for the given name, or None if unknown.
/// Callers are responsible for trimming input; trim() here handles direct callers
/// outside `build_contract_provider` (e.g. runtime_compose).
pub fn resolve_provider_id(name: &str) -> Option<&'static str> {
    match name.trim() {
        "" | "mock-local" => Some(DEFAULT_PROVIDER_ID),
        "openai" => Some("openai"),
        "anthropic" => Some("anthropic"),
        "openrouter" => Some("openrouter"),
        _ => None,
    }
}

/// Builds a ProviderAdapter for tool-log annotation purposes.
/// ProviderAdapter is used for runtime tool-log annotations only.
/// Real LLM/agent interaction goes through AgentAdapter (coclai).
pub fn build_contract_provider(id: &str) -> Result<Box<dyn ProviderAdapter>, String> {
    match id.trim() {
        "" | "mock-local" => Ok(Box::new(MockContractProvider)),
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| String::from("OPENAI_API_KEY env var not set"))?;
            Ok(Box::new(OpenAiCompatProvider::new(
                "openai",
                api_key,
                "https://api.openai.com/v1",
            )))
        }
        "openrouter" => {
            let api_key = std::env::var("OPENROUTER_API_KEY")
                .map_err(|_| String::from("OPENROUTER_API_KEY env var not set"))?;
            Ok(Box::new(OpenAiCompatProvider::new(
                "openrouter",
                api_key,
                "https://openrouter.ai/api/v1",
            )))
        }
        "anthropic" => Err(String::from(
            "anthropic provider uses a different API format; use openrouter with anthropic models instead",
        )),
        other => Err(format!(
            "unknown provider '{other}'. supported: mock-local, openai, openrouter, anthropic. \
             for real LLM use, configure AgentAdapter (coclai)."
        )),
    }
}

struct MockContractProvider;

impl ProviderAdapter for MockContractProvider {
    fn id(&self) -> &str {
        DEFAULT_PROVIDER_ID
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn complete(&self, request: ProviderRequest) -> AdapterResult<ProviderResponse> {
        use crate::error::AdapterError;
        if request.prompt.trim().is_empty() {
            return Err(AdapterError::invalid_input("prompt", "must not be empty"));
        }
        if request.max_tokens == 0 {
            return Err(AdapterError::invalid_input(
                "max_tokens",
                "must be greater than zero",
            ));
        }
        Ok(ProviderResponse {
            content: request.prompt,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_provider_id_recognizes_new_providers() {
        assert_eq!(resolve_provider_id("openai"), Some("openai"));
        assert_eq!(resolve_provider_id("anthropic"), Some("anthropic"));
        assert_eq!(resolve_provider_id("openrouter"), Some("openrouter"));
        assert_eq!(resolve_provider_id("unknown-xyz"), None);
    }

    #[test]
    fn build_contract_provider_fails_without_openai_key() {
        // Only run if env var is not set (CI-safe)
        if std::env::var("OPENAI_API_KEY").is_ok() {
            return; // skip if key is set
        }
        let err = build_contract_provider("openai").err().expect("should fail without key");
        assert!(err.contains("OPENAI_API_KEY"));
    }
}
