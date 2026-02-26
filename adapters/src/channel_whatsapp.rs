use crate::contracts::{AdapterHealth, ChannelAdapter, ChannelMessage, ChannelSendReceipt};
use crate::error::{classify_reqwest_error, AdapterError, AdapterResult, RetryClass};
use std::collections::VecDeque;

const MAX_WHATSAPP_BODY_BYTES: usize = 4096;
/// Maximum bytes of an API error response body included in error messages.
const MAX_ERROR_BODY_PREVIEW: usize = 200;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq)]
pub struct WhatsAppConfig {
    pub api_token: String,
    pub phone_number_id: Option<String>,
    pub business_account_id: Option<String>,
    pub allowed_users: Vec<String>,
}

impl std::fmt::Debug for WhatsAppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhatsAppConfig")
            .field("api_token", &"[redacted]")
            .field("phone_number_id", &self.phone_number_id)
            .field("business_account_id", &self.business_account_id)
            .field("allowed_users", &self.allowed_users)
            .finish()
    }
}

impl WhatsAppConfig {
    pub fn new(
        api_token: impl Into<String>,
        phone_number_id: Option<String>,
        business_account_id: Option<String>,
        allowed_users: Vec<String>,
    ) -> AdapterResult<Self> {
        let api_token = normalize_token(api_token.into(), "whatsapp.api_token")?;
        let phone_number_id =
            normalize_optional_value(phone_number_id, "whatsapp.phone_number_id")?;
        let business_account_id =
            normalize_optional_value(business_account_id, "whatsapp.business_account_id")?;
        let allowed_users = normalize_allowed_users(allowed_users, "whatsapp.allowed_users")?;

        Ok(Self {
            api_token,
            phone_number_id,
            business_account_id,
            allowed_users,
        })
    }

    fn health(&self) -> AdapterHealth {
        if self.api_token.contains("invalid") {
            return AdapterHealth::Unavailable;
        }

        if self.phone_number_id.is_none() || self.business_account_id.is_none() {
            AdapterHealth::Degraded
        } else {
            AdapterHealth::Healthy
        }
    }
}

// ---------------------------------------------------------------------------
// Transport enum
// ---------------------------------------------------------------------------

enum WhatsAppTransport {
    Offline {
        queue: VecDeque<ChannelMessage>,
        sequence: u64,
    },
    Live {
        client: reqwest::blocking::Client,
        api_token: String,
        phone_number_id: String,
        sequence: u64,
    },
}

// ---------------------------------------------------------------------------
// Adapter struct
// ---------------------------------------------------------------------------

pub struct WhatsAppChannelAdapter {
    config: WhatsAppConfig,
    transport: WhatsAppTransport,
}

impl std::fmt::Debug for WhatsAppChannelAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match &self.transport {
            WhatsAppTransport::Offline { .. } => "offline",
            WhatsAppTransport::Live { .. } => "live",
        };
        f.debug_struct("WhatsAppChannelAdapter")
            .field("transport", &mode)
            .finish()
    }
}

impl WhatsAppChannelAdapter {
    /// Create an offline adapter backed by an in-process queue. Safe for tests.
    pub fn new(config: WhatsAppConfig) -> Self {
        Self {
            transport: WhatsAppTransport::Offline {
                queue: VecDeque::new(),
                sequence: 0,
            },
            config,
        }
    }

    /// Create a live adapter that calls the Meta WhatsApp Business Cloud API.
    ///
    /// Falls back to offline mode if `config.phone_number_id` is None.
    /// The `phone_number_id` is required to construct the API endpoint URL.
    pub fn live(config: WhatsAppConfig) -> Result<Self, String> {
        let phone_number_id = match &config.phone_number_id {
            Some(id) => id.clone(),
            None => return Ok(Self::new(config)),
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("whatsapp http client init failed: {e}"))?;

        Ok(Self {
            transport: WhatsAppTransport::Live {
                client,
                api_token: config.api_token.clone(),
                phone_number_id,
                sequence: 0,
            },
            config,
        })
    }

    pub fn config(&self) -> &WhatsAppConfig {
        &self.config
    }

    /// Returns true when this adapter holds an active HTTP client targeting the real API.
    pub fn is_live(&self) -> bool {
        matches!(self.transport, WhatsAppTransport::Live { .. })
    }
}

// ---------------------------------------------------------------------------
// ChannelAdapter impl
// ---------------------------------------------------------------------------

impl ChannelAdapter for WhatsAppChannelAdapter {
    fn id(&self) -> &str {
        "channel.whatsapp"
    }

    fn health(&self) -> AdapterHealth {
        self.config.health()
    }

