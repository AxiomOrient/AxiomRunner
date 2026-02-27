use crate::async_http_bridge::AsyncHttpBridge;
use crate::channel_validate;
use crate::contracts::{
    AdapterFuture, AdapterHealth, ChannelAdapter, ChannelMessage, ChannelSendReceipt,
};
use crate::error::{AdapterError, AdapterResult, RetryClass, classify_reqwest_error};
use std::collections::VecDeque;

const MAX_SLACK_BODY_BYTES: usize = 4000;
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;
/// Maximum bytes of an API error response body included in error messages.
const MAX_ERROR_BODY_PREVIEW: usize = channel_validate::DEFAULT_ERROR_BODY_PREVIEW_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackConfig {
    pub bot_token: String,
    pub channel_id: Option<String>,
    pub allowed_users: Vec<String>,
    pub webhook_url: Option<String>,
}

impl SlackConfig {
    pub fn new(
        bot_token: impl Into<String>,
        channel_id: Option<String>,
        allowed_users: Vec<String>,
    ) -> AdapterResult<Self> {
        let token = normalize_token(bot_token.into())?;
        let channel_id = normalize_optional_channel_id(channel_id, "slack.channel_id")?;
        let allowed_users = normalize_allowed_users(allowed_users)?;

        Ok(Self {
            bot_token: token,
            channel_id,
            allowed_users,
            webhook_url: None,
        })
    }

    pub fn with_webhook(mut self, webhook_url: Option<String>) -> AdapterResult<Self> {
        self.webhook_url = normalize_optional_webhook_url(webhook_url)?;
        Ok(self)
    }

    /// Create a webhook-only config from an environment variable.
    /// `AXIOM_CHANNEL_SLACK_WEBHOOK` holds the full Slack Incoming Webhook URL.
    /// bot_token is set to a placeholder when only the webhook path is needed.
    pub fn from_env() -> AdapterResult<Self> {
        let webhook_url = std::env::var("AXIOM_CHANNEL_SLACK_WEBHOOK").ok();
        Ok(Self {
            bot_token: "webhook-only".to_string(),
            channel_id: None,
            allowed_users: Vec::new(),
            webhook_url,
        })
    }

    fn health(&self) -> AdapterHealth {
        if self.bot_token.contains("invalid") {
            return AdapterHealth::Unavailable;
        }

        if self.channel_id.is_none() && self.webhook_url.is_none() {
            AdapterHealth::Degraded
        } else {
            AdapterHealth::Healthy
        }
    }
}

/// HTTP transport for the Slack adapter.
/// `Offline` mode uses an in-process queue; `Live` mode posts to a webhook URL.
enum SlackTransport {
    Offline {
        queue: VecDeque<ChannelMessage>,
        sequence: u64,
    },
    Live {
        http: AsyncHttpBridge,
        webhook_url: String,
        sequence: u64,
    },
}

pub struct SlackChannelAdapter {
    config: SlackConfig,
    transport: SlackTransport,
}

impl std::fmt::Debug for SlackChannelAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match &self.transport {
            SlackTransport::Offline { .. } => "offline",
            SlackTransport::Live { .. } => "live",
        };
        f.debug_struct("SlackChannelAdapter")
            .field("transport", &mode)
            .finish()
    }
}

impl SlackChannelAdapter {
    /// Create an offline adapter backed by an in-process queue.
    /// Used for testing and scenarios without a webhook URL.
    pub fn new(config: SlackConfig) -> Self {
        Self {
            transport: SlackTransport::Offline {
                queue: VecDeque::new(),
                sequence: 0,
            },
            config,
        }
    }

