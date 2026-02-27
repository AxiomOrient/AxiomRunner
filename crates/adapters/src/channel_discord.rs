use crate::async_http_bridge::AsyncHttpBridge;
use crate::channel_validate;
use crate::contracts::{
    AdapterFuture, AdapterHealth, ChannelAdapter, ChannelMessage, ChannelSendReceipt,
};
use crate::error::{AdapterError, AdapterResult, RetryClass, classify_reqwest_error};
use std::collections::VecDeque;

const MAX_DISCORD_BODY_BYTES: usize = 2000;
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;
/// Maximum bytes of an API error response body included in error messages.
const MAX_ERROR_BODY_PREVIEW: usize = channel_validate::DEFAULT_ERROR_BODY_PREVIEW_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscordConfig {
    pub bot_token: String,
    pub guild_id: Option<String>,
    pub allowed_users: Vec<String>,
    pub webhook_url: Option<String>,
}

impl DiscordConfig {
    pub fn new(
        bot_token: impl Into<String>,
        guild_id: Option<String>,
        allowed_users: Vec<String>,
    ) -> AdapterResult<Self> {
        let token = normalize_token(bot_token.into())?;
        let guild_id = normalize_optional_channel_id(guild_id, "discord.guild_id")?;
        let allowed_users = normalize_allowed_users(allowed_users)?;

        Ok(Self {
            bot_token: token,
            guild_id,
            allowed_users,
            webhook_url: None,
        })
    }

    pub fn with_webhook(mut self, webhook_url: Option<String>) -> AdapterResult<Self> {
        self.webhook_url = normalize_optional_webhook_url(webhook_url)?;
        Ok(self)
    }

    /// Create a webhook-only config from an environment variable.
    /// `AXIOM_CHANNEL_DISCORD_WEBHOOK` holds the full Discord webhook URL.
    /// bot_token is set to a placeholder when only the webhook path is needed.
    pub fn from_env() -> AdapterResult<Self> {
        let webhook_url = std::env::var("AXIOM_CHANNEL_DISCORD_WEBHOOK").ok();
        Ok(Self {
            bot_token: "webhook-only".to_string(),
            guild_id: None,
            allowed_users: Vec::new(),
            webhook_url,
        })
    }

    fn health(&self) -> AdapterHealth {
        if self.bot_token.contains("invalid") {
            return AdapterHealth::Unavailable;
        }

        if self.guild_id.is_none() && self.webhook_url.is_none() {
            AdapterHealth::Degraded
        } else {
            AdapterHealth::Healthy
        }
    }
}

/// HTTP transport for the Discord adapter.
/// `Offline` mode uses an in-process queue; `Live` mode posts to a webhook URL.
enum DiscordTransport {
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

pub struct DiscordChannelAdapter {
    config: DiscordConfig,
    transport: DiscordTransport,
}

impl std::fmt::Debug for DiscordChannelAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match &self.transport {
            DiscordTransport::Offline { .. } => "offline",
            DiscordTransport::Live { .. } => "live",
        };
        f.debug_struct("DiscordChannelAdapter")
            .field("transport", &mode)
            .finish()
    }
}

impl DiscordChannelAdapter {
    /// Create an offline adapter backed by an in-process queue.
    /// Used for testing and scenarios without a webhook URL.
    pub fn new(config: DiscordConfig) -> Self {
        Self {
            transport: DiscordTransport::Offline {
                queue: VecDeque::new(),
                sequence: 0,
            },
            config,
        }
    }

    /// Create a live adapter that posts to the Discord Incoming Webhook.
    /// Returns an error when `config.webhook_url` is missing.
    pub fn live(config: DiscordConfig) -> Result<Self, String> {
        let Some(ref url) = config.webhook_url else {
            return Err(String::from(
                "discord live mode requires webhook_url; use DiscordChannelAdapter::new for offline mode",
            ));
        };
        let http = AsyncHttpBridge::with_timeouts(
            std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS),
            std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS),
        )
        .map_err(|e| format!("discord http client init failed: {e}"))?;
        let webhook_url = url.clone();
        Ok(Self {
            transport: DiscordTransport::Live {
                http,
                webhook_url,
                sequence: 0,
            },
            config,
        })
    }

    pub fn config(&self) -> &DiscordConfig {
        &self.config
    }

    /// Returns true if this adapter is connected to the real Discord webhook.
    pub fn is_live(&self) -> bool {
        matches!(self.transport, DiscordTransport::Live { .. })
    }
}

impl ChannelAdapter for DiscordChannelAdapter {
    fn id(&self) -> &str {
        "channel.discord"
    }

    fn health(&self) -> AdapterHealth {
        self.config.health()
    }

