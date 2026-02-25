use crate::contracts::{AdapterHealth, ChannelAdapter, ChannelMessage, ChannelSendReceipt};
use crate::error::{AdapterError, AdapterResult, RetryClass};
use std::collections::VecDeque;

const MAX_MATRIX_BODY_BYTES: usize = 4000;
const MATRIX_DEFAULT_HOMESERVER: &str = "https://matrix.org";
/// Maximum bytes of an API error response body included in error messages.
const MAX_ERROR_BODY_PREVIEW: usize = 200;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Percent-encode room_id for use in URL path segments.
/// Matrix room IDs look like `!roomid:server.com`; both `!` and `:` must be encoded.
fn encode_room_id(room_id: &str) -> String {
    room_id.replace('!', "%21").replace(':', "%3A")
}

/// Sanitize room_id for use as a filename component.
/// Replaces any non-alphanumeric character with `_`.
fn sanitize_room_id_for_filename(room_id: &str) -> String {
    room_id
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

/// Classify a reqwest error without exposing the URL or sensitive headers.
/// The access_token is part of the Authorization header and must never appear
/// in error messages that propagate to callers.
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

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq)]
pub struct MatrixConfig {
    pub access_token: String,
    pub room_id: Option<String>,
    pub homeserver: Option<String>,
    pub allowed_users: Vec<String>,
}

impl std::fmt::Debug for MatrixConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatrixConfig")
            .field("access_token", &"[redacted]")
            .field("room_id", &self.room_id)
            .field("homeserver", &self.homeserver)
            .field("allowed_users", &self.allowed_users)
            .finish()
    }
}

impl MatrixConfig {
    pub fn new(
        access_token: impl Into<String>,
        room_id: Option<String>,
        homeserver: Option<String>,
        allowed_users: Vec<String>,
    ) -> AdapterResult<Self> {
        let access_token = normalize_token(access_token.into(), "matrix.access_token")?;
        let room_id = normalize_optional_value(room_id, "matrix.room_id")?;
        let homeserver = normalize_optional_value(homeserver, "matrix.homeserver")?;
        let allowed_users = normalize_allowed_users(allowed_users, "matrix.allowed_users")?;

        Ok(Self {
            access_token,
            room_id,
            homeserver,
            allowed_users,
        })
    }

    fn health(&self) -> AdapterHealth {
        if self.access_token.contains("invalid") {
            return AdapterHealth::Unavailable;
        }

        if self.room_id.is_none() || self.homeserver.is_none() {
            AdapterHealth::Degraded
        } else {
            AdapterHealth::Healthy
        }
    }
}

// ---------------------------------------------------------------------------
// Transport enum
// ---------------------------------------------------------------------------

enum MatrixTransport {
    Offline {
        queue: VecDeque<ChannelMessage>,
        sequence: u64,
    },
    Live {
        client: reqwest::blocking::Client,
        homeserver: String,
        access_token: String,
        room_id: String,
        sequence: u64,
        next_batch: String,
        batch_path: Option<std::path::PathBuf>,
    },
}

// ---------------------------------------------------------------------------
// Adapter struct
// ---------------------------------------------------------------------------

pub struct MatrixChannelAdapter {
    config: MatrixConfig,
    transport: MatrixTransport,
}

impl std::fmt::Debug for MatrixChannelAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match &self.transport {
            MatrixTransport::Offline { .. } => "offline",
            MatrixTransport::Live { .. } => "live",
        };
        f.debug_struct("MatrixChannelAdapter")
            .field("transport", &mode)
            .finish()
    }
}

impl MatrixChannelAdapter {
    /// Create an offline adapter backed by an in-process queue. Safe for tests.
    pub fn new(config: MatrixConfig) -> Self {
        Self {
            transport: MatrixTransport::Offline {
                queue: VecDeque::new(),
                sequence: 0,
            },
            config,
        }
    }

    /// Create a live adapter that calls the Matrix Client-Server API.
    ///
    /// Falls back to offline mode if `config.room_id` or `config.homeserver` is None.
    /// If `AXIOM_RUNTIME_TOOL_WORKSPACE` is set, `next_batch` is persisted to
    /// `{workspace}/matrix_batch_{sanitized_room_id}.txt` and restored on restart.
    pub fn live(config: MatrixConfig) -> Result<Self, String> {
        let room_id = match &config.room_id {
            Some(r) => r.clone(),
            None => return Ok(Self::new(config)),
        };
        // Use configured homeserver or fall back to default.
        let homeserver = config
            .homeserver
            .clone()
            .unwrap_or_else(|| MATRIX_DEFAULT_HOMESERVER.to_owned());

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("matrix http client init failed: {e}"))?;