    fn send(&mut self, message: ChannelMessage) -> AdapterResult<ChannelSendReceipt> {
        validate_message(&message)?;

        match &mut self.transport {
            WhatsAppTransport::Offline { queue, sequence } => {
                *sequence = sequence.saturating_add(1);
                queue.push_back(message);
                Ok(ChannelSendReceipt {
                    sequence: *sequence,
                    accepted: true,
                })
            }
            WhatsAppTransport::Live {
                client,
                api_token,
                phone_number_id,
                sequence,
            } => {
                *sequence = sequence.saturating_add(1);

                let url = format!(
                    "https://graph.facebook.com/v17.0/{phone_number_id}/messages"
                );

                let body = serde_json::json!({
                    "messaging_product": "whatsapp",
                    "to": message.topic,
                    "type": "text",
                    "text": { "body": message.body },
                });

                let resp = client
                    .post(&url)
                    .bearer_auth(api_token.as_str())
                    .json(&body)
                    .send()
                    .map_err(|e| {
                        AdapterError::failed(
                            "whatsapp.send",
                            format!("http {}: {}", classify_reqwest_error(&e), e.without_url()),
                            RetryClass::Retryable,
                        )
                    })?;

                let status = resp.status();

                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    return Err(AdapterError::failed(
                        "whatsapp.send",
                        "HTTP 429: rate limited by Meta API",
                        RetryClass::Retryable,
                    ));
                }

                if status == reqwest::StatusCode::UNAUTHORIZED
                    || status == reqwest::StatusCode::FORBIDDEN
                    || status == reqwest::StatusCode::BAD_REQUEST
                {
                    let preview = resp
                        .text()
                        .unwrap_or_default()
                        .chars()
                        .take(MAX_ERROR_BODY_PREVIEW)
                        .collect::<String>();
                    return Err(AdapterError::failed(
                        "whatsapp.send",
                        format!("HTTP {}: {}", status.as_u16(), preview),
                        RetryClass::NonRetryable,
                    ));
                }

                if status.is_server_error() {
                    let preview = resp
                        .text()
                        .unwrap_or_default()
                        .chars()
                        .take(MAX_ERROR_BODY_PREVIEW)
                        .collect::<String>();
                    return Err(AdapterError::failed(
                        "whatsapp.send",
                        format!("HTTP {}: {}", status.as_u16(), preview),
                        RetryClass::Retryable,
                    ));
                }

                if !status.is_success() {
                    let preview = resp
                        .text()
                        .unwrap_or_default()
                        .chars()
                        .take(MAX_ERROR_BODY_PREVIEW)
                        .collect::<String>();
                    return Err(AdapterError::failed(
                        "whatsapp.send",
                        format!("HTTP {}: {}", status.as_u16(), preview),
                        RetryClass::Retryable,
                    ));
                }

                Ok(ChannelSendReceipt {
                    sequence: *sequence,
                    accepted: true,
                })
            }
        }
    }

    /// WhatsApp Business Cloud API does not support polling.
    /// Receiving messages requires a webhook. This method always returns empty.
    ///
    /// For the Offline transport, the queue is drained (backward compat for tests).
    /// For the Live transport, this always returns `Ok(vec![])`.
    fn drain(&mut self) -> AdapterResult<Vec<ChannelMessage>> {
        match &mut self.transport {
            WhatsAppTransport::Offline { queue, .. } => {
                let mut drained = Vec::with_capacity(queue.len());
                while let Some(msg) = queue.pop_front() {
                    drained.push(msg);
                }
                Ok(drained)
            }
            WhatsAppTransport::Live { .. } => {
                // WhatsApp has no polling API; receiving is webhook-only.
                Ok(vec![])
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn normalize_token(raw: String, field: &'static str) -> AdapterResult<String> {
    let token = raw.trim();
    if token.is_empty() {
        return Err(AdapterError::invalid_input(field, "must not be empty"));
    }
    if token.contains(char::is_whitespace) {
        return Err(AdapterError::invalid_input(
            field,
            "must not contain whitespace",
        ));
    }

    Ok(token.to_string())
}

fn normalize_optional_value(
    raw: Option<String>,
    field: &'static str,
) -> AdapterResult<Option<String>> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(AdapterError::invalid_input(field, "must not be empty"));
    }
    if value.contains(char::is_whitespace) {
        return Err(AdapterError::invalid_input(
            field,
            "must not contain whitespace",
        ));
    }

    Ok(Some(value.to_string()))
}

fn normalize_allowed_users(users: Vec<String>, field: &'static str) -> AdapterResult<Vec<String>> {
    let mut normalized = Vec::new();
    for user in users {
        let user = user.trim();
        if user.is_empty() {
            return Err(AdapterError::invalid_input(field, "contains empty entry"));
        }
        if user.contains(char::is_whitespace) {
            return Err(AdapterError::invalid_input(
                field,
                "entries must not contain whitespace",
            ));
        }
        normalized.push(user.to_string());
    }
    normalized.sort_unstable();
    normalized.dedup();
    Ok(normalized)
}

fn validate_message(message: &ChannelMessage) -> AdapterResult<()> {
    if message.topic.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "whatsapp.message.topic",
            "must not be empty",
        ));
    }
    if message.body.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "whatsapp.message.body",
            "must not be empty",
        ));
    }
    if message.body.len() > MAX_WHATSAPP_BODY_BYTES {
        return Err(AdapterError::invalid_input(
            "whatsapp.message.body",
            "must not exceed 4096 bytes",
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests (all offline — no real Meta API calls)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::ChannelMessage;

    fn make_config() -> WhatsAppConfig {
        WhatsAppConfig::new(
            "EAAValidToken123",
            Some("123456789012345".to_string()),
            Some("987654321098765".to_string()),
            vec![],
        )
        .unwrap()
    }

    fn make_config_no_phone_number_id() -> WhatsAppConfig {
        WhatsAppConfig::new(
            "EAAValidToken123",
            None,
            Some("987654321098765".to_string()),
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn offline_send_accepts_valid_message() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("15551234567", "hello world");
        let receipt = adapter.send(msg).unwrap();
        assert!(receipt.accepted);
        assert_eq!(receipt.sequence, 1);
    }

    #[test]
    fn offline_send_rejects_empty_body() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("15551234567", "   ");
        assert!(adapter.send(msg).is_err());
    }

    #[test]
    fn offline_send_rejects_empty_topic() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("   ", "hello world");
        assert!(adapter.send(msg).is_err());
    }

    #[test]
    fn offline_send_rejects_oversized_body() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("15551234567", "x".repeat(MAX_WHATSAPP_BODY_BYTES + 1));
        assert!(adapter.send(msg).is_err());
    }

    #[test]
    fn offline_drain_returns_queued_messages() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        adapter
            .send(ChannelMessage::new("15551234567", "msg1"))
            .unwrap();
        adapter
            .send(ChannelMessage::new("15551234567", "msg2"))
            .unwrap();
        let drained = adapter.drain().unwrap();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].body, "msg1");
        assert_eq!(drained[1].body, "msg2");
    }

    #[test]
    fn offline_adapter_is_not_live() {
        let adapter = WhatsAppChannelAdapter::new(make_config());
        assert!(!adapter.is_live());
    }

    #[test]
    fn live_without_phone_number_id_falls_back_to_offline() {
        let config = make_config_no_phone_number_id();
        let adapter = WhatsAppChannelAdapter::live(config).unwrap();
        assert!(!adapter.is_live());
    }

    #[test]
    fn debug_does_not_expose_api_token() {
        let adapter = WhatsAppChannelAdapter::new(make_config());
        let debug_str = format!("{adapter:?}");
        assert!(debug_str.contains("offline"));
        assert!(!debug_str.contains("EAAValidToken123"));
    }

    #[test]
    fn health_is_healthy_when_phone_and_business_set() {
        let config = make_config();
        assert_eq!(config.health(), AdapterHealth::Healthy);
    }

    #[test]
    fn health_is_degraded_when_phone_number_id_missing() {
        let config = make_config_no_phone_number_id();
        assert_eq!(config.health(), AdapterHealth::Degraded);
    }

    #[test]
    fn sequence_increments_per_send() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        let r1 = adapter
            .send(ChannelMessage::new("15551234567", "a"))
            .unwrap();
        let r2 = adapter
            .send(ChannelMessage::new("15551234567", "b"))
            .unwrap();
        assert_eq!(r1.sequence, 1);
        assert_eq!(r2.sequence, 2);
    }

    #[test]
    fn offline_drain_clears_queue() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        adapter
            .send(ChannelMessage::new("15551234567", "once"))
            .unwrap();
        let first = adapter.drain().unwrap();
        assert_eq!(first.len(), 1);
        let second = adapter.drain().unwrap();
        assert!(second.is_empty());
    }

    #[test]
    fn config_debug_redacts_token() {
        let config = make_config();
        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("[redacted]"));
        assert!(!debug_str.contains("EAAValidToken123"));
    }

    #[test]
    fn live_with_phone_number_id_is_live() {
        let config = make_config();
        let adapter = WhatsAppChannelAdapter::live(config).unwrap();
        assert!(adapter.is_live());
    }
}
