use crate::async_http_bridge::AsyncHttpBridge;
use crate::contracts::{
    AdapterFuture, AdapterHealth, ProviderAdapter, ProviderRequest, ProviderResponse,
};
use crate::error::{AdapterError, RetryClass, classify_reqwest_error};

const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;

pub struct OpenAiCompatProvider {
    id_str: &'static str,
    api_key: String,
    base_url: String,
    http: AsyncHttpBridge,
}

fn build_http_client() -> AsyncHttpBridge {
    AsyncHttpBridge::with_timeouts(
        std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS),
        std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS),
    )
    .unwrap_or_default()
}

impl OpenAiCompatProvider {
    pub fn new(
        id_str: &'static str,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            id_str,
            api_key: api_key.into(),
            base_url: base_url.into(),
            http: build_http_client(),
        }
    }
}

impl ProviderAdapter for OpenAiCompatProvider {
    fn id(&self) -> &str {
        self.id_str
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn complete(&self, request: ProviderRequest) -> AdapterFuture<'_, ProviderResponse> {
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

            let body = serde_json::json!({
                "model": request.model,
                "messages": [{"role": "user", "content": request.prompt}],
                "max_tokens": request.max_tokens
            });

            // SECURITY: base_url may contain embedded credentials in some deployments,
            // and the Authorization header carries the API key. Use classify_reqwest_error
            // to avoid including URL or header contents in error messages.
            let auth_header = format!("Bearer {}", self.api_key);
            let response = self
                .http
                .post_json(
                    &format!("{}/chat/completions", self.base_url),
                    &[
                        ("Authorization", auth_header.as_str()),
                        ("Content-Type", "application/json"),
                    ],
                    &body,
                )
                .map_err(|e| {
                    AdapterError::failed(
                        "http_send",
                        classify_reqwest_error(&e),
                        RetryClass::Retryable,
                    )
                })?;

            if !response.status.is_success() {
                let status = response.status.as_u16();
                return Err(AdapterError::failed(
                    "http_response",
                    format!("status {status}"),
                    RetryClass::NonRetryable,
                ));
            }

            let json: serde_json::Value = serde_json::from_str(&response.body).map_err(|e| {
                AdapterError::failed("response_parse", e.to_string(), RetryClass::NonRetryable)
            })?;

            let content = json["choices"][0]["message"]["content"]
                .as_str()
                .ok_or_else(|| {
                    AdapterError::failed(
                        "response_extract",
                        "missing choices[0].message.content".to_string(),
                        RetryClass::NonRetryable,
                    )
                })?
                .to_string();

            Ok(ProviderResponse { content })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::ProviderRequest;
    use std::future::Future;

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should initialize")
            .block_on(future)
    }

    #[test]
    fn openai_compat_rejects_empty_prompt() {
        let provider = OpenAiCompatProvider::new("openai", "test-key", "https://api.openai.com/v1");
        let err = block_on(provider.complete(ProviderRequest::new("gpt-4o-mini", "", 100)))
            .expect_err("empty prompt should fail");
        assert!(matches!(
            err,
            AdapterError::InvalidInput {
                field: "prompt",
                ..
            }
        ));
    }

    #[test]
    fn openai_compat_rejects_zero_max_tokens() {
        let provider = OpenAiCompatProvider::new("openai", "test-key", "https://api.openai.com/v1");
        let err = block_on(provider.complete(ProviderRequest::new("gpt-4o-mini", "hello", 0)))
            .expect_err("zero max_tokens should fail");
        assert!(matches!(
            err,
            AdapterError::InvalidInput {
                field: "max_tokens",
                ..
            }
        ));
    }

    #[test]
    fn openai_compat_id_matches_constructor() {
        let provider =
            OpenAiCompatProvider::new("openrouter", "key", "https://openrouter.ai/api/v1");
        assert_eq!(provider.id(), "openrouter");
    }

    /// classify_reqwest_error returns &'static str — base_url and Bearer token cannot leak.
    #[test]
    fn classify_reqwest_error_returns_static_str() {
        let _: fn(&reqwest::Error) -> &'static str = classify_reqwest_error;
    }

    /// Requires OPENAI_API_KEY env var. Run with: cargo test -- --ignored
    #[test]
    #[ignore]
    fn openai_live_complete() {
        let Ok(api_key) = std::env::var("OPENAI_API_KEY") else {
            eprintln!("skipping openai_live_complete: OPENAI_API_KEY is not set");
            return;
        };
        let provider = OpenAiCompatProvider::new("openai", api_key, "https://api.openai.com/v1");
        let response =
            block_on(provider.complete(ProviderRequest::new("gpt-4o-mini", "say hello", 10)))
                .expect("live call should succeed");
        assert!(!response.content.is_empty());
    }
}
