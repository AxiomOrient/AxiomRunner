use crate::contracts::{
    AdapterFuture, AdapterHealth, ProviderAdapter, ProviderRequest, ProviderResponse,
};
use crate::provider_codex_runtime::CodexRuntimeProvider;
use crate::provider_openai::OpenAiCompatProvider;

pub const DEFAULT_PROVIDER_ID: &str = "mock-local";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRegistryEntry {
    pub id: &'static str,
}

static PROVIDER_REGISTRY: &[ProviderRegistryEntry] = &[
    ProviderRegistryEntry {
        id: DEFAULT_PROVIDER_ID,
    },
    ProviderRegistryEntry { id: "codek" },
    ProviderRegistryEntry { id: "openai" },
];

pub fn provider_registry() -> &'static [ProviderRegistryEntry] {
    PROVIDER_REGISTRY
}

pub fn resolve_provider_id(name: &str) -> Option<&'static str> {
    let key = name.trim();
    if key.is_empty() {
        return Some(DEFAULT_PROVIDER_ID);
    }
    provider_registry()
        .iter()
        .find(|entry| entry.id.eq_ignore_ascii_case(key))
        .map(|entry| entry.id)
}

pub fn build_contract_provider(id: &str) -> Result<Box<dyn ProviderAdapter>, String> {
    let canonical = resolve_provider_id(id).ok_or_else(|| {
        let available = provider_registry()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>()
            .join(", ");
        format!("unknown provider '{}'. supported: {available}", id.trim())
    })?;

    match canonical {
        "mock-local" => Ok(Box::new(MockContractProvider)),
        "codek" => Ok(Box::new(CodexRuntimeProvider::new("codek"))),
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| String::from("OPENAI_API_KEY env var not set"))?;
            Ok(Box::new(OpenAiCompatProvider::new(
                "openai",
                api_key,
                "https://api.openai.com/v1",
            )))
        }
        _ => Err(String::from("unreachable provider registry state")),
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

    fn complete(&self, request: ProviderRequest) -> AdapterFuture<'_, ProviderResponse> {
        Box::pin(async move {
            if request.prompt.trim().is_empty() {
                return Err(crate::error::AdapterError::invalid_input(
                    "prompt",
                    "must not be empty",
                ));
            }
            if request.max_tokens == 0 {
                return Err(crate::error::AdapterError::invalid_input(
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
    use super::{DEFAULT_PROVIDER_ID, provider_registry, resolve_provider_id};

    #[test]
    fn resolve_provider_id_accepts_only_canonical_ids() {
        assert_eq!(resolve_provider_id(""), Some(DEFAULT_PROVIDER_ID));
        assert_eq!(resolve_provider_id("mock-local"), Some("mock-local"));
        assert_eq!(resolve_provider_id("codek"), Some("codek"));
        assert_eq!(resolve_provider_id("openai"), Some("openai"));
        assert_eq!(resolve_provider_id("provider.openai"), None);
    }

    #[test]
    fn provider_registry_lists_only_retained_ids() {
        let ids = provider_registry()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["mock-local", "codek", "openai"]);
    }
}