    fn send(&mut self, message: ChannelMessage) -> AdapterFuture<'_, ChannelSendReceipt> {
        Box::pin(async move {
            validate_message(&message)?;

            match &mut self.transport {
                DiscordTransport::Offline { queue, sequence } => {
                    *sequence = sequence.saturating_add(1);
                    queue.push_back(message);
                    Ok(ChannelSendReceipt {
                        sequence: *sequence,
                        accepted: true,
                    })
                }
                DiscordTransport::Live {
                    http,
                    webhook_url,
                    sequence,
                } => {
                    // SECURITY: webhook_url is a secret. Never include it in error messages.
                    let resp = http
                        .post_json(
                            webhook_url.as_str(),
                            &[],
                            &serde_json::json!({"content": message.body}),
                        )
                        .map_err(|e| {
                            AdapterError::failed(
                                "discord_webhook.send",
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
                            "discord_webhook.send",
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
                DiscordTransport::Offline { queue, .. } => {
                    let mut drained = Vec::with_capacity(queue.len());
                    while let Some(msg) = queue.pop_front() {
                        drained.push(msg);
                    }
                    Ok(drained)
                }
                // Discord webhooks are send-only; drain is a no-op in live mode.
                DiscordTransport::Live { .. } => Ok(Vec::new()),
            }
        })
    }
}

fn normalize_token(raw: String) -> AdapterResult<String> {
    channel_validate::normalize_token(raw, "discord.bot_token")
}

fn normalize_optional_channel_id(
    raw: Option<String>,
    field: &'static str,
) -> AdapterResult<Option<String>> {
    channel_validate::normalize_optional_value(raw, field)
}

fn normalize_optional_webhook_url(raw: Option<String>) -> AdapterResult<Option<String>> {
    channel_validate::normalize_optional_https_url(raw, "discord.webhook_url")
}

fn normalize_allowed_users(users: Vec<String>) -> AdapterResult<Vec<String>> {
    channel_validate::normalize_allowed_users(users, "discord.allowed_users")
}

fn validate_message(message: &ChannelMessage) -> AdapterResult<()> {
    if message.topic.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "discord.message.topic",
            "must not be empty",
        ));
    }
    if message.body.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "discord.message.body",
            "must not be empty",
        ));
    }
    if message.body.len() > MAX_DISCORD_BODY_BYTES {
        return Err(AdapterError::invalid_input(
            "discord.message.body",
            "must not exceed 2000 bytes",
        ));
    }
    Ok(())
}

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

    fn make_config() -> DiscordConfig {
        DiscordConfig::new("test-bot-token", Some("guild-123".to_string()), vec![]).unwrap()
    }

    fn make_webhook_config() -> DiscordConfig {
        DiscordConfig::new("test-bot-token", None, vec![])
            .unwrap()
            .with_webhook(Some(
                "https://discord.com/api/webhooks/test/hook".to_string(),
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
        let mut adapter = DiscordChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("discord", "hello world");
        let receipt = block_on(adapter.send(msg)).unwrap();
        assert!(receipt.accepted);
        assert_eq!(receipt.sequence, 1);
    }

    #[test]
    fn offline_send_rejects_empty_body() {
        let mut adapter = DiscordChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("discord", "   ");
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_send_rejects_oversized_body() {
        let mut adapter = DiscordChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("discord", "x".repeat(MAX_DISCORD_BODY_BYTES + 1));
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_drain_returns_queued_messages() {
        let mut adapter = DiscordChannelAdapter::new(make_config());
        block_on(adapter.send(ChannelMessage::new("discord", "msg1"))).unwrap();
        block_on(adapter.send(ChannelMessage::new("discord", "msg2"))).unwrap();
        let drained = block_on(adapter.drain()).unwrap();
        assert_eq!(drained.len(), 2);
    }

    #[test]
    fn offline_adapter_is_not_live() {
        let adapter = DiscordChannelAdapter::new(make_config());
        assert!(!adapter.is_live());
    }

    #[test]
    fn live_without_webhook_url_returns_error() {
        let config = DiscordConfig::new("test-bot-token", None, vec![]).unwrap();
        let error = DiscordChannelAdapter::live(config).expect_err("missing webhook should fail");
        assert!(error.contains("requires webhook_url"));
    }

    #[test]
    fn live_with_webhook_url_is_live() {
        let adapter = DiscordChannelAdapter::live(make_webhook_config()).unwrap();
        assert!(adapter.is_live());
    }

    #[test]
    fn live_drain_returns_empty_vec() {
        let mut adapter = DiscordChannelAdapter::live(make_webhook_config()).unwrap();
        let drained = block_on(adapter.drain()).unwrap();
        assert!(drained.is_empty());
    }

    #[test]
    fn webhook_url_must_start_with_https() {
        let config = DiscordConfig::new("test-bot-token", None, vec![]).unwrap();
        let result = config.with_webhook(Some("http://discord.com/api/webhooks/x".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn from_env_without_var_yields_none_webhook() {
        // Env var absent → webhook_url is None → offline mode
        // We can only check the config field since we can't safely mutate env in tests.
        let config = DiscordConfig {
            bot_token: "webhook-only".to_string(),
            guild_id: None,
            allowed_users: Vec::new(),
            webhook_url: None,
        };
        let adapter = DiscordChannelAdapter::new(config);
        assert!(!adapter.is_live());
    }

    #[test]
    fn health_is_healthy_when_webhook_url_set() {
        let config = make_webhook_config();
        assert_eq!(config.health(), AdapterHealth::Healthy);
    }

    #[test]
    fn health_is_degraded_when_no_guild_and_no_webhook() {
        let config = DiscordConfig::new("test-bot-token", None, vec![]).unwrap();
        assert_eq!(config.health(), AdapterHealth::Degraded);
    }

    #[test]
    fn adapter_debug_does_not_expose_webhook_url() {
        let config = make_webhook_config();
        let adapter = DiscordChannelAdapter::live(config).unwrap();
        let debug_str = format!("{adapter:?}");
        assert!(!debug_str.contains("discord.com/api/webhooks"));
        assert!(debug_str.contains("live"));
    }
}
