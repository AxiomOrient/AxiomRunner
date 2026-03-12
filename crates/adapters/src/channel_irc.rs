use crate::channel_validate;
use crate::contracts::{
    AdapterFuture, AdapterHealth, ChannelAdapter, ChannelMessage, ChannelSendReceipt,
};
use crate::error::{AdapterError, AdapterResult, RetryClass};
use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_IRC_BODY_BYTES: usize = 512;
const IRC_DEFAULT_PORT: u16 = 6667;
/// Timeout used during non-blocking drain reads.
const IRC_READ_TIMEOUT_MS: u64 = 100;
/// How long to wait for RPL_WELCOME (001) before giving up.
const IRC_WELCOME_TIMEOUT_SECS: u64 = 5;

// ---------------------------------------------------------------------------
// IRC line parser
// ---------------------------------------------------------------------------

enum IrcParsed {
    Ping(String),
    Privmsg(String),
}

fn parse_irc_line(line: &str) -> Option<IrcParsed> {
    let line = line.trim_end_matches(['\r', '\n']);

    if line.starts_with("PING ") {
        let server = line.strip_prefix("PING :").unwrap_or("");
        return Some(IrcParsed::Ping(server.to_owned()));
    }

    // :nick!user@host PRIVMSG #channel :message
    let rest = if line.starts_with(':') {
        line.split_once(' ').map(|x| x.1).unwrap_or("")
    } else {
        line
    };

    let parts: Vec<&str> = rest.splitn(3, ' ').collect();
    if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("PRIVMSG") {
        let trailing = parts[2].strip_prefix(':').unwrap_or(parts[2]);
        return Some(IrcParsed::Privmsg(trailing.to_owned()));
    }

    None
}

// ---------------------------------------------------------------------------
// Read one line from TcpStream byte-by-byte; respects the socket read_timeout.
// ---------------------------------------------------------------------------

fn read_line_with_timeout(
    stream: &mut std::net::TcpStream,
    buf: &mut String,
) -> std::io::Result<usize> {
    use std::io::Read;
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Ok(buf.len()),
            Ok(_) => {
                let ch = byte[0] as char;
                buf.push(ch);
                if ch == '\n' {
                    return Ok(buf.len());
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                return Err(e);
            }
            Err(e) => return Err(e),
        }
    }
}

// ---------------------------------------------------------------------------
// Transport enum
// ---------------------------------------------------------------------------

enum IrcTransport {
    Offline {
        queue: VecDeque<ChannelMessage>,
        sequence: u64,
    },
    Live {
        stream: Arc<Mutex<std::net::TcpStream>>,
        /// Accumulation buffer for partial lines between drain calls.
        read_buf: String,
        sequence: u64,
        channel: String,
    },
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrcConfig {
    pub server: String,
    pub channel: Option<String>,
    pub nick: String,
    pub allowed_users: Vec<String>,
}

impl IrcConfig {
    pub fn new(
        server: impl Into<String>,
        channel: Option<String>,
        nick: impl Into<String>,
        allowed_users: Vec<String>,
    ) -> AdapterResult<Self> {
        let server = normalize_server(server.into())?;
        let channel = normalize_optional_value(channel, "irc.channel")?;
        let nick = normalize_token(nick.into(), "irc.nick")?;
        let allowed_users = normalize_allowed_users(allowed_users, "irc.allowed_users")?;

        Ok(Self {
            server,
            channel,
            nick,
            allowed_users,
        })
    }

    fn health(&self) -> AdapterHealth {
        if self.server.contains("invalid") {
            return AdapterHealth::Unavailable;
        }

        if self.channel.is_none() {
            AdapterHealth::Degraded
        } else {
            AdapterHealth::Healthy
        }
    }
}

// ---------------------------------------------------------------------------
// Adapter struct
// ---------------------------------------------------------------------------

pub struct IrcChannelAdapter {
    config: IrcConfig,
    transport: IrcTransport,
}

impl std::fmt::Debug for IrcChannelAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match &self.transport {
            IrcTransport::Offline { .. } => "offline",
            IrcTransport::Live { .. } => "live",
        };
        f.debug_struct("IrcChannelAdapter")
            .field("transport", &mode)
            .finish()
    }
}

impl IrcChannelAdapter {
    /// Create an offline adapter backed by an in-process queue. Always safe for tests.
    pub fn new(config: IrcConfig) -> Self {
        Self {
            transport: IrcTransport::Offline {
                queue: VecDeque::new(),
                sequence: 0,
            },
            config,
        }
    }

