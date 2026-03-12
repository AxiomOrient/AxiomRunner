use crate::contracts::{
    AdapterFuture, AdapterHealth, ProviderAdapter, ProviderRequest, ProviderResponse,
};
use crate::error::{AdapterError, RetryClass};
use codex_runtime::runtime::{Client, ClientConfig, SessionConfig};
use std::path::PathBuf;
use std::time::Duration;

pub const ENV_CODEX_BIN: &str = "AXONRUNNER_CODEX_BIN";
const DEFAULT_CODEX_BIN: &str = "codex";
const DEFAULT_SESSION_CWD: &str = ".";

pub struct CodexRuntimeProvider {
    id_str: &'static str,
    cli_bin: PathBuf,
}

impl CodexRuntimeProvider {
    pub fn new(id_str: &'static str) -> Self {
        Self {
            id_str,
            cli_bin: cli_bin_from_env(),
        }
    }

    #[cfg(test)]
    fn new_with_cli_bin(id_str: &'static str, cli_bin: impl Into<PathBuf>) -> Self {
        Self {
            id_str,
            cli_bin: cli_bin.into(),
        }
    }
}

fn cli_bin_from_env() -> PathBuf {
    std::env::var_os(ENV_CODEX_BIN)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CODEX_BIN))
}

impl ProviderAdapter for CodexRuntimeProvider {
    fn id(&self) -> &str {
        self.id_str
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn complete(&self, request: ProviderRequest) -> AdapterFuture<'_, ProviderResponse> {
        let cli_bin = self.cli_bin.clone();
        Box::pin(async move {
            if request.prompt.trim().is_empty() {
                return Err(AdapterError::invalid_input("prompt", "must not be empty"));
            }
            if request.max_tokens == 0 {
                return Err(AdapterError::invalid_input(
                    "max_tokens",
                    "must be greater than zero",
                ));
            }

            let client = Client::connect(ClientConfig::new().with_cli_bin(cli_bin))
                .await
                .map_err(|error| {
                    AdapterError::failed(
                        "codex_runtime.connect",
                        error.to_string(),
                        RetryClass::NonRetryable,
                    )
                })?;

            let session = client
                .start_session(
                    SessionConfig::new(DEFAULT_SESSION_CWD)
                        .with_model(request.model.clone())
                        .with_timeout(Duration::from_secs(120)),
                )
                .await
                .map_err(|error| {
                    AdapterError::failed(
                        "codex_runtime.start_session",
                        error.to_string(),
                        RetryClass::NonRetryable,
                    )
                })?;

            let result = session.ask(request.prompt).await.map_err(|error| {
                AdapterError::failed(
                    "codex_runtime.ask",
                    error.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;

            session.close().await.map_err(|error| {
                AdapterError::failed(
                    "codex_runtime.close",
                    error.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;

            client.shutdown().await.map_err(|error| {
                AdapterError::failed(
                    "codex_runtime.shutdown",
                    error.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;

            Ok(ProviderResponse {
                content: result.assistant_text,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CodexRuntimeProvider;
    use crate::contracts::{ProviderAdapter, ProviderRequest};
    use std::path::PathBuf;

    #[tokio::test]
    async fn codex_runtime_provider_rejects_empty_prompt() {
        let provider = CodexRuntimeProvider::new("codek");
        let err = provider
            .complete(ProviderRequest::new("gpt-5-codex", "", 100))
            .await
            .expect_err("empty prompt should fail");
        assert!(matches!(
            err,
            crate::error::AdapterError::InvalidInput {
                field: "prompt",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn codex_runtime_provider_rejects_zero_max_tokens() {
        let provider = CodexRuntimeProvider::new("codek");
        let err = provider
            .complete(ProviderRequest::new("gpt-5-codex", "hello", 0))
            .await
            .expect_err("zero max_tokens should fail");
        assert!(matches!(
            err,
            crate::error::AdapterError::InvalidInput {
                field: "max_tokens",
                ..
            }
        ));
    }

    #[test]
    fn codex_runtime_provider_uses_explicit_cli_bin_override() {
        let provider = CodexRuntimeProvider::new_with_cli_bin("codek", "/tmp/custom-codex");
        assert_eq!(provider.cli_bin, PathBuf::from("/tmp/custom-codex"));
    }
}
