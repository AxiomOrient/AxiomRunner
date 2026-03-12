use crate::contracts::{
    AdapterFuture, AdapterHealth, ProviderAdapter, ProviderRequest, ProviderResponse,
};
use crate::error::{AdapterError, RetryClass, classify_reqwest_error};
use std::time::Duration;

const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    api_key: String,
    base_url: String,
    http_client: reqwest::blocking::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        let http_client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            http_client,
        }
    }
}

impl ProviderAdapter for AnthropicProvider {
    fn id(&self) -> &str {
        "anthropic"
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
                "max_tokens": request.max_tokens,
                "messages": [{"role": "user", "content": request.prompt}]
            });

            let client = self.http_client.clone();
            let api_key = self.api_key.clone();
            let url = format!("{}/messages", self.base_url);

            let call_result = std::thread::spawn(move || {
                let response = client
                    .post(&url)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .header("content-type", "application/json")
                    .json(&body)
                    .send()?;
                let status = response.status().as_u16();
                let body = response.text()?;
                Ok::<(u16, String), reqwest::Error>((status, body))
            })
            .join()
            .map_err(|_| {
                AdapterError::failed("http_send", "worker thread panicked", RetryClass::Retryable)
            })?;

            let (status, response_body) = call_result.map_err(|error| {
                AdapterError::failed(
                    "http_send",
                    classify_reqwest_error(&error),
                    RetryClass::Retryable,
                )
            })?;

            if status >= 400 {
                return Err(AdapterError::failed(
                    "http_response",
                    format!("status {status}"),
                    RetryClass::NonRetryable,
                ));
            }

            let json: serde_json::Value =
                serde_json::from_str(&response_body).map_err(|error| {
                    AdapterError::failed(
                        "response_parse",
                        error.to_string(),
                        RetryClass::NonRetryable,
                    )
                })?;

            let content = json["content"]
                .as_array()
                .and_then(|parts| parts.iter().find_map(|part| part["text"].as_str()))
                .ok_or_else(|| {
                    AdapterError::failed(
                        "response_extract",
                        "missing content[].text".to_string(),
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
    use super::AnthropicProvider;
    use crate::contracts::{ProviderAdapter, ProviderRequest};
    use crate::error::AdapterError;
    use std::future::Future;

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should initialize")
            .block_on(future)
    }

    #[test]
    fn anthropic_rejects_empty_prompt() {
        let provider = AnthropicProvider::new("test-key", "https://api.anthropic.com/v1");
        let err = block_on(provider.complete(ProviderRequest::new("claude-3-5-sonnet", "", 100)))
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
    fn anthropic_rejects_zero_max_tokens() {
        let provider = AnthropicProvider::new("test-key", "https://api.anthropic.com/v1");
        let err =
            block_on(provider.complete(ProviderRequest::new("claude-3-5-sonnet", "hello", 0)))
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
    fn anthropic_id_is_stable() {
        let provider = AnthropicProvider::new("test-key", "https://api.anthropic.com/v1");
        assert_eq!(provider.id(), "anthropic");
    }
}
