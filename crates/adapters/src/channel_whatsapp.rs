use crate::async_http_bridge::AsyncHttpBridge;
use crate::channel_validate;
use crate::contracts::{
    AdapterFuture, AdapterHealth, ChannelAdapter, ChannelMessage, ChannelSendReceipt,
};
use crate::error::{AdapterError, AdapterResult, RetryClass, classify_reqwest_error};
use std::collections::VecDeque;

const MAX_WHATSAPP_BODY_BYTES: usize = 4096;
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;
/// Maximum bytes of an API error response body included in error messages.
const MAX_ERROR_BODY_PREVIEW: usize = channel_validate::DEFAULT_ERROR_BODY_PREVIEW_BYTES;

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
        client: AsyncHttpBridge,
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
    /// Returns an error when `config.phone_number_id` is missing.
    /// The `phone_number_id` is required to construct the API endpoint URL.
    pub fn live(config: WhatsAppConfig) -> Result<Self, String> {
        let phone_number_id = match &config.phone_number_id {
            Some(id) => id.clone(),
            None => {
                return Err(String::from(
                    "whatsapp live mode requires phone_number_id; use WhatsAppChannelAdapter::new for offline mode",
                ));
            }
        };

        let client = AsyncHttpBridge::with_timeouts(
            std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS),
            std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS),
        )
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

    fn send(&mut self, message: ChannelMessage) -> AdapterFuture<'_, ChannelSendReceipt> {
        Box::pin(async move {
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
                    let recipient =
                        channel_validate::resolve_route_topic(&message.topic, "whatsapp");

                    let url =
                        format!("https://graph.facebook.com/v17.0/{phone_number_id}/messages");

                    let body = serde_json::json!({
                        "messaging_product": "whatsapp",
                        "to": recipient,
                        "type": "text",
                        "text": { "body": message.body },
                    });

                    let auth_header = format!("Bearer {}", api_token.as_str());
                    let resp = client
                        .post_json(&url, &[("Authorization", auth_header.as_str())], &body)
                        .map_err(|e| {
                            AdapterError::failed(
                                "whatsapp.send",
                                format!("http {}", classify_reqwest_error(&e)),
                                RetryClass::Retryable,
                            )
                        })?;

                    let status = resp.status;

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
                        let preview = channel_validate::truncate_utf8_preview(
                            resp.body.as_str(),
                            MAX_ERROR_BODY_PREVIEW,
                        );
                        return Err(AdapterError::failed(
                            "whatsapp.send",
                            format!("HTTP {}: {preview}", status.as_u16()),
                            RetryClass::NonRetryable,
                        ));
                    }

                    if status.is_server_error() {
                        let preview = channel_validate::truncate_utf8_preview(
                            resp.body.as_str(),
                            MAX_ERROR_BODY_PREVIEW,
                        );
                        return Err(AdapterError::failed(
                            "whatsapp.send",
                            format!("HTTP {}: {preview}", status.as_u16()),
                            RetryClass::Retryable,
                        ));
                    }

                    if !status.is_success() {
                        let preview = channel_validate::truncate_utf8_preview(
                            resp.body.as_str(),
                            MAX_ERROR_BODY_PREVIEW,
                        );
                        return Err(AdapterError::failed(
                            "whatsapp.send",
                            format!("HTTP {}: {preview}", status.as_u16()),
                            RetryClass::Retryable,
                        ));
                    }

                    Ok(ChannelSendReceipt {
                        sequence: *sequence,
                        accepted: true,
                    })
                }
            }
        })
    }

    /// WhatsApp Business Cloud API does not support polling.
    /// Receiving messages requires a webhook. This method always returns empty.
    ///
    /// For the Offline transport, the queue is drained (backward compat for tests).
    /// For the Live transport, this always returns `Ok(vec![])`.
    fn drain(&mut self) -> AdapterFuture<'_, Vec<ChannelMessage>> {
        Box::pin(async move {
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
        })
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn normalize_token(raw: String, field: &'static str) -> AdapterResult<String> {
    channel_validate::normalize_token(raw, field)
}

fn normalize_optional_value(
    raw: Option<String>,
    field: &'static str,
) -> AdapterResult<Option<String>> {
    channel_validate::normalize_optional_value(raw, field)
}

fn normalize_allowed_users(users: Vec<String>, field: &'static str) -> AdapterResult<Vec<String>> {
    channel_validate::normalize_allowed_users(users, field)
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
    use std::future::Future;

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should initialize")
            .block_on(future)
    }

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
        let receipt = block_on(adapter.send(msg)).unwrap();
        assert!(receipt.accepted);
        assert_eq!(receipt.sequence, 1);
    }

    #[test]
    fn offline_send_rejects_empty_body() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("15551234567", "   ");
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_send_rejects_empty_topic() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("   ", "hello world");
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_send_rejects_oversized_body() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("15551234567", "x".repeat(MAX_WHATSAPP_BODY_BYTES + 1));
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_drain_returns_queued_messages() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        block_on(adapter.send(ChannelMessage::new("15551234567", "msg1"))).unwrap();
        block_on(adapter.send(ChannelMessage::new("15551234567", "msg2"))).unwrap();
        let drained = block_on(adapter.drain()).unwrap();
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
    fn live_without_phone_number_id_returns_error() {
        let config = make_config_no_phone_number_id();
        let error =
            WhatsAppChannelAdapter::live(config).expect_err("missing phone_number_id should fail");
        assert!(error.contains("requires phone_number_id"));
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
        let r1 = block_on(adapter.send(ChannelMessage::new("15551234567", "a"))).unwrap();
        let r2 = block_on(adapter.send(ChannelMessage::new("15551234567", "b"))).unwrap();
        assert_eq!(r1.sequence, 1);
        assert_eq!(r2.sequence, 2);
    }

    #[test]
    fn offline_drain_clears_queue() {
        let mut adapter = WhatsAppChannelAdapter::new(make_config());
        block_on(adapter.send(ChannelMessage::new("15551234567", "once"))).unwrap();
        let first = block_on(adapter.drain()).unwrap();
        assert_eq!(first.len(), 1);
        let second = block_on(adapter.drain()).unwrap();
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
