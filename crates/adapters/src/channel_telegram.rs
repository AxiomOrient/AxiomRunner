use crate::async_http_bridge::AsyncHttpBridge;
use crate::channel_validate;
use crate::contracts::{
    AdapterFuture, AdapterHealth, ChannelAdapter, ChannelMessage, ChannelSendReceipt,
};
use crate::error::{AdapterError, AdapterResult, RetryClass, classify_reqwest_error};
use std::collections::VecDeque;

const MAX_TELEGRAM_BODY_BYTES: usize = 4096;
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;
/// Maximum bytes of an API error response body included in error messages.
/// Caps exposure of any sensitive data that the remote may echo back.
const MAX_ERROR_BODY_PREVIEW: usize = channel_validate::DEFAULT_ERROR_BODY_PREVIEW_BYTES;

#[derive(Clone, PartialEq, Eq)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub allowed_users: Vec<String>,
}

impl std::fmt::Debug for TelegramConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelegramConfig")
            .field("bot_token", &"[redacted]")
            .field("allowed_users", &self.allowed_users)
            .finish()
    }
}

impl TelegramConfig {
    pub fn new(bot_token: impl Into<String>, allowed_users: Vec<String>) -> AdapterResult<Self> {
        let token = normalize_token(bot_token.into())?;
        let allowed_users = normalize_allowed_users(allowed_users)?;

        Ok(Self {
            bot_token: token,
            allowed_users,
        })
    }

    fn health(&self) -> AdapterHealth {
        if self.bot_token.contains("invalid") {
            return AdapterHealth::Unavailable;
        }

        if self.allowed_users.is_empty() {
            AdapterHealth::Unavailable
        } else {
            AdapterHealth::Healthy
        }
    }
}

/// HTTP transport for the Telegram adapter.
/// `Offline` mode uses an in-process queue; `Live` mode calls the Telegram Bot API.
enum TelegramTransport {
    Offline {
        queue: VecDeque<ChannelMessage>,
        sequence: u64,
    },
    Live {
        http: AsyncHttpBridge,
        next_offset: i64,
    },
}

pub struct TelegramChannelAdapter {
    config: TelegramConfig,
    transport: TelegramTransport,
    /// Path to persist next_offset across restarts.
    /// Set from AXIOM_RUNTIME_TOOL_WORKSPACE env var during live construction.
    /// None in offline/test mode.
    offset_path: Option<std::path::PathBuf>,
}

impl std::fmt::Debug for TelegramChannelAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match &self.transport {
            TelegramTransport::Offline { .. } => "offline",
            TelegramTransport::Live { .. } => "live",
        };
        f.debug_struct("TelegramChannelAdapter")
            .field("config", &self.config)
            .field("transport", &mode)
            .finish()
    }
}

impl TelegramChannelAdapter {
    /// Create an offline adapter backed by an in-process queue.
    /// Used for testing and scenarios without real Telegram credentials.
    pub fn new(config: TelegramConfig) -> Self {
        Self {
            config,
            transport: TelegramTransport::Offline {
                queue: VecDeque::new(),
                sequence: 0,
            },
            offset_path: None,
        }
    }