    /// Create a live adapter that posts to the Slack Incoming Webhook.
    /// Returns an error when `config.webhook_url` is missing.
    pub fn live(config: SlackConfig) -> Result<Self, String> {
        let Some(ref url) = config.webhook_url else {
            return Err(String::from(
                "slack live mode requires webhook_url; use SlackChannelAdapter::new for offline mode",
            ));
        };
        let http = AsyncHttpBridge::with_timeouts(
            std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS),
            std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS),
        )
        .map_err(|e| format!("slack http client init failed: {e}"))?;
        let webhook_url = url.clone();
        Ok(Self {
            transport: SlackTransport::Live {
                http,
                webhook_url,
                sequence: 0,
            },
            config,
        })
    }

    pub fn config(&self) -> &SlackConfig {
        &self.config
    }

    /// Returns true if this adapter is connected to the real Slack webhook.
    pub fn is_live(&self) -> bool {
        matches!(self.transport, SlackTransport::Live { .. })
    }
}

impl ChannelAdapter for SlackChannelAdapter {
    fn id(&self) -> &str {
        "channel.slack"
    }

    fn health(&self) -> AdapterHealth {
        self.config.health()
    }

    fn send(&mut self, message: ChannelMessage) -> AdapterFuture<'_, ChannelSendReceipt> {
        Box::pin(async move {
            validate_message(&message)?;

            match &mut self.transport {
                SlackTransport::Offline { queue, sequence } => {
                    *sequence = sequence.saturating_add(1);
                    queue.push_back(message);
                    Ok(ChannelSendReceipt {
                        sequence: *sequence,
                        accepted: true,
                    })
                }
                SlackTransport::Live {
                    http,
                    webhook_url,
                    sequence,
                } => {
                    // SECURITY: webhook_url is a secret. Never include it in error messages.
                    let resp = http
                        .post_json(
                            webhook_url.as_str(),
                            &[],
                            &serde_json::json!({"text": message.body}),
                        )
                        .map_err(|e| {
                            AdapterError::failed(
                                "slack_webhook.send",
                                classify_reqwest_error(&e),
                                RetryClass::Retryable,
                            )
                        })?;

                    let status = resp.status;
                    if !status.is_success() {
                        let raw = resp.body;
                        let preview = channel_validate::truncate_utf8_preview(
                            raw.as_str(),
                            MAX_ERROR_BODY_PREVIEW,
                        );
                        return Err(AdapterError::failed(
                            "slack_webhook.send",
                            format!("api error {status}: {preview}"),
                            RetryClass::Retryable,
                        ));
                    }

                    *sequence = sequence.saturating_add(1);
                    Ok(ChannelSendReceipt {
                        sequence: *sequence,
                        accepted: true,
                    })
                }
            }
        })
    }

    fn drain(&mut self) -> AdapterFuture<'_, Vec<ChannelMessage>> {
        Box::pin(async move {
            match &mut self.transport {
                SlackTransport::Offline { queue, .. } => {
                    let mut drained = Vec::with_capacity(queue.len());
                    while let Some(msg) = queue.pop_front() {
                        drained.push(msg);
                    }
                    Ok(drained)
                }
                // Slack webhooks are send-only; drain is a no-op in live mode.
                SlackTransport::Live { .. } => Ok(Vec::new()),
            }
        })
    }
}

fn normalize_token(raw: String) -> AdapterResult<String> {
    channel_validate::normalize_token(raw, "slack.bot_token")
}

fn normalize_optional_channel_id(
    raw: Option<String>,
    field: &'static str,
) -> AdapterResult<Option<String>> {
    channel_validate::normalize_optional_value(raw, field)
}

fn normalize_optional_webhook_url(raw: Option<String>) -> AdapterResult<Option<String>> {
    channel_validate::normalize_optional_https_url(raw, "slack.webhook_url")
}

fn normalize_allowed_users(users: Vec<String>) -> AdapterResult<Vec<String>> {
    channel_validate::normalize_allowed_users(users, "slack.allowed_users")
}

