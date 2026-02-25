use crate::contracts::{AdapterHealth, ToolAdapter, ToolCall, ToolOutput};
use crate::error::{AdapterError, AdapterResult, RetryClass};
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserToolAction {
    Open,
    Current,
}

impl BrowserToolAction {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "browser.open" | "browser_open" => Some(Self::Open),
            "browser.current" | "browser_current" => Some(Self::Current),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserToolConfig {
    pub allowed_hosts: Vec<String>,
    pub require_https: bool,
    pub max_url_bytes: usize,
}

impl Default for BrowserToolConfig {
    fn default() -> Self {
        Self {
            allowed_hosts: vec![String::from("example.com")],
            require_https: true,
            max_url_bytes: 2048,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserOpenInput {
    url: String,
}

#[derive(Debug)]
pub struct BrowserToolAdapter {
    config: BrowserToolConfig,
    current_url: Mutex<Option<String>>,
}

impl BrowserToolAdapter {
    pub fn new(mut config: BrowserToolConfig) -> Self {
        normalize_allowed_hosts(&mut config.allowed_hosts);

        Self {
            config,
            current_url: Mutex::new(None),
        }
    }
}

impl ToolAdapter for BrowserToolAdapter {
    fn id(&self) -> &str {
        "tool.browser"
    }

    fn health(&self) -> AdapterHealth {
        if self.config.allowed_hosts.is_empty() {
            AdapterHealth::Degraded
        } else {
            AdapterHealth::Healthy
        }
    }

    fn execute(&self, call: ToolCall) -> AdapterResult<ToolOutput> {
        let action = BrowserToolAction::parse(call.name.trim()).ok_or_else(|| {
            AdapterError::not_found(
                "browser_tool_action",
                if call.name.trim().is_empty() {
                    String::from("<empty>")
                } else {
                    call.name.clone()
                },
            )
        })?;

        match action {
            BrowserToolAction::Open => {
                let input = parse_open_input(&call.args, self.config.max_url_bytes)?;
                let parsed = parse_url(&input.url)?;
                validate_url(parsed, &self.config)?;

                let mut current = self.current_url.lock().map_err(|_| {
                    AdapterError::failed(
                        "browser.current.lock",
                        "browser lock poisoned",
                        RetryClass::NonRetryable,
                    )
                })?;
                *current = Some(input.url.clone());

                Ok(ToolOutput {
                    content: format!("browser.open url={} host={}", input.url, parsed.host),
                })
            }
            BrowserToolAction::Current => {
                let current = self.current_url.lock().map_err(|_| {
                    AdapterError::failed(
                        "browser.current.lock",
                        "browser lock poisoned",
                        RetryClass::NonRetryable,
                    )
                })?;
                let current = current.as_deref().unwrap_or("<none>");
                Ok(ToolOutput {
                    content: format!("browser.current url={current}"),
                })
            }
        }
    }
}

fn parse_open_input(
    args: &BTreeMap<String, String>,
    max_url_bytes: usize,
) -> AdapterResult<BrowserOpenInput> {
    let raw_url = args
        .get("url")
        .ok_or_else(|| AdapterError::invalid_input("browser.url", "is required"))?;
    let url = raw_url.trim();
    if url.is_empty() {
        return Err(AdapterError::invalid_input(
            "browser.url",
            "must not be empty",
        ));
    }
    if url.len() > max_url_bytes {
        return Err(AdapterError::invalid_input(
            "browser.url",
            "exceeds byte limit",
        ));
    }
    Ok(BrowserOpenInput {
        url: url.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedUrl<'a> {
    scheme: &'a str,
    host: &'a str,
}

fn parse_url(url: &str) -> AdapterResult<ParsedUrl<'_>> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| AdapterError::invalid_input("browser.url", "must include scheme"))?;

    if scheme.trim().is_empty() {
        return Err(AdapterError::invalid_input(
            "browser.url",
            "scheme must not be empty",
        ));
    }

    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .ok_or_else(|| AdapterError::invalid_input("browser.url", "host is required"))?;

    let host = authority
        .split(':')
        .next()
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .ok_or_else(|| AdapterError::invalid_input("browser.url", "host is required"))?;

    if host.contains(char::is_whitespace) {
        return Err(AdapterError::invalid_input(
            "browser.url",
            "host must not contain whitespace",
        ));
    }

    Ok(ParsedUrl { scheme, host })
}

fn validate_url(parsed: ParsedUrl<'_>, config: &BrowserToolConfig) -> AdapterResult<()> {
    if config.require_https && !parsed.scheme.eq_ignore_ascii_case("https") {
        return Err(AdapterError::invalid_input(
            "browser.url",
            "https scheme is required",
        ));
    }

    if !is_host_allowed(parsed.host, &config.allowed_hosts) {
        return Err(AdapterError::invalid_input(
            "browser.url",
            "host is not in allowlist",
        ));
    }

    Ok(())
}

fn is_host_allowed(host: &str, allowed_hosts: &[String]) -> bool {
    let host = host.to_ascii_lowercase();

    for allowed in allowed_hosts {
        if allowed.starts_with("*.") {
            let suffix = &allowed[1..];
            if host.ends_with(suffix) && host.len() > suffix.len() {
                return true;
            }
            continue;
        }

        if host == *allowed {
            return true;
        }
    }

    false
}

fn normalize_allowed_hosts(allowed_hosts: &mut Vec<String>) {
    for host in allowed_hosts.iter_mut() {
        *host = host.trim().to_ascii_lowercase();
    }
    allowed_hosts.retain(|host| !host.is_empty());
    allowed_hosts.sort_unstable();
    allowed_hosts.dedup();
}