    /// Create a live adapter that calls the Telegram Bot API.
    /// Panics if called within a tokio runtime (use spawn_blocking to avoid this).
    ///
    /// If `AXIOM_RUNTIME_TOOL_WORKSPACE` is set, next_offset is persisted to
    /// `{workspace}/telegram_offset_{channel_name}.txt` and restored on restart.
    pub fn live(config: TelegramConfig) -> Result<Self, String> {
        let http = AsyncHttpBridge::with_timeouts(
            std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS),
            std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS),
        )
        .map_err(|e| format!("telegram http client init failed: {e}"))?;

        // Resolve offset persistence path from workspace env var.
        let offset_path = std::env::var("AXIOM_RUNTIME_TOOL_WORKSPACE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|workspace| {
                std::path::PathBuf::from(workspace).join("telegram_offset_channel.telegram.txt")
            });

        // Restore next_offset from file if available.
        let next_offset = Self::load_offset(offset_path.as_deref());

        Ok(Self {
            config,
            transport: TelegramTransport::Live { http, next_offset },
            offset_path,
        })
    }

    /// Read persisted offset from path; returns 0 if file is absent or unparseable.
    fn load_offset(path: Option<&std::path::Path>) -> i64 {
        path.and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(0)
    }

    /// Test-only constructor: live transport with an explicit offset_path.
    /// Avoids unsafe env-var mutation required by `set_var`/`remove_var` in Rust 1.81+.
    #[cfg(test)]
    fn live_with_offset_path(
        config: TelegramConfig,
        offset_path: Option<std::path::PathBuf>,
    ) -> Result<Self, String> {
        let http = AsyncHttpBridge::with_timeouts(
            std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS),
            std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS),
        )
        .map_err(|e| format!("telegram http client init failed: {e}"))?;
        let next_offset = Self::load_offset(offset_path.as_deref());
        Ok(Self {
            config,
            transport: TelegramTransport::Live { http, next_offset },
            offset_path,
        })
    }

    pub fn config(&self) -> &TelegramConfig {
        &self.config
    }

    /// Returns true if this adapter is connected to the real Telegram API.
    pub fn is_live(&self) -> bool {
        matches!(self.transport, TelegramTransport::Live { .. })
    }
}

impl ChannelAdapter for TelegramChannelAdapter {
    fn id(&self) -> &str {
        "channel.telegram"
    }

    fn health(&self) -> AdapterHealth {
        self.config.health()
    }

