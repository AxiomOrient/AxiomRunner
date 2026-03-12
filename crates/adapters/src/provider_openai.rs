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
    http: reqwest::blocking::Client,
}

impl OpenAiCompatProvider {
    pub fn new(
        id_str: &'static str,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .timeout(std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self {
            id_str,
            api_key: api_key.into(),
            base_url: base_url.into(),
            http,
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

            let url = format!("{}/chat/completions", self.base_url);
            let response = self
                .http
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .map_err(|error| {
                    AdapterError::failed(
                        "http_send",
                        classify_reqwest_error(&error),
                        RetryClass::Retryable,
                    )
                })?;

            if !response.status().is_success() {
                return Err(AdapterError::failed(
                    "http_response",
                    format!("status {}", response.status().as_u16()),
                    RetryClass::NonRetryable,
                ));
            }

            let json: serde_json::Value = response.json().map_err(|error| {
                AdapterError::failed(
                    "response_parse",
                    error.to_string(),
                    RetryClass::NonRetryable,
                )
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
        let provider = OpenAiCompatProvider::new("openai", "key", "https://api.openai.com/v1");
        assert_eq!(provider.id(), "openai");
    }
}