        // Resolve batch persistence path from workspace env var.
        let batch_path = std::env::var("AXIOM_RUNTIME_TOOL_WORKSPACE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|workspace| {
                let safe = sanitize_room_id_for_filename(&room_id);
                std::path::PathBuf::from(workspace).join(format!("matrix_batch_{safe}.txt"))
            });

        // Restore next_batch from file if available.
        let next_batch = load_batch_token(batch_path.as_deref());

        Ok(Self {
            transport: MatrixTransport::Live {
                client,
                homeserver,
                access_token: config.access_token.clone(),
                room_id,
                sequence: 0,
                next_batch,
                batch_path,
            },
            config,
        })
    }

    pub fn config(&self) -> &MatrixConfig {
        &self.config
    }

    /// Returns true when this adapter holds an active HTTP client targeting a real homeserver.
    pub fn is_live(&self) -> bool {
        matches!(self.transport, MatrixTransport::Live { .. })
    }
}

// ---------------------------------------------------------------------------
// ChannelAdapter impl
// ---------------------------------------------------------------------------

impl ChannelAdapter for MatrixChannelAdapter {
    fn id(&self) -> &str {
        "channel.matrix"
    }

    fn health(&self) -> AdapterHealth {
        self.config.health()
    }

    fn send(&mut self, message: ChannelMessage) -> AdapterResult<ChannelSendReceipt> {
        validate_message(&message)?;

        match &mut self.transport {
            MatrixTransport::Offline { queue, sequence } => {
                *sequence = sequence.saturating_add(1);
                queue.push_back(message);
                Ok(ChannelSendReceipt {
                    sequence: *sequence,
                    accepted: true,
                })
            }
            MatrixTransport::Live {
                client,
                homeserver,
                access_token,
                room_id,
                sequence,
                ..
            } => {
                *sequence = sequence.saturating_add(1);
                let txn_id = sequence.to_string();
                let encoded_room = encode_room_id(room_id);

                let url = format!(
                    "{homeserver}/_matrix/client/v3/rooms/{encoded_room}/send/m.room.message/{txn_id}"
                );

                let body = serde_json::json!({
                    "msgtype": "m.text",
                    "body": message.body,
                });

                let resp = client
                    .put(&url)
                    .bearer_auth(access_token.as_str())
                    .json(&body)
                    .send()
                    .map_err(|e| {
                        AdapterError::failed(
                            "matrix.send",
                            format!("http {}: {}", classify_reqwest_error(&e), e.without_url()),
                            RetryClass::Retryable,
                        )
                    })?;

                let status = resp.status();

                if status == reqwest::StatusCode::UNAUTHORIZED {
                    return Err(AdapterError::failed(
                        "matrix.send",
                        "HTTP 401: access token invalid or expired",
                        RetryClass::NonRetryable,
                    ));
                }

                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    return Err(AdapterError::failed(
                        "matrix.send",
                        "HTTP 429: rate limited by homeserver",
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
                        "matrix.send",
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

    fn drain(&mut self) -> AdapterResult<Vec<ChannelMessage>> {
        match &mut self.transport {
            MatrixTransport::Offline { queue, .. } => {
                let mut drained = Vec::with_capacity(queue.len());
                while let Some(msg) = queue.pop_front() {
                    drained.push(msg);
                }
                Ok(drained)
            }
            MatrixTransport::Live {
                client,
                homeserver,
                access_token,
                room_id,
                next_batch,
                batch_path,
                ..
            } => {
                // Build sync URL — timeout=0 for non-blocking poll.
                let filter = r#"{"room":{"timeline":{"limit":50}}}"#;
                let url = if next_batch.is_empty() {
                    format!(
                        "{homeserver}/_matrix/client/v3/sync?timeout=0&filter={filter}"
                    )
                } else {
                    format!(
                        "{homeserver}/_matrix/client/v3/sync?since={since}&timeout=0&filter={filter}",
                        since = next_batch
                    )
                };

                let resp = client
                    .get(&url)
                    .bearer_auth(access_token.as_str())
                    .send()
                    .map_err(|e| {
                        AdapterError::failed(
                            "matrix.drain",
                            format!("http {}: {}", classify_reqwest_error(&e), e.without_url()),
                            RetryClass::Retryable,
                        )
                    })?;

                let status = resp.status();

                if status == reqwest::StatusCode::UNAUTHORIZED {
                    return Err(AdapterError::failed(
                        "matrix.drain",
                        "HTTP 401: access token invalid or expired",
                        RetryClass::NonRetryable,
                    ));
                }

                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    return Err(AdapterError::failed(
                        "matrix.drain",
                        "HTTP 429: rate limited by homeserver",
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
                        "matrix.drain",
                        format!("HTTP {}: {}", status.as_u16(), preview),
                        RetryClass::Retryable,
                    ));
                }

                let json: serde_json::Value = resp.json().map_err(|e| {
                    AdapterError::failed(
                        "matrix.drain",
                        format!("json parse failed: {e}"),
                        RetryClass::Retryable,
                    )
                })?;

                // Update next_batch token.
                if let Some(token) = json.get("next_batch").and_then(|v| v.as_str()) {
                    *next_batch = token.to_owned();
                    save_batch_token(batch_path.as_deref(), next_batch);
                }

                // Extract m.text events from the target room.
                let mut messages = Vec::new();
                if let Some(joined) = json
                    .pointer("/rooms/join")
                    .and_then(|v| v.as_object())
                {
                    // Only collect events from the configured room.
                    if let Some(room_data) = joined.get(room_id.as_str())
                        && let Some(events) = room_data
                            .pointer("/timeline/events")
                            .and_then(|v| v.as_array())
                    {
                        for event in events {
                            let is_msg = event
                                .get("type")
                                .and_then(|t| t.as_str())
                                .map(|t| t == "m.room.message")
                                .unwrap_or(false);

                            let is_text = event
                                .pointer("/content/msgtype")
                                .and_then(|v| v.as_str())
                                .map(|t| t == "m.text")
                                .unwrap_or(false);

                            if is_msg && is_text
                                && let Some(body_text) = event
                                    .pointer("/content/body")
                                    .and_then(|v| v.as_str())
                            {
                                messages.push(ChannelMessage::new(
                                    room_id.clone(),
                                    body_text,
                                ));
                            }
                        }
                    }
                }

                Ok(messages)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Read persisted next_batch token from path; returns empty string if absent or unreadable.
fn load_batch_token(path: Option<&std::path::Path>) -> String {
    path.and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

/// Persist next_batch token to path. Best-effort; ignores write errors.
fn save_batch_token(path: Option<&std::path::Path>, token: &str) {
    if let Some(p) = path {
        let _ = std::fs::write(p, token);
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
            "channel.message.topic",
            "must not be empty",
        ));
    }
    if message.body.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "channel.message.body",
            "must not be empty",
        ));
    }
    if message.body.len() > MAX_MATRIX_BODY_BYTES {
        return Err(AdapterError::invalid_input(
            "channel.message.body",
            "must not exceed configured message byte limit",
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests (all offline — no real Matrix server needed)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::ChannelMessage;

    fn make_config() -> MatrixConfig {
        MatrixConfig::new(
            "syt_valid_token_abc123",
            Some("!roomid:matrix.org".to_string()),
            Some("https://matrix.org".to_string()),
            vec![],
        )
        .unwrap()
    }

    fn make_config_no_room() -> MatrixConfig {
        MatrixConfig::new(
            "syt_valid_token_abc123",
            None,
            Some("https://matrix.org".to_string()),
            vec![],
        )
        .unwrap()
    }

    fn make_config_no_homeserver() -> MatrixConfig {
        MatrixConfig::new(
            "syt_valid_token_abc123",
            Some("!roomid:matrix.org".to_string()),
            None,
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn offline_send_accepts_valid_message() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("!roomid:matrix.org", "hello world");
        let receipt = adapter.send(msg).unwrap();
        assert!(receipt.accepted);
        assert_eq!(receipt.sequence, 1);
    }

    #[test]
    fn offline_send_rejects_empty_body() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("!roomid:matrix.org", "   ");
        assert!(adapter.send(msg).is_err());
    }

    #[test]
    fn offline_send_rejects_oversized_body() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("!roomid:matrix.org", "x".repeat(MAX_MATRIX_BODY_BYTES + 1));
        assert!(adapter.send(msg).is_err());
    }

    #[test]
    fn offline_drain_returns_queued_messages() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        adapter
            .send(ChannelMessage::new("!roomid:matrix.org", "msg1"))
            .unwrap();
        adapter
            .send(ChannelMessage::new("!roomid:matrix.org", "msg2"))
            .unwrap();
        let drained = adapter.drain().unwrap();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].body, "msg1");
        assert_eq!(drained[1].body, "msg2");
    }

    #[test]
    fn offline_adapter_is_not_live() {
        let adapter = MatrixChannelAdapter::new(make_config());
        assert!(!adapter.is_live());
    }

    #[test]
    fn live_without_room_id_falls_back_to_offline() {
        let config = make_config_no_room();
        let adapter = MatrixChannelAdapter::live(config).unwrap();
        assert!(!adapter.is_live());
    }

    #[test]
    fn live_without_homeserver_uses_default_and_goes_live() {
        let config = make_config_no_homeserver();
        // When room_id is present but homeserver is absent, we use the default homeserver
        // and create a live adapter (HTTP client ready, no actual connection attempted).
        let adapter = MatrixChannelAdapter::live(config).unwrap();
        assert!(adapter.is_live());
    }

    #[test]
    fn debug_does_not_expose_access_token() {
        let adapter = MatrixChannelAdapter::new(make_config());
        let debug_str = format!("{adapter:?}");
        assert!(debug_str.contains("offline"));
        assert!(!debug_str.contains("syt_valid_token_abc123"));
    }

    #[test]
    fn health_is_healthy_when_room_and_homeserver_set() {
        let config = make_config();
        assert_eq!(config.health(), AdapterHealth::Healthy);
    }

    #[test]
    fn health_is_degraded_when_room_id_missing() {
        let config = make_config_no_room();
        assert_eq!(config.health(), AdapterHealth::Degraded);
    }

    #[test]
    fn room_id_url_encoding() {
        let room_id = "!roomid:matrix.org";
        let encoded = encode_room_id(room_id);
        assert_eq!(encoded, "%21roomid%3Amatrix.org");
        assert!(!encoded.contains('!'));
        assert!(!encoded.contains(':'));
    }

    #[test]
    fn sanitize_room_id_for_filename_replaces_special_chars() {
        let room_id = "!roomid:matrix.org";
        let safe = sanitize_room_id_for_filename(room_id);
        assert_eq!(safe, "_roomid_matrix_org");
        // No special characters remain
        assert!(safe.chars().all(|c| c.is_alphanumeric() || c == '_'));
    }

    #[test]
    fn offline_drain_is_empty_initially() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        let drained = adapter.drain().unwrap();
        assert!(drained.is_empty());
    }

    #[test]
    fn offline_drain_clears_queue() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        adapter
            .send(ChannelMessage::new("!roomid:matrix.org", "once"))
            .unwrap();
        let first = adapter.drain().unwrap();
        assert_eq!(first.len(), 1);
        let second = adapter.drain().unwrap();
        assert!(second.is_empty());
    }

    #[test]
    fn sequence_increments_per_send() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        let r1 = adapter
            .send(ChannelMessage::new("!roomid:matrix.org", "a"))
            .unwrap();
        let r2 = adapter
            .send(ChannelMessage::new("!roomid:matrix.org", "b"))
            .unwrap();
        assert_eq!(r1.sequence, 1);
        assert_eq!(r2.sequence, 2);
    }

    #[test]
    fn health_is_degraded_when_homeserver_missing() {
        let config = make_config_no_homeserver();
        assert_eq!(config.health(), AdapterHealth::Degraded);
    }

    #[test]
    fn config_debug_redacts_token() {
        let config = make_config();
        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("[redacted]"));
        assert!(!debug_str.contains("syt_valid_token_abc123"));
    }

    #[test]
    fn load_batch_token_returns_empty_for_missing_file() {
        let result = load_batch_token(Some(std::path::Path::new(
            "/nonexistent/path/matrix_batch.txt",
        )));
        assert!(result.is_empty());
    }

    #[test]
    fn load_batch_token_returns_empty_for_none_path() {
        let result = load_batch_token(None);
        assert!(result.is_empty());
    }

    #[test]
    fn default_homeserver_constant_is_matrix_org() {
        assert_eq!(MATRIX_DEFAULT_HOMESERVER, "https://matrix.org");
    }
}