    fn send(&mut self, message: ChannelMessage) -> AdapterFuture<'_, ChannelSendReceipt> {
        Box::pin(async move {
            validate_message(&message)?;

            match &mut self.transport {
                TelegramTransport::Offline { queue, sequence } => {
                    *sequence = sequence.saturating_add(1);
                    queue.push_back(message);
                    Ok(ChannelSendReceipt {
                        sequence: *sequence,
                        accepted: true,
                    })
                }
                TelegramTransport::Live { http, .. } => {
                    // Use topic as chat_id (set by drain() from message.chat.id).
                    // Fall back to allowed_users[0] only when topic is absent or non-numeric.
                    let chat_id = {
                        let topic =
                            channel_validate::resolve_route_topic(&message.topic, "telegram");
                        if !topic.is_empty() && topic.parse::<i64>().is_ok() {
                            topic
                        } else {
                            self.config
                                .allowed_users
                                .first()
                                .ok_or_else(|| {
                                    AdapterError::invalid_input(
                                        "telegram.chat_id",
                                        "no allowed users configured",
                                    )
                                })?
                                .clone()
                        }
                    };

                    // SECURITY: URL contains bot_token. Never include this URL in error messages.
                    let url = format!(
                        "https://api.telegram.org/bot{}/sendMessage",
                        self.config.bot_token
                    );

                    let resp = http
                        .post_json(
                            &url,
                            &[],
                            &serde_json::json!({
                                "chat_id": chat_id,
                                "text": message.body,
                            }),
                        )
                        .map_err(|e| {
                            AdapterError::failed(
                                "telegram.send",
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
                            "telegram.send",
                            format!("api error {status}: {preview}"),
                            RetryClass::Retryable,
                        ));
                    }

                    let body: serde_json::Value =
                        serde_json::from_str(&resp.body).map_err(|e| {
                            AdapterError::failed(
                                "telegram.send.parse",
                                e.to_string(),
                                RetryClass::NonRetryable,
                            )
                        })?;

                    let message_id = body["result"]["message_id"].as_u64().unwrap_or(0);
                    Ok(ChannelSendReceipt {
                        sequence: message_id,
                        accepted: true,
                    })
                }
            }
        })
    }

    fn drain(&mut self) -> AdapterFuture<'_, Vec<ChannelMessage>> {
        Box::pin(async move {
            match &mut self.transport {
                TelegramTransport::Offline { queue, .. } => {
                    let mut drained = Vec::with_capacity(queue.len());
                    while let Some(msg) = queue.pop_front() {
                        drained.push(msg);
                    }
                    Ok(drained)
                }
                TelegramTransport::Live { http, next_offset } => {
                    // SECURITY: URL contains bot_token. Never include this URL in error messages.
                    let url = format!(
                        "https://api.telegram.org/bot{}/getUpdates",
                        self.config.bot_token
                    );

                    let resp = http
                        .post_json(
                            &url,
                            &[],
                            &serde_json::json!({
                                "offset": *next_offset,
                                "limit": 100,
                                "timeout": 1,
                            }),
                        )
                        .map_err(|e| {
                            AdapterError::failed(
                                "telegram.drain",
                                classify_reqwest_error(&e),
                                RetryClass::Retryable,
                            )
                        })?;

                    let drain_status = resp.status;
                    if !drain_status.is_success() {
                        let raw = resp.body;
                        let preview = channel_validate::truncate_utf8_preview(
                            raw.as_str(),
                            MAX_ERROR_BODY_PREVIEW,
                        );
                        return Err(AdapterError::failed(
                            "telegram.drain",
                            format!("api error {drain_status}: {preview}"),
                            RetryClass::Retryable,
                        ));
                    }

                    let body: serde_json::Value =
                        serde_json::from_str(&resp.body).map_err(|e| {
                            AdapterError::failed(
                                "telegram.drain.parse",
                                e.to_string(),
                                RetryClass::NonRetryable,
                            )
                        })?;

                    let mut messages = Vec::new();
                    if let Some(updates) = body["result"].as_array() {
                        for update in updates {
                            let Some(update_id) = update["update_id"].as_i64() else {
                                continue;
                            };
                            *next_offset = update_id + 1;

                            // Persist offset to file; ignore write failures to avoid
                            // masking the drain result.
                            if let Some(ref path) = self.offset_path
                                && let Err(e) = std::fs::write(path, next_offset.to_string())
                            {
                                eprintln!("warn: telegram offset save failed: {e}");
                            }

                            if let Some(text) = update["message"]["text"].as_str() {
                                let from_id = update["message"]["from"]["id"]
                                    .as_i64()
                                    .map(|id| id.to_string())
                                    .unwrap_or_default();

                                if !self.config.allowed_users.contains(&from_id) {
                                    continue;
                                }

                                // Extract chat_id from message.chat.id; skip if absent.
                                let chat_id = match update["message"]["chat"]["id"].as_i64() {
                                    Some(id) => id.to_string(),
                                    None => continue,
                                };

                                messages.push(ChannelMessage::new(
                                    channel_validate::encode_routed_topic("telegram", &chat_id),
                                    text,
                                ));
                            }
                        }
                    }

                    Ok(messages)
                }
            }
        })
    }
}

fn normalize_token(raw: String) -> AdapterResult<String> {
    channel_validate::normalize_token(raw, "telegram.bot_token")
}

fn normalize_allowed_users(users: Vec<String>) -> AdapterResult<Vec<String>> {
    channel_validate::normalize_allowed_users(users, "telegram.allowed_users")
}