    /// Connect to the IRC server and join the configured channel.
    /// Returns an error when `config.channel` is missing.
    pub fn live(config: IrcConfig) -> Result<Self, String> {
        let Some(ref raw_channel) = config.channel else {
            return Err(String::from(
                "irc live mode requires channel; use IrcChannelAdapter::new for offline mode",
            ));
        };

        let (host, port) = parse_server_addr(&config.server);

        let mut stream = std::net::TcpStream::connect((host, port))
            .map_err(|e| format!("irc: tcp connect failed: {e}"))?;

        // Registration phase: use a generous timeout for welcome wait.
        stream
            .set_read_timeout(Some(Duration::from_secs(IRC_WELCOME_TIMEOUT_SECS)))
            .map_err(|e| format!("irc: set_read_timeout failed: {e}"))?;

        write!(stream, "PASS *\r\n").map_err(|e| format!("irc: PASS write failed: {e}"))?;
        write!(stream, "NICK {}\r\n", config.nick)
            .map_err(|e| format!("irc: NICK write failed: {e}"))?;
        write!(stream, "USER {} 0 * :AxonRunner Bot\r\n", config.nick)
            .map_err(|e| format!("irc: USER write failed: {e}"))?;

        wait_for_welcome(&mut stream).map_err(|e| format!("irc: welcome handshake failed: {e}"))?;

        let channel = if raw_channel.starts_with('#') {
            raw_channel.clone()
        } else {
            format!("#{}", raw_channel)
        };

        write!(stream, "JOIN {}\r\n", channel)
            .map_err(|e| format!("irc: JOIN write failed: {e}"))?;

        // Switch to non-blocking drain timeout.
        stream
            .set_read_timeout(Some(Duration::from_millis(IRC_READ_TIMEOUT_MS)))
            .map_err(|e| format!("irc: set drain read_timeout failed: {e}"))?;

        Ok(Self {
            transport: IrcTransport::Live {
                stream: Arc::new(Mutex::new(stream)),
                read_buf: String::new(),
                sequence: 0,
                channel,
            },
            config,
        })
    }

    pub fn config(&self) -> &IrcConfig {
        &self.config
    }

    /// Returns true when this adapter holds an active TCP connection.
    pub fn is_live(&self) -> bool {
        matches!(self.transport, IrcTransport::Live { .. })
    }
}

// ---------------------------------------------------------------------------
// ChannelAdapter impl
// ---------------------------------------------------------------------------

impl ChannelAdapter for IrcChannelAdapter {
    fn id(&self) -> &str {
        "channel.irc"
    }

    fn health(&self) -> AdapterHealth {
        self.config.health()
    }

