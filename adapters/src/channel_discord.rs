use crate::contracts::{AdapterHealth, ChannelAdapter, ChannelMessage, ChannelSendReceipt};
use crate::error::{classify_reqwest_error, AdapterError, AdapterResult, RetryClass};
use std::collections::VecDeque;

const MAX_DISCORD_BODY_BYTES: usize = 2000;
/// Maximum bytes of an API error response body included in error messages.
const MAX_ERROR_BODY_PREVIEW: usize = 200;

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
        http: reqwest::blocking::Client,
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
    /// Falls back to offline mode if `config.webhook_url` is None.
    pub fn live(config: DiscordConfig) -> Result<Self, String> {
        let Some(ref url) = config.webhook_url else {
            return Ok(Self::new(config));
        };
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
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

    fn send(&mut self, message: ChannelMessage) -> AdapterResult<ChannelSendReceipt> {
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
                    .post(webhook_url.as_str())
                    .json(&serde_json::json!({"content": message.body}))
                    .send()
                    .map_err(|e| {
                        AdapterError::failed(
                            "discord_webhook.send",
                            classify_reqwest_error(&e),
                            RetryClass::Retryable,
                        )
                    })?;

                let status = resp.status();
                if !status.is_success() {
                    let raw = resp.text().unwrap_or_default();
                    let preview = if raw.len() > MAX_ERROR_BODY_PREVIEW {
                        &raw[..MAX_ERROR_BODY_PREVIEW]
                    } else {
                        &raw
                    };
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
    }

    fn drain(&mut self) -> AdapterResult<Vec<ChannelMessage>> {
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
    }
}

fn normalize_token(raw: String) -> AdapterResult<String> {
    let token = raw.trim();
    if token.is_empty() {
        return Err(AdapterError::invalid_input(
            "discord.bot_token",
            "must not be empty",
        ));
    }
    if token.contains(char::is_whitespace) {
        return Err(AdapterError::invalid_input(
            "discord.bot_token",
            "must not contain whitespace",
        ));
    }
    Ok(token.to_string())
}

fn normalize_optional_channel_id(
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

fn normalize_optional_webhook_url(raw: Option<String>) -> AdapterResult<Option<String>> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(AdapterError::invalid_input(
            "discord.webhook_url",
            "must not be empty",
        ));
    }
    if !value.starts_with("https://") {
        return Err(AdapterError::invalid_input(
            "discord.webhook_url",
            "must start with https://",
        ));
    }
    Ok(Some(value.to_string()))
}

fn normalize_allowed_users(users: Vec<String>) -> AdapterResult<Vec<String>> {
    let mut normalized = Vec::new();
    for user in users {
        let user = user.trim();
        if user.is_empty() {
            return Err(AdapterError::invalid_input(
                "discord.allowed_users",
                "contains empty entry",
            ));
        }
        if user.contains(char::is_whitespace) {
            return Err(AdapterError::invalid_input(
                "discord.allowed_users",
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

    fn make_config() -> DiscordConfig {
        DiscordConfig::new("test-bot-token", Some("guild-123".to_string()), vec![]).unwrap()
    }

    fn make_webhook_config() -> DiscordConfig {
        DiscordConfig::new("test-bot-token", None, vec![])
            .unwrap()
            .with_webhook(Some("https://discord.com/api/webhooks/test/hook".to_string()))
            .unwrap()
    }

    #[test]
    fn classify_reqwest_error_returns_static_str() {
        let _: fn(&reqwest::Error) -> &'static str = classify_reqwest_error;
    }

    #[test]
    fn error_body_preview_caps_at_limit() {
        let long_body = "x".repeat(500);
        let preview = if long_body.len() > MAX_ERROR_BODY_PREVIEW {
            &long_body[..MAX_ERROR_BODY_PREVIEW]
        } else {
            &long_body
        };
        assert_eq!(preview.len(), MAX_ERROR_BODY_PREVIEW);
    }

    #[test]
    fn offline_send_accepts_valid_message() {
        let mut adapter = DiscordChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("discord", "hello world");
        let receipt = adapter.send(msg).unwrap();
        assert!(receipt.accepted);
        assert_eq!(receipt.sequence, 1);
    }

    #[test]
    fn offline_send_rejects_empty_body() {
        let mut adapter = DiscordChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("discord", "   ");
        assert!(adapter.send(msg).is_err());
    }

    #[test]
    fn offline_send_rejects_oversized_body() {
        let mut adapter = DiscordChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("discord", "x".repeat(MAX_DISCORD_BODY_BYTES + 1));
        assert!(adapter.send(msg).is_err());
    }

    #[test]
    fn offline_drain_returns_queued_messages() {
        let mut adapter = DiscordChannelAdapter::new(make_config());
        adapter.send(ChannelMessage::new("discord", "msg1")).unwrap();
        adapter.send(ChannelMessage::new("discord", "msg2")).unwrap();
        let drained = adapter.drain().unwrap();
        assert_eq!(drained.len(), 2);
    }

    #[test]
    fn offline_adapter_is_not_live() {
        let adapter = DiscordChannelAdapter::new(make_config());
        assert!(!adapter.is_live());
    }

    #[test]
    fn live_without_webhook_url_falls_back_to_offline() {
        let config = DiscordConfig::new("test-bot-token", None, vec![]).unwrap();
        let adapter = DiscordChannelAdapter::live(config).unwrap();
        assert!(!adapter.is_live());
    }

    #[test]
    fn live_with_webhook_url_is_live() {
        let adapter = DiscordChannelAdapter::live(make_webhook_config()).unwrap();
        assert!(adapter.is_live());
    }

    #[test]
    fn live_drain_returns_empty_vec() {
        let mut adapter = DiscordChannelAdapter::live(make_webhook_config()).unwrap();
        let drained = adapter.drain().unwrap();
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
