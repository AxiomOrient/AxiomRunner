use crate::contracts::{
    AdapterFuture, ProviderAdapter, ProviderHealthReport, ProviderRequest, ProviderResponse,
};
use crate::error::{AdapterError, RetryClass, classify_reqwest_error};

pub const ENV_EXPERIMENTAL_OPENAI: &str = "AXONRUNNER_EXPERIMENTAL_OPENAI";
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;

pub struct OpenAiCompatProvider {
    id_str: &'static str,
    api_key: Option<String>,
    base_url: String,
    experimental_enabled: bool,
    http: reqwest::blocking::Client,
}

impl OpenAiCompatProvider {
    pub fn new(id_str: &'static str, api_key: Option<String>, base_url: impl Into<String>) -> Self {
        Self::new_with_experimental_enabled(
            id_str,
            api_key,
            base_url,
            experimental_openai_enabled(),
        )
    }

    fn new_with_experimental_enabled(
        id_str: &'static str,
        api_key: Option<String>,
        base_url: impl Into<String>,
        experimental_enabled: bool,
    ) -> Self {
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .timeout(std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self {
            id_str,
            api_key,
            base_url: base_url.into(),
            experimental_enabled,
            http,
        }
    }
}

impl ProviderAdapter for OpenAiCompatProvider {
    fn id(&self) -> &str {
        self.id_str
    }

    fn health(&self) -> AdapterFuture<'_, ProviderHealthReport> {
        if !self.experimental_enabled {
            return Box::pin(async {
                Ok(ProviderHealthReport::blocked(
                    "reason=experimental_provider_disabled,provider=openai",
                ))
            });
        }
        let has_api_key = self
            .api_key
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        let detail = if has_api_key {
            format!(
                "provider=openai,mode=experimental,base_url={}",
                self.base_url
            )
        } else {
            String::from("reason=missing_openai_api_key,provider=openai,mode=experimental")
        };
        Box::pin(async move {
            Ok(if has_api_key {
                ProviderHealthReport::ready(detail)
            } else {
                ProviderHealthReport::blocked(detail)
            })
        })
    }

    fn complete(&self, request: ProviderRequest) -> AdapterFuture<'_, ProviderResponse> {
        Box::pin(async move {
            if !self.experimental_enabled {
                return Err(AdapterError::unavailable_with_class(
                    "openai_experimental",
                    "AXONRUNNER_EXPERIMENTAL_OPENAI=1 required",
                    RetryClass::NonRetryable,
                ));
            }
            if request.prompt.trim().is_empty() {
                return Err(AdapterError::invalid_input("prompt", "must not be empty"));
            }
            if request.max_tokens == 0 {
                return Err(AdapterError::invalid_input(
                    "max_tokens",
                    "must be greater than zero",
                ));
            }
            let api_key = self.api_key.as_deref().ok_or_else(|| {
                AdapterError::unavailable_with_class(
                    "openai_api_key",
                    "OPENAI_API_KEY not set",
                    RetryClass::NonRetryable,
                )
            })?;

            let body = serde_json::json!({
                "model": request.model,
                "messages": [{"role": "user", "content": request.prompt}],
                "max_tokens": request.max_tokens
            });

            let url = format!("{}/chat/completions", self.base_url);
            let response = self
                .http
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
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

            let content = extract_message_content(&json)?;

            Ok(ProviderResponse { content })
        })
    }
}

fn experimental_openai_enabled() -> bool {
    matches!(
        std::env::var(ENV_EXPERIMENTAL_OPENAI).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn extract_message_content(json: &serde_json::Value) -> Result<String, AdapterError> {
    if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
        return Ok(content.to_owned());
    }

    if let Some(parts) = json["choices"][0]["message"]["content"].as_array() {
        let text = parts
            .iter()
            .filter_map(
                |part| match part.get("type").and_then(serde_json::Value::as_str) {
                    Some("text") => part.get("text").and_then(serde_json::Value::as_str),
                    _ => None,
                },
            )
            .collect::<String>();
        if !text.is_empty() {
            return Ok(text);
        }
    }

    Err(AdapterError::failed(
        "response_extract",
        "missing choices[0].message.content".to_string(),
        RetryClass::NonRetryable,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::ProviderRequest;
    use crate::test_util::block_on;

    #[test]
    fn openai_compat_rejects_empty_prompt() {
        let provider = OpenAiCompatProvider::new_with_experimental_enabled(
            "openai",
            Some(String::from("test-key")),
            "https://api.openai.com/v1",
            true,
        );
        let err = block_on(provider.complete(ProviderRequest::new("gpt-4o-mini", "", 100, "/tmp")))
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
        let provider = OpenAiCompatProvider::new_with_experimental_enabled(
            "openai",
            Some(String::from("test-key")),
            "https://api.openai.com/v1",
            true,
        );
        let err =
            block_on(provider.complete(ProviderRequest::new("gpt-4o-mini", "hello", 0, "/tmp")))
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
        let provider = OpenAiCompatProvider::new(
            "openai",
            Some(String::from("key")),
            "https://api.openai.com/v1",
        );
        assert_eq!(provider.id(), "openai");
    }

    #[test]
    fn openai_compat_health_is_blocked_without_api_key() {
        let provider = OpenAiCompatProvider::new("openai", None, "https://api.openai.com/v1");
        let report = block_on(provider.health()).expect("health probe should succeed");
        assert_eq!(report.status.as_str(), "blocked");
        assert_eq!(
            report.detail,
            "reason=experimental_provider_disabled,provider=openai"
        );
    }
}
