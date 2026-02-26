use crate::contracts::{AdapterHealth, ToolAdapter, ToolCall, ToolOutput};
use crate::error::{AdapterError, AdapterResult, RetryClass};

/// Maximum bytes of a Composio error response body included in error messages.
/// Limits exposure in case the server echoes back request headers containing the API key.
const MAX_ERROR_BODY_PREVIEW: usize = 200;

/// Classify a reqwest transport error without exposing the URL or request headers.
fn classify_reqwest_error(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connection failed"
    } else if e.is_status() {
        "unexpected status"
    } else {
        "request failed"
    }
}

/// Composio tool adapter — executes named actions via the Composio API.
/// Reads COMPOSIO_API_KEY from the environment at construction time.
pub struct ComposioToolAdapter {
    api_key: String,
    http: reqwest::blocking::Client,
}

impl ComposioToolAdapter {
    const BASE_URL: &'static str = "https://backend.composio.dev/api/v1/actions";

    /// Construct from the `COMPOSIO_API_KEY` environment variable.
    pub fn new() -> Result<Self, String> {
        let api_key = std::env::var("COMPOSIO_API_KEY")
            .map_err(|_| "COMPOSIO_API_KEY env var not set".to_string())?;
        Self::with_key(api_key)
    }

    /// Construct with an explicit API key (useful for tests and injection).
    pub fn with_key(api_key: impl Into<String>) -> Result<Self, String> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err("COMPOSIO_API_KEY must not be empty".to_string());
        }
        let http = reqwest::blocking::Client::new();
        Ok(Self { api_key, http })
    }
}

impl ToolAdapter for ComposioToolAdapter {
    fn id(&self) -> &str {
        "composio"
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn execute(&self, call: ToolCall) -> AdapterResult<ToolOutput> {
        // Validate tool name: only [A-Za-z0-9_] permitted to prevent path traversal injection.
        if !call
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(AdapterError::invalid_input(
                "tool_name",
                "must contain only alphanumeric characters and underscores",
            ));
        }

        let url = format!("{}/{}/execute", Self::BASE_URL, call.name);

        // Build body: { "params": { key: value, ... } }
        let params: serde_json::Map<String, serde_json::Value> = call
            .args
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        let body = serde_json::json!({ "params": params });

        let resp = self
            .http
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .json(&body)
            .send()
            .map_err(|e| {
                // SECURITY: Do not include URL or error details — request headers
                // carry the API key and may be reflected in reqwest error strings.
                AdapterError::failed(
                    "composio_send",
                    classify_reqwest_error(&e),
                    RetryClass::Retryable,
                )
            })?;

        let status = resp.status();
        let text = resp.text().map_err(|e| {
            AdapterError::failed(
                "composio_read",
                classify_reqwest_error(&e),
                RetryClass::Retryable,
            )
        })?;

        if !status.is_success() {
            // SECURITY: Truncate body to prevent API key reflection from leaking
            // if Composio echoes request headers in error responses.
            // Use char-boundary-safe slicing to avoid panic on multibyte UTF-8.
            let preview = if text.len() > MAX_ERROR_BODY_PREVIEW {
                let end = (0..=MAX_ERROR_BODY_PREVIEW)
                    .rev()
                    .find(|&i| text.is_char_boundary(i))
                    .unwrap_or(0);
                &text[..end]
            } else {
                text.as_str()
            };
            return Err(AdapterError::failed(
                "composio_execute",
                format!("HTTP {status}: {preview}"),
                RetryClass::NonRetryable,
            ));
        }

        Ok(ToolOutput { content: text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn missing_api_key_returns_err() {
        // new() reads the env var; we test with_key() directly to avoid
        // mutating process environment (set_var/remove_var are unsafe in Rust 2024).
        assert!(ComposioToolAdapter::with_key("").is_err());
    }

    #[test]
    fn present_api_key_constructs_ok() {
        assert!(ComposioToolAdapter::with_key("test-key-123").is_ok());
    }

    #[test]
    fn id_returns_composio() {
        let adapter = ComposioToolAdapter::with_key("key").unwrap();
        assert_eq!(adapter.id(), "composio");
    }

    #[test]
    fn health_returns_healthy() {
        let adapter = ComposioToolAdapter::with_key("key").unwrap();
        assert_eq!(adapter.health(), AdapterHealth::Healthy);
    }

    /// classify_reqwest_error returns a &'static str — no URL or API key can leak.
    #[test]
    fn classify_reqwest_error_returns_static_str() {
        let _: fn(&reqwest::Error) -> &'static str = classify_reqwest_error;
    }

    #[test]
    fn error_body_preview_caps_at_limit() {
        let long_body = "x".repeat(500);
        let preview = if long_body.len() > MAX_ERROR_BODY_PREVIEW {
            let end = (0..=MAX_ERROR_BODY_PREVIEW)
                .rev()
                .find(|&i| long_body.is_char_boundary(i))
                .unwrap_or(0);
            &long_body[..end]
        } else {
            long_body.as_str()
        };
        assert_eq!(preview.len(), MAX_ERROR_BODY_PREVIEW);
    }

    #[test]
    fn error_body_preview_safe_on_multibyte_utf8() {
        // "é" is 2 bytes (0xC3 0xA9). Construct a string where MAX_ERROR_BODY_PREVIEW
        // falls inside a multibyte character to verify no panic occurs.
        let mut body = "a".repeat(MAX_ERROR_BODY_PREVIEW - 1);
        body.push('é'); // byte offset MAX_ERROR_BODY_PREVIEW is inside this char
        body.push_str(&"b".repeat(100));
        let preview = if body.len() > MAX_ERROR_BODY_PREVIEW {
            let end = (0..=MAX_ERROR_BODY_PREVIEW)
                .rev()
                .find(|&i| body.is_char_boundary(i))
                .unwrap_or(0);
            &body[..end]
        } else {
            body.as_str()
        };
        // end must retreat to MAX_ERROR_BODY_PREVIEW - 1 (the char boundary before 'é')
        assert_eq!(preview.len(), MAX_ERROR_BODY_PREVIEW - 1);
    }

    #[test]
    fn execute_rejects_invalid_tool_name() {
        let adapter = ComposioToolAdapter::with_key("key").unwrap();
        let call = ToolCall {
            name: "../secret".to_string(),
            args: BTreeMap::new(),
        };
        let err = adapter.execute(call).unwrap_err();
        assert_eq!(err.kind(), crate::error::AdapterErrorKind::InvalidInput);
    }

    /// Real network call — only run with a valid key and connection.
    #[test]
    #[ignore]
    fn live_execute_action() {
        let adapter =
            ComposioToolAdapter::with_key("REPLACE_WITH_REAL_KEY").unwrap();
        let call = ToolCall {
            name: "GITHUB_LIST_REPOS".to_string(),
            args: BTreeMap::new(),
        };
        let output = adapter.execute(call).unwrap();
        assert!(!output.content.is_empty());
    }
}