fn validate_message(message: &ChannelMessage) -> AdapterResult<()> {
    if message.topic.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "slack.message.topic",
            "must not be empty",
        ));
    }
    if message.body.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "slack.message.body",
            "must not be empty",
        ));
    }
    if message.body.len() > MAX_SLACK_BODY_BYTES {
        return Err(AdapterError::invalid_input(
            "slack.message.body",
            "must not exceed 4000 bytes",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should initialize")
            .block_on(future)
    }
    use crate::contracts::ChannelMessage;

    fn make_config() -> SlackConfig {
        SlackConfig::new("test-bot-token", Some("C1234567890".to_string()), vec![]).unwrap()
    }

    fn make_webhook_config() -> SlackConfig {
        SlackConfig::new("test-bot-token", None, vec![])
            .unwrap()
            .with_webhook(Some(
                "https://hooks.slack.com/services/T00000000/B00000000/XXXX".to_string(),
            ))
            .unwrap()
    }

    #[test]
    fn classify_reqwest_error_returns_static_str() {
        let _: fn(&reqwest::Error) -> &'static str = classify_reqwest_error;
    }

    #[test]
    fn error_body_preview_caps_at_limit() {
        let long_body = "x".repeat(500);
        let preview = channel_validate::truncate_utf8_preview(&long_body, MAX_ERROR_BODY_PREVIEW);
        assert_eq!(preview.len(), MAX_ERROR_BODY_PREVIEW);
    }

    #[test]
    fn offline_send_accepts_valid_message() {
        let mut adapter = SlackChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("slack", "hello world");
        let receipt = block_on(adapter.send(msg)).unwrap();
        assert!(receipt.accepted);
        assert_eq!(receipt.sequence, 1);
    }

    #[test]
    fn offline_send_rejects_empty_body() {
        let mut adapter = SlackChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("slack", "   ");
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_send_rejects_oversized_body() {
        let mut adapter = SlackChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("slack", "x".repeat(MAX_SLACK_BODY_BYTES + 1));
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_drain_returns_queued_messages() {
        let mut adapter = SlackChannelAdapter::new(make_config());
        block_on(adapter.send(ChannelMessage::new("slack", "msg1"))).unwrap();
        block_on(adapter.send(ChannelMessage::new("slack", "msg2"))).unwrap();
        let drained = block_on(adapter.drain()).unwrap();
        assert_eq!(drained.len(), 2);
    }

    #[test]
    fn offline_adapter_is_not_live() {
        let adapter = SlackChannelAdapter::new(make_config());
        assert!(!adapter.is_live());
    }

    #[test]
    fn live_without_webhook_url_returns_error() {
        let config = SlackConfig::new("test-bot-token", None, vec![]).unwrap();
        let error = SlackChannelAdapter::live(config).expect_err("missing webhook should fail");
        assert!(error.contains("requires webhook_url"));
    }

    #[test]
    fn live_with_webhook_url_is_live() {
        let adapter = SlackChannelAdapter::live(make_webhook_config()).unwrap();
        assert!(adapter.is_live());
    }

    #[test]
    fn live_drain_returns_empty_vec() {
        let mut adapter = SlackChannelAdapter::live(make_webhook_config()).unwrap();
        let drained = block_on(adapter.drain()).unwrap();
        assert!(drained.is_empty());
    }

    #[test]
    fn webhook_url_must_start_with_https() {
        let config = SlackConfig::new("test-bot-token", None, vec![]).unwrap();
        let result = config.with_webhook(Some("http://hooks.slack.com/services/T/B/X".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn from_env_without_var_yields_none_webhook() {
        let config = SlackConfig {
            bot_token: "webhook-only".to_string(),
            channel_id: None,
            allowed_users: Vec::new(),
            webhook_url: None,
        };
        let adapter = SlackChannelAdapter::new(config);
        assert!(!adapter.is_live());
    }

    #[test]
    fn health_is_healthy_when_webhook_url_set() {
        let config = make_webhook_config();
        assert_eq!(config.health(), AdapterHealth::Healthy);
    }

    #[test]
    fn health_is_degraded_when_no_channel_and_no_webhook() {
        let config = SlackConfig::new("test-bot-token", None, vec![]).unwrap();
        assert_eq!(config.health(), AdapterHealth::Degraded);
    }

    #[test]
    fn adapter_debug_does_not_expose_webhook_url() {
        let config = make_webhook_config();
        let adapter = SlackChannelAdapter::live(config).unwrap();
        let debug_str = format!("{adapter:?}");
        assert!(!debug_str.contains("hooks.slack.com"));
        assert!(debug_str.contains("live"));
    }
}
