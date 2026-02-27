use crate::contracts::{
    AdapterFuture, AdapterHealth, ProviderAdapter, ProviderRequest, ProviderResponse,
};
use crate::provider_openai::OpenAiCompatProvider;

pub const DEFAULT_PROVIDER_ID: &str = "mock-local";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRegistryEntry {
    pub id: &'static str,
    pub adapter_id: &'static str,
    pub aliases: &'static [&'static str],
}

const MOCK_LOCAL_ALIASES: &[&str] = &["provider.mock-local"];
const OPENAI_ALIASES: &[&str] = &["provider.openai"];
const OPENROUTER_ALIASES: &[&str] = &["provider.openrouter"];
const ANTHROPIC_ALIASES: &[&str] = &["provider.anthropic"];

static PROVIDER_REGISTRY: &[ProviderRegistryEntry] = &[
    ProviderRegistryEntry {
        id: DEFAULT_PROVIDER_ID,
        adapter_id: DEFAULT_PROVIDER_ID,
        aliases: MOCK_LOCAL_ALIASES,
    },
    ProviderRegistryEntry {
        id: "openai",
        adapter_id: "openai",
        aliases: OPENAI_ALIASES,
    },
    ProviderRegistryEntry {
        id: "openrouter",
        adapter_id: "openrouter",
        aliases: OPENROUTER_ALIASES,
    },
    ProviderRegistryEntry {
        id: "anthropic",
        adapter_id: "anthropic",
        aliases: ANTHROPIC_ALIASES,
    },
];

pub fn provider_registry() -> &'static [ProviderRegistryEntry] {
    PROVIDER_REGISTRY
}

/// Returns the canonical provider ID for the given name, or None if unknown.
/// Callers are responsible for trimming input; trim() here handles direct callers
/// outside `build_contract_provider` (e.g. runtime_compose).
pub fn resolve_provider_id(name: &str) -> Option<&'static str> {
    let key = name.trim();
    if key.is_empty() {
        return Some(DEFAULT_PROVIDER_ID);
    }
    resolve_provider_entry(key).map(|entry| entry.id)
}

pub fn resolve_provider_adapter_id(name: &str) -> Option<&'static str> {
    let key = name.trim();
    if key.is_empty() {
        return Some(DEFAULT_PROVIDER_ID);
    }
    resolve_provider_entry(key).map(|entry| entry.adapter_id)
}

/// Builds a ProviderAdapter for tool-log annotation purposes.
/// ProviderAdapter is used for runtime tool-log annotations only.
/// Real LLM/agent interaction goes through AgentAdapter (coclai).
pub fn build_contract_provider(id: &str) -> Result<Box<dyn ProviderAdapter>, String> {
    let canonical = resolve_provider_id(id).ok_or_else(|| {
        let available = provider_registry()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "unknown provider '{}'. supported: {available}. for real LLM use, configure AgentAdapter (coclai).",
            id.trim()
        )
    })?;

    match canonical {
        "mock-local" => Ok(Box::new(MockContractProvider)),
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
        _ => Err(String::from("unreachable provider registry state")),
    }
}

fn resolve_provider_entry(name: &str) -> Option<&'static ProviderRegistryEntry> {
    provider_registry().iter().find(|entry| {
        entry.id.eq_ignore_ascii_case(name)
            || entry
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(name))
    })
}

struct MockContractProvider;

impl ProviderAdapter for MockContractProvider {
    fn id(&self) -> &str {
        DEFAULT_PROVIDER_ID
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn complete(&self, request: ProviderRequest) -> AdapterFuture<'_, ProviderResponse> {
        Box::pin(async move {
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
        assert_eq!(resolve_provider_id("provider.openai"), Some("openai"));
        assert_eq!(
            resolve_provider_id("provider.openrouter"),
            Some("openrouter")
        );
        assert_eq!(resolve_provider_id("unknown-xyz"), None);
    }

    #[test]
    fn resolve_provider_adapter_id_returns_registry_adapter_id() {
        assert_eq!(resolve_provider_adapter_id(""), Some("mock-local"));
        assert_eq!(
            resolve_provider_adapter_id("mock-local"),
            Some("mock-local")
        );
        assert_eq!(
            resolve_provider_adapter_id("provider.openai"),
            Some("openai")
        );
        assert_eq!(
            resolve_provider_adapter_id("openrouter"),
            Some("openrouter")
        );
        assert_eq!(resolve_provider_adapter_id("unknown-xyz"), None);
    }

    #[test]
    fn build_contract_provider_fails_without_openai_key() {
        // Only run if env var is not set (CI-safe)
        if std::env::var("OPENAI_API_KEY").is_ok() {
            return; // skip if key is set
        }
        let err = build_contract_provider("openai")
            .err()
            .expect("should fail without key");
        assert!(err.contains("OPENAI_API_KEY"));
    }
}