fn validate_message(message: &ChannelMessage) -> AdapterResult<()> {
    if message.topic.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "telegram.message.topic",
            "must not be empty",
        ));
    }
    if message.body.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "telegram.message.body",
            "must not be empty",
        ));
    }
    if message.body.len() > MAX_TELEGRAM_BODY_BYTES {
        return Err(AdapterError::invalid_input(
            "telegram.message.body",
            "must not exceed 4096 bytes",
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

    fn make_config() -> TelegramConfig {
        TelegramConfig::new("test-token-123", vec!["12345".to_string()]).unwrap()
    }

    /// classify_reqwest_error returns a static string — no URL or token present.
    /// This is a compile-time structural check; reqwest::Error cannot be
    /// constructed directly in tests so we verify the return type is &'static str.
    #[test]
    fn classify_reqwest_error_returns_static_str() {
        // The function signature guarantees &'static str — no owned allocation
        // that could carry URL / token data.
        let _: fn(&reqwest::Error) -> &'static str = classify_reqwest_error;
    }

    #[test]
    fn error_body_preview_caps_at_limit() {
        let long_body = "x".repeat(500);
        let preview = channel_validate::truncate_utf8_preview(&long_body, MAX_ERROR_BODY_PREVIEW);
        assert_eq!(preview.len(), MAX_ERROR_BODY_PREVIEW);
    }

    #[test]
    fn error_body_preview_passes_short_body() {
        let short = "error: bad request";
        let preview = channel_validate::truncate_utf8_preview(short, MAX_ERROR_BODY_PREVIEW);
        assert_eq!(preview, short);
    }

    #[test]
    fn offline_send_accepts_valid_message() {
        let mut adapter = TelegramChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("telegram", "hello world");
        let receipt = block_on(adapter.send(msg)).unwrap();
        assert!(receipt.accepted);
        assert_eq!(receipt.sequence, 1);
    }

    #[test]
    fn offline_send_rejects_empty_body() {
        let mut adapter = TelegramChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("telegram", "   ");
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_drain_returns_queued_messages() {
        let mut adapter = TelegramChannelAdapter::new(make_config());
        block_on(adapter.send(ChannelMessage::new("telegram", "msg1"))).unwrap();
        block_on(adapter.send(ChannelMessage::new("telegram", "msg2"))).unwrap();
        let drained = block_on(adapter.drain()).unwrap();
        assert_eq!(drained.len(), 2);
    }

    #[test]
    fn bot_token_does_not_appear_in_adapter_debug() {
        let config = TelegramConfig::new("super-secret-bot-token", vec!["99".to_string()]).unwrap();
        let adapter = TelegramChannelAdapter::new(config);
        let debug_str = format!("{adapter:?}");
        assert!(debug_str.contains("offline"));
        assert!(!debug_str.contains("super-secret-bot-token"));
    }

    #[test]
    fn config_debug_redacts_bot_token() {
        let config = TelegramConfig::new("super-secret-bot-token", vec!["99".to_string()]).unwrap();
        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("[redacted]"));
        assert!(!debug_str.contains("super-secret-bot-token"));
    }

    #[test]
    fn offline_adapter_has_no_offset_path() {
        let adapter = TelegramChannelAdapter::new(make_config());
        assert!(adapter.offset_path.is_none());
    }

    #[test]
    fn live_without_offset_path_has_none() {
        // Use explicit None path — avoids unsafe env mutation.
        let adapter = TelegramChannelAdapter::live_with_offset_path(make_config(), None).unwrap();
        assert!(adapter.offset_path.is_none());
    }

    #[test]
    fn live_with_explicit_offset_path_sets_field() {
        let dir = std::env::temp_dir();
        let path = dir.join("tg_test_sets_field_offset.txt");
        let _ = std::fs::remove_file(&path);
        let adapter =
            TelegramChannelAdapter::live_with_offset_path(make_config(), Some(path.clone()))
                .unwrap();
        assert!(adapter.offset_path.is_some());
        assert_eq!(adapter.offset_path.unwrap(), path);
    }

    #[test]
    fn live_restores_offset_from_file() {
        let dir = std::env::temp_dir();
        let offset_file = dir.join("tg_test_restore_offset.txt");
        std::fs::write(&offset_file, "42").unwrap();
        let adapter =
            TelegramChannelAdapter::live_with_offset_path(make_config(), Some(offset_file.clone()))
                .unwrap();
        let _ = std::fs::remove_file(&offset_file);
        match &adapter.transport {
            TelegramTransport::Live { next_offset, .. } => {
                assert_eq!(*next_offset, 42);
            }
            _ => panic!("expected Live transport"),
        }
    }

    #[test]
    fn live_defaults_to_zero_when_no_offset_file() {
        let dir = std::env::temp_dir();
        let offset_file = dir.join("tg_test_zero_default_offset.txt");
        let _ = std::fs::remove_file(&offset_file);
        let adapter =
            TelegramChannelAdapter::live_with_offset_path(make_config(), Some(offset_file))
                .unwrap();
        match &adapter.transport {
            TelegramTransport::Live { next_offset, .. } => {
                assert_eq!(*next_offset, 0);
            }
            _ => panic!("expected Live transport"),
        }
    }
}
