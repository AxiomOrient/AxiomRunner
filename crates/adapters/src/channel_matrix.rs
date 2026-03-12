use crate::async_http_bridge::AsyncHttpBridge;
use crate::channel_registry::read_env_trimmed;
use crate::channel_validate;
use crate::contracts::{
    AdapterFuture, AdapterHealth, ChannelAdapter, ChannelMessage, ChannelSendReceipt,
};
use crate::error::{AdapterError, AdapterResult, RetryClass, classify_reqwest_error};
use std::collections::VecDeque;

const MAX_MATRIX_BODY_BYTES: usize = 4000;
const MATRIX_DEFAULT_HOMESERVER: &str = "https://matrix.org";
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;
/// Maximum bytes of an API error response body included in error messages.
const MAX_ERROR_BODY_PREVIEW: usize = channel_validate::DEFAULT_ERROR_BODY_PREVIEW_BYTES;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Percent-encode room_id for use in URL path segments.
/// Matrix room IDs look like `!roomid:server.com`; both `!` and `:` must be encoded.
fn encode_room_id(room_id: &str) -> String {
    room_id.replace('!', "%21").replace(':', "%3A")
}

/// Percent-encode a string for use as a URL query parameter value.
/// Only unreserved characters (RFC 3986 §2.3) are left as-is; all other bytes are
/// encoded as `%XX` (uppercase hex).
fn percent_encode_query(s: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

/// Sanitize room_id for use as a filename component.
/// Replaces any non-alphanumeric character with `_`.
fn sanitize_room_id_for_filename(room_id: &str) -> String {
    room_id
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

fn resolve_target_room_id(topic: &str, default_room_id: &str) -> String {
    if let Some(route) = channel_validate::decode_routed_topic(topic, "matrix") {
        return route;
    }

    let legacy = topic.trim();
    if legacy.starts_with('!') {
        legacy.to_string()
    } else {
        default_room_id.to_string()
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
        client: AsyncHttpBridge,
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
    /// Returns an error when `config.room_id` is missing.
    /// If `AXONRUNNER_RUNTIME_TOOL_WORKSPACE` is set, `next_batch` is persisted to
    /// `{workspace}/matrix_batch_{sanitized_room_id}.txt` and restored on restart.
    pub fn live(config: MatrixConfig) -> Result<Self, String> {
        let room_id = match &config.room_id {
            Some(r) => r.clone(),
            None => {
                return Err(String::from(
                    "matrix live mode requires room_id; use MatrixChannelAdapter::new for offline mode",
                ));
            }
        };
        // Use configured homeserver or fall back to default.
        let homeserver = config
            .homeserver
            .clone()
            .unwrap_or_else(|| MATRIX_DEFAULT_HOMESERVER.to_owned());

        let client = AsyncHttpBridge::with_timeouts(
            std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS),
            std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS),
        )
        .map_err(|e| format!("matrix http client init failed: {e}"))?;

        // Resolve batch persistence path from workspace env var.
        let batch_path = read_env_trimmed("AXONRUNNER_RUNTIME_TOOL_WORKSPACE").map(|workspace| {
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

    fn send(&mut self, message: ChannelMessage) -> AdapterFuture<'_, ChannelSendReceipt> {
        Box::pin(async move {
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
                    room_id: default_room_id,
                    sequence,
                    ..
                } => {
                    *sequence = sequence.saturating_add(1);
                    let txn_id = sequence.to_string();
                    let room_id = resolve_target_room_id(&message.topic, default_room_id);
                    let encoded_room = encode_room_id(room_id.as_str());

                    let url = format!(
                        "{homeserver}/_matrix/client/v3/rooms/{encoded_room}/send/m.room.message/{txn_id}"
                    );

                    let body = serde_json::json!({
                        "msgtype": "m.text",
                        "body": message.body,
                    });

                    let auth_header = format!("Bearer {}", access_token.as_str());
                    let resp = client
                        .put_json(&url, &[("Authorization", auth_header.as_str())], &body)
                        .map_err(|e| {
                            AdapterError::failed(
                                "matrix.send",
                                format!("http {}", classify_reqwest_error(&e)),
                                RetryClass::Retryable,
                            )
                        })?;

                    let status = resp.status;

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
                        let preview = channel_validate::truncate_utf8_preview(
                            resp.body.as_str(),
                            MAX_ERROR_BODY_PREVIEW,
                        );
                        return Err(AdapterError::failed(
                            "matrix.send",
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

    fn drain(&mut self) -> AdapterFuture<'_, Vec<ChannelMessage>> {
        Box::pin(async move {
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
                    // The filter JSON must be percent-encoded; `{`, `}`, `"`, `:` are
                    // not valid unescaped in a query parameter value (RFC 3986 §3.4).
                    let filter_raw = r#"{"room":{"timeline":{"limit":50}}}"#;
                    let filter = percent_encode_query(filter_raw);
                    let url = if next_batch.is_empty() {
                        format!("{homeserver}/_matrix/client/v3/sync?timeout=0&filter={filter}")
                    } else {
                        format!(
                            "{homeserver}/_matrix/client/v3/sync?since={since}&timeout=0&filter={filter}",
                            since = next_batch
                        )
                    };

                    let auth_header = format!("Bearer {}", access_token.as_str());
                    let resp = client
                        .get(&url, &[("Authorization", auth_header.as_str())])
                        .map_err(|e| {
                            AdapterError::failed(
                                "matrix.drain",
                                format!("http {}", classify_reqwest_error(&e)),
                                RetryClass::Retryable,
                            )
                        })?;

                    let status = resp.status;

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
                        let preview = channel_validate::truncate_utf8_preview(
                            resp.body.as_str(),
                            MAX_ERROR_BODY_PREVIEW,
                        );
                        return Err(AdapterError::failed(
                            "matrix.drain",
                            format!("HTTP {}: {preview}", status.as_u16()),
                            RetryClass::Retryable,
                        ));
                    }

                    let json: serde_json::Value =
                        serde_json::from_str(&resp.body).map_err(|e| {
                            AdapterError::failed(
                                "matrix.drain",
                                format!("json parse failed: {e}"),
                                RetryClass::Retryable,
                            )
                        })?;

                    // Extract m.text events from the target room, applying allowed_users filter.
                    let (new_batch, messages) =
                        extract_sync_messages(&json, room_id.as_str(), &self.config.allowed_users);

                    // Update next_batch token after extraction.
                    if let Some(token) = new_batch {
                        *next_batch = token;
                        save_batch_token(batch_path.as_deref(), next_batch);
                    }

                    Ok(messages)
                }
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Extract `m.text` messages from a Matrix `/sync` JSON response for a specific room,
/// applying the `allowed_users` whitelist filter.
///
/// Returns `(next_batch_token, messages)`.  If `allowed_users` is empty every sender is
/// accepted.  Events without a `sender` field are always skipped.
fn extract_sync_messages(
    json: &serde_json::Value,
    room_id: &str,
    allowed_users: &[String],
) -> (Option<String>, Vec<ChannelMessage>) {
    let next_batch = json
        .get("next_batch")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    let mut messages = Vec::new();

    let Some(joined) = json.pointer("/rooms/join").and_then(|v| v.as_object()) else {
        return (next_batch, messages);
    };

    let Some(room_data) = joined.get(room_id) else {
        return (next_batch, messages);
    };

    let Some(events) = room_data
        .pointer("/timeline/events")
        .and_then(|v| v.as_array())
    else {
        return (next_batch, messages);
    };

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

        if !is_msg || !is_text {
            continue;
        }

        // Skip events that have no sender field — malformed events.
        let Some(sender) = event.get("sender").and_then(|v| v.as_str()) else {
            continue;
        };

        // Self-reply prevention: configure allowed_users to exclude the bot's own MXID.
        if !allowed_users.is_empty() && !allowed_users.iter().any(|u| u.as_str() == sender) {
            continue;
        }

        if let Some(body_text) = event.pointer("/content/body").and_then(|v| v.as_str()) {
            messages.push(ChannelMessage::new(
                channel_validate::encode_routed_topic("matrix", room_id),
                body_text,
            ));
        }
    }

    (next_batch, messages)
}

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
    use std::future::Future;

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should initialize")
            .block_on(future)
    }

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
        let receipt = block_on(adapter.send(msg)).unwrap();
        assert!(receipt.accepted);
        assert_eq!(receipt.sequence, 1);
    }

    #[test]
    fn offline_send_rejects_empty_body() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("!roomid:matrix.org", "   ");
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_send_rejects_oversized_body() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("!roomid:matrix.org", "x".repeat(MAX_MATRIX_BODY_BYTES + 1));
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_drain_returns_queued_messages() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        block_on(adapter.send(ChannelMessage::new("!roomid:matrix.org", "msg1"))).unwrap();
        block_on(adapter.send(ChannelMessage::new("!roomid:matrix.org", "msg2"))).unwrap();
        let drained = block_on(adapter.drain()).unwrap();
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
    fn live_without_room_id_returns_error() {
        let config = make_config_no_room();
        let error = MatrixChannelAdapter::live(config).expect_err("missing room_id should fail");
        assert!(error.contains("requires room_id"));
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
        let drained = block_on(adapter.drain()).unwrap();
        assert!(drained.is_empty());
    }

    #[test]
    fn offline_drain_clears_queue() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        block_on(adapter.send(ChannelMessage::new("!roomid:matrix.org", "once"))).unwrap();
        let first = block_on(adapter.drain()).unwrap();
        assert_eq!(first.len(), 1);
        let second = block_on(adapter.drain()).unwrap();
        assert!(second.is_empty());
    }

    #[test]
    fn sequence_increments_per_send() {
        let mut adapter = MatrixChannelAdapter::new(make_config());
        let r1 = block_on(adapter.send(ChannelMessage::new("!roomid:matrix.org", "a"))).unwrap();
        let r2 = block_on(adapter.send(ChannelMessage::new("!roomid:matrix.org", "b"))).unwrap();
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

    #[test]
    fn percent_encode_query_encodes_json_filter() {
        let filter = r#"{"room":{"timeline":{"limit":50}}}"#;
        let encoded = percent_encode_query(filter);
        // Curly braces, quotes, and colons must all be encoded.
        assert!(!encoded.contains('{'));
        assert!(!encoded.contains('}'));
        assert!(!encoded.contains('"'));
        assert!(!encoded.contains(':'));
        // Digits and letters pass through unmodified.
        assert!(encoded.contains("room"));
        assert!(encoded.contains("timeline"));
        assert!(encoded.contains("limit"));
        assert!(encoded.contains("50"));
        // Spot-check exact encoding of a few characters.
        assert!(encoded.contains("%7B")); // {
        assert!(encoded.contains("%7D")); // }
        assert!(encoded.contains("%22")); // "
        assert!(encoded.contains("%3A")); // :
    }

    #[test]
    fn percent_encode_query_preserves_unreserved_chars() {
        let input = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~";
        assert_eq!(percent_encode_query(input), input);
    }

    // -----------------------------------------------------------------------
    // extract_sync_messages — allowed_users filter tests
    // -----------------------------------------------------------------------

    fn make_sync_json(events: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "next_batch": "s123",
            "rooms": {
                "join": {
                    "!room:example.org": {
                        "timeline": {
                            "events": events
                        }
                    }
                }
            }
        })
    }

    #[test]
    fn drain_filters_by_allowed_users() {
        // allowed_users = ["@alice:example.org"] — bob's message must be dropped.
        let allowed = vec!["@alice:example.org".to_string()];
        let json = make_sync_json(serde_json::json!([
            {
                "type": "m.room.message",
                "sender": "@alice:example.org",
                "content": { "msgtype": "m.text", "body": "hello" }
            },
            {
                "type": "m.room.message",
                "sender": "@bob:example.org",
                "content": { "msgtype": "m.text", "body": "world" }
            }
        ]));

        let (_, messages) = extract_sync_messages(&json, "!room:example.org", &allowed);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].body, "hello");
    }

    #[test]
    fn drain_accepts_all_when_allowed_users_empty() {
        // allowed_users = [] — every sender must be accepted.
        let json = make_sync_json(serde_json::json!([
            {
                "type": "m.room.message",
                "sender": "@alice:example.org",
                "content": { "msgtype": "m.text", "body": "hello" }
            },
            {
                "type": "m.room.message",
                "sender": "@bob:example.org",
                "content": { "msgtype": "m.text", "body": "world" }
            }
        ]));

        let (_, messages) = extract_sync_messages(&json, "!room:example.org", &[]);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].body, "hello");
        assert_eq!(messages[1].body, "world");
    }

    #[test]
    fn drain_skips_events_without_sender() {
        // Events lacking a "sender" field must be skipped regardless of allowed_users.
        let json = make_sync_json(serde_json::json!([
            {
                "type": "m.room.message",
                "content": { "msgtype": "m.text", "body": "no sender" }
            },
            {
                "type": "m.room.message",
                "sender": "@alice:example.org",
                "content": { "msgtype": "m.text", "body": "has sender" }
            }
        ]));

        let (_, messages) = extract_sync_messages(&json, "!room:example.org", &[]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].body, "has sender");
    }
}