    fn send(&mut self, message: ChannelMessage) -> AdapterFuture<'_, ChannelSendReceipt> {
        Box::pin(async move {
            validate_message(&message)?;

            match &mut self.transport {
                IrcTransport::Offline { queue, sequence } => {
                    *sequence = sequence.saturating_add(1);
                    queue.push_back(message);
                    Ok(ChannelSendReceipt {
                        sequence: *sequence,
                        accepted: true,
                    })
                }
                IrcTransport::Live {
                    stream,
                    channel,
                    sequence,
                    ..
                } => {
                    let stream = Arc::clone(stream);
                    let channel = channel_validate::decode_routed_topic(&message.topic, "irc")
                        .unwrap_or_else(|| channel.clone());
                    let body = truncate_to_bytes(&message.body, MAX_IRC_BODY_BYTES).to_string();
                    run_blocking_io("irc.send", move || {
                        let mut guard = stream.lock().map_err(|_| {
                            AdapterError::failed(
                                "irc.send",
                                "stream lock poisoned",
                                RetryClass::NonRetryable,
                            )
                        })?;

                        // Drain pending PINGs before writing so the connection stays alive.
                        drain_pings(&mut guard);

                        write!(guard, "PRIVMSG {} :{}\r\n", channel, body).map_err(|e| {
                            AdapterError::failed(
                                "irc.send",
                                format!("write failed: {e}"),
                                RetryClass::Retryable,
                            )
                        })?;
                        Ok(())
                    })
                    .await?;

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
                IrcTransport::Offline { queue, .. } => {
                    let mut drained = Vec::with_capacity(queue.len());
                    while let Some(msg) = queue.pop_front() {
                        drained.push(msg);
                    }
                    Ok(drained)
                }
                IrcTransport::Live {
                    stream,
                    read_buf,
                    channel,
                    ..
                } => {
                    let stream = Arc::clone(stream);
                    let channel = channel.clone();
                    let carry_buf = std::mem::take(read_buf);
                    let (messages, next_read_buf) = run_blocking_io("irc.drain", move || {
                        drain_live_messages(stream, channel, carry_buf)
                    })
                    .await?;
                    *read_buf = next_read_buf;
                    Ok(messages)
                }
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn run_blocking_io<T, F>(context: &'static str, operation: F) -> AdapterResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AdapterResult<T> + Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(_) => tokio::task::spawn_blocking(operation)
            .await
            .map_err(|error| {
                AdapterError::failed(
                    context,
                    format!("blocking task join failed: {error}"),
                    RetryClass::Retryable,
                )
            })?,
        Err(_) => operation(),
    }
}

fn drain_live_messages(
    stream: Arc<Mutex<std::net::TcpStream>>,
    channel: String,
    mut carry_buf: String,
) -> AdapterResult<(Vec<ChannelMessage>, String)> {
    let mut guard = stream.lock().map_err(|_| {
        AdapterError::failed(
            "irc.drain",
            "stream lock poisoned",
            RetryClass::NonRetryable,
        )
    })?;

    let mut messages = Vec::new();
    loop {
        let mut line_buf = String::new();
        match read_line_with_timeout(&mut guard, &mut line_buf) {
            Ok(0) => break,
            Ok(_) => {
                let full_line = if carry_buf.is_empty() {
                    line_buf
                } else {
                    let mut merged = std::mem::take(&mut carry_buf);
                    merged.push_str(&line_buf);
                    merged
                };
                match parse_irc_line(&full_line) {
                    Some(IrcParsed::Ping(server)) => {
                        // Best-effort PONG; ignore write errors.
                        let _ = write!(guard, "PONG :{}\r\n", server);
                    }
                    Some(IrcParsed::Privmsg(text)) => {
                        messages.push(ChannelMessage::new(
                            channel_validate::encode_routed_topic("irc", &channel),
                            text,
                        ));
                    }
                    None => {}
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                if !line_buf.is_empty() {
                    carry_buf = line_buf;
                }
                break;
            }
            Err(error) => {
                return Err(AdapterError::failed(
                    "irc.drain",
                    format!("read failed: {error}"),
                    RetryClass::Retryable,
                ));
            }
        }
    }
    Ok((messages, carry_buf))
}

/// Parse "host:port" or just "host" from the server string.
fn parse_server_addr(server: &str) -> (&str, u16) {
    if let Some(colon) = server.rfind(':') {
        let host = &server[..colon];
        let port_str = &server[colon + 1..];
        if let Ok(p) = port_str.parse::<u16>() {
            return (host, p);
        }
    }
    (server, IRC_DEFAULT_PORT)
}

/// Truncate `s` to at most `max_bytes` without splitting a UTF-8 code point.
fn truncate_to_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Read and respond to any pending PING messages. Best-effort; ignores errors.
fn drain_pings(stream: &mut std::net::TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
    let mut line = String::new();
    while let Ok(n) = read_line_with_timeout(stream, &mut line) {
        if n == 0 {
            break;
        }
        if let Some(IrcParsed::Ping(server)) = parse_irc_line(&line) {
            let _ = write!(stream, "PONG :{}\r\n", server);
        }
        line.clear();
    }
    let _ = stream.set_read_timeout(Some(Duration::from_millis(IRC_READ_TIMEOUT_MS)));
}

/// Block until the server sends a line containing " 001 " (RPL_WELCOME).
/// Responds to PING messages encountered during the wait.
fn wait_for_welcome(stream: &mut std::net::TcpStream) -> std::io::Result<()> {
    loop {
        let mut line = String::new();
        read_line_with_timeout(stream, &mut line)?;
        if line.contains(" 001 ") {
            return Ok(());
        }
        if line.starts_with("PING ") {
            let server = line
                .strip_prefix("PING :")
                .unwrap_or("")
                .trim_end_matches('\r')
                .trim_end_matches('\n');
            write!(stream, "PONG :{}\r\n", server)?;
        }
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn normalize_server(raw: String) -> AdapterResult<String> {
    let server = raw.trim();
    if server.is_empty() {
        return Err(AdapterError::invalid_input(
            "irc.server",
            "must not be empty",
        ));
    }
    if server.contains(char::is_whitespace) {
        return Err(AdapterError::invalid_input(
            "irc.server",
            "must not contain whitespace",
        ));
    }
    if !server.contains('.') && !server.contains(':') {
        return Err(AdapterError::invalid_input(
            "irc.server",
            "must include hostname separator",
        ));
    }

    Ok(server.to_string())
}

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
            "irc.message.topic",
            "must not be empty",
        ));
    }
    if message.body.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "irc.message.body",
            "must not be empty",
        ));
    }
    if message.body.len() > MAX_IRC_BODY_BYTES {
        return Err(AdapterError::invalid_input(
            "irc.message.body",
            "must not exceed 512 bytes",
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests (all offline — no real IRC server needed)
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

    fn make_config() -> IrcConfig {
        IrcConfig::new(
            "irc.example.com:6667",
            Some("test".to_string()),
            "axonrunner-bot",
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn offline_send_accepts_valid_message() {
        let mut adapter = IrcChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("irc", "hello world");
        let receipt = block_on(adapter.send(msg)).unwrap();
        assert!(receipt.accepted);
        assert_eq!(receipt.sequence, 1);
    }

    #[test]
    fn offline_send_rejects_empty_body() {
        let mut adapter = IrcChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("irc", "   ");
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_send_rejects_oversized_body() {
        let mut adapter = IrcChannelAdapter::new(make_config());
        let msg = ChannelMessage::new("irc", "x".repeat(MAX_IRC_BODY_BYTES + 1));
        assert!(block_on(adapter.send(msg)).is_err());
    }

    #[test]
    fn offline_drain_returns_queued_messages() {
        let mut adapter = IrcChannelAdapter::new(make_config());
        block_on(adapter.send(ChannelMessage::new("irc", "msg1"))).unwrap();
        block_on(adapter.send(ChannelMessage::new("irc", "msg2"))).unwrap();
        let drained = block_on(adapter.drain()).unwrap();
        assert_eq!(drained.len(), 2);
    }

    #[test]
    fn offline_adapter_is_not_live() {
        let adapter = IrcChannelAdapter::new(make_config());
        assert!(!adapter.is_live());
    }

    #[test]
    fn live_without_channel_returns_error() {
        let config =
            IrcConfig::new("irc.example.com:6667", None, "axonrunner-bot", vec![]).unwrap();
        let error = IrcChannelAdapter::live(config).expect_err("missing channel should fail");
        assert!(error.contains("requires channel"));
    }

    #[test]
    fn parse_irc_line_ping_returns_server() {
        match parse_irc_line("PING :irc.example.com\r\n") {
            Some(IrcParsed::Ping(s)) => assert_eq!(s, "irc.example.com"),
            _ => panic!("expected Ping"),
        }
    }

    #[test]
    fn parse_irc_line_privmsg_returns_message() {
        let line = ":alice!alice@host PRIVMSG #test :hello there\r\n";
        match parse_irc_line(line) {
            Some(IrcParsed::Privmsg(text)) => assert_eq!(text, "hello there"),
            _ => panic!("expected Privmsg"),
        }
    }

    #[test]
    fn health_is_healthy_when_channel_is_set() {
        let config = make_config();
        assert_eq!(config.health(), AdapterHealth::Healthy);
    }

    #[test]
    fn health_is_degraded_when_channel_is_none() {
        let config =
            IrcConfig::new("irc.example.com:6667", None, "axonrunner-bot", vec![]).unwrap();
        assert_eq!(config.health(), AdapterHealth::Degraded);
    }

    #[test]
    fn parse_server_addr_splits_host_and_port() {
        let (host, port) = parse_server_addr("irc.example.com:6697");
        assert_eq!(host, "irc.example.com");
        assert_eq!(port, 6697);
    }

    #[test]
    fn parse_server_addr_defaults_to_6667() {
        let (host, port) = parse_server_addr("irc.example.com");
        assert_eq!(host, "irc.example.com");
        assert_eq!(port, IRC_DEFAULT_PORT);
    }

    #[test]
    fn truncate_to_bytes_within_limit_unchanged() {
        let s = "hello";
        assert_eq!(truncate_to_bytes(s, 512), s);
    }

    #[test]
    fn truncate_to_bytes_clips_at_boundary() {
        let s = "a".repeat(600);
        assert_eq!(truncate_to_bytes(&s, 512).len(), 512);
    }

    #[test]
    fn debug_does_not_expose_server_details() {
        let adapter = IrcChannelAdapter::new(make_config());
        let debug_str = format!("{adapter:?}");
        assert!(debug_str.contains("offline"));
        assert!(!debug_str.contains("irc.example.com"));
    }
}
