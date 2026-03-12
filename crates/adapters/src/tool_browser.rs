use crate::contracts::{AdapterFuture, AdapterHealth, ToolAdapter, ToolCall, ToolOutput};
use crate::error::{AdapterError, AdapterResult, RetryClass, classify_reqwest_error};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserToolAction {
    Open,
    Current,
    Find,
}

impl BrowserToolAction {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "browser.open" | "browser_open" => Some(Self::Open),
            "browser.current" | "browser_current" => Some(Self::Current),
            "browser.find" | "browser_find" => Some(Self::Find),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserToolConfig {
    pub allowed_hosts: Vec<String>,
    pub require_https: bool,
    pub max_url_bytes: usize,
    pub max_page_bytes: usize,
    pub max_query_bytes: usize,
    pub request_timeout_secs: u64,
}

impl Default for BrowserToolConfig {
    fn default() -> Self {
        Self {
            allowed_hosts: vec![String::from("example.com")],
            require_https: true,
            max_url_bytes: 2048,
            max_page_bytes: 256 * 1024,
            max_query_bytes: 512,
            request_timeout_secs: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserOpenInput {
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserFindInput {
    query: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserPageSnapshot {
    url: String,
    host: String,
    status: u16,
    title: Option<String>,
    body: String,
    body_bytes: usize,
}

pub struct BrowserToolAdapter {
    config: BrowserToolConfig,
    http_client: reqwest::blocking::Client,
    current_page: Mutex<Option<BrowserPageSnapshot>>,
}

impl BrowserToolAdapter {
    pub fn new(mut config: BrowserToolConfig) -> Self {
        normalize_allowed_hosts(&mut config.allowed_hosts);
        let timeout = Duration::from_secs(config.request_timeout_secs.max(1));
        let http_client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self {
            config,
            http_client,
            current_page: Mutex::new(None),
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

    fn execute(&self, call: ToolCall) -> AdapterFuture<'_, ToolOutput> {
        Box::pin(async move {
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
                    let snapshot = fetch_page_snapshot(
                        &self.http_client,
                        &input.url,
                        parsed.host,
                        self.config.max_page_bytes,
                    )?;
                    let title = snapshot.title.as_deref().unwrap_or("<none>");

                    let mut current = self.current_page.lock().map_err(|_| {
                        AdapterError::failed(
                            "browser.current.lock",
                            "browser lock poisoned",
                            RetryClass::NonRetryable,
                        )
                    })?;
                    *current = Some(snapshot.clone());

                    Ok(ToolOutput {
                        content: format!(
                            "browser.open url={} host={} status={} bytes={} title={}",
                            snapshot.url,
                            snapshot.host,
                            snapshot.status,
                            snapshot.body_bytes,
                            title
                        ),
                    })
                }
                BrowserToolAction::Current => {
                    let current = self.current_page.lock().map_err(|_| {
                        AdapterError::failed(
                            "browser.current.lock",
                            "browser lock poisoned",
                            RetryClass::NonRetryable,
                        )
                    })?;
                    match current.as_ref() {
                        Some(snapshot) => Ok(ToolOutput {
                            content: format!(
                                "browser.current url={} host={} status={} bytes={} title={}",
                                snapshot.url,
                                snapshot.host,
                                snapshot.status,
                                snapshot.body_bytes,
                                snapshot.title.as_deref().unwrap_or("<none>")
                            ),
                        }),
                        None => Ok(ToolOutput {
                            content: String::from("browser.current url=<none>"),
                        }),
                    }
                }
                BrowserToolAction::Find => {
                    let input = parse_find_input(&call.args, self.config.max_query_bytes)?;
                    let current = self.current_page.lock().map_err(|_| {
                        AdapterError::failed(
                            "browser.current.lock",
                            "browser lock poisoned",
                            RetryClass::NonRetryable,
                        )
                    })?;
                    let snapshot = current.as_ref().ok_or_else(|| {
                        AdapterError::invalid_input(
                            "browser.current",
                            "no page is currently loaded",
                        )
                    })?;
                    let hits = count_case_insensitive_occurrences(&snapshot.body, &input.query);
                    Ok(ToolOutput {
                        content: format!(
                            "browser.find url={} query={} hits={hits}",
                            snapshot.url, input.query
                        ),
                    })
                }
            }
        })
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

fn parse_find_input(
    args: &BTreeMap<String, String>,
    max_query_bytes: usize,
) -> AdapterResult<BrowserFindInput> {
    let raw_query = args
        .get("query")
        .ok_or_else(|| AdapterError::invalid_input("browser.query", "is required"))?;
    let query = raw_query.trim();
    if query.is_empty() {
        return Err(AdapterError::invalid_input(
            "browser.query",
            "must not be empty",
        ));
    }
    if query.len() > max_query_bytes {
        return Err(AdapterError::invalid_input(
            "browser.query",
            "exceeds byte limit",
        ));
    }
    Ok(BrowserFindInput {
        query: query.to_string(),
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

fn fetch_page_snapshot(
    client: &reqwest::blocking::Client,
    url: &str,
    host: &str,
    max_page_bytes: usize,
) -> AdapterResult<BrowserPageSnapshot> {
    let client = client.clone();
    let url_owned = url.to_string();
    let fetch_result = std::thread::spawn(move || {
        let response = client.get(&url_owned).send()?;
        let status = response.status().as_u16();
        let body = response.text()?;
        Ok::<(u16, String), reqwest::Error>((status, body))
    })
    .join()
    .map_err(|_| {
        AdapterError::failed(
            "browser.open.fetch",
            "worker thread panicked",
            RetryClass::Retryable,
        )
    })?;

    let (status, body_raw) = fetch_result.map_err(|error| {
        AdapterError::failed(
            "browser.open.fetch",
            classify_reqwest_error(&error),
            RetryClass::Retryable,
        )
    })?;
    if status >= 400 {
        return Err(AdapterError::failed(
            "browser.open.fetch",
            format!("http status {status}"),
            RetryClass::NonRetryable,
        ));
    }

    let body = truncate_utf8_bytes(&body_raw, max_page_bytes);
    let title = extract_html_title(&body);
    let body_bytes = body.len();

    Ok(BrowserPageSnapshot {
        url: url.to_string(),
        host: host.to_string(),
        status,
        title,
        body,
        body_bytes,
    })
}

fn truncate_utf8_bytes(raw: &str, max_bytes: usize) -> String {
    if raw.len() <= max_bytes {
        return raw.to_string();
    }
    let cut = (0..=max_bytes)
        .rev()
        .find(|i| raw.is_char_boundary(*i))
        .unwrap_or(0);
    raw[..cut].to_string()
}

fn extract_html_title(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let title_tag_start = lower.find("<title")?;
    let title_open_end = lower[title_tag_start..].find('>')?;
    let content_start = title_tag_start + title_open_end + 1;
    let title_close_rel = lower[content_start..].find("</title>")?;
    let content_end = content_start + title_close_rel;
    let title = normalize_inline_text(&body[content_start..content_end]);
    if title.is_empty() { None } else { Some(title) }
}

fn normalize_inline_text(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn count_case_insensitive_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let needle = needle.to_ascii_lowercase();
    let haystack = haystack.to_ascii_lowercase();
    let mut count = 0usize;
    let mut offset = 0usize;
    while let Some(idx) = haystack[offset..].find(&needle) {
        count = count.saturating_add(1);
        offset = offset.saturating_add(idx + needle.len());
    }
    count
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

#[cfg(test)]
mod tests {
    use super::{
        count_case_insensitive_occurrences, extract_html_title, normalize_inline_text,
        truncate_utf8_bytes,
    };

    #[test]
    fn extract_html_title_returns_trimmed_single_line_title() {
        let body = "<html><head><title>  AxonRunner   Browser \n Tool </title></head></html>";
        let title = extract_html_title(body);
        assert_eq!(title.as_deref(), Some("AxonRunner Browser Tool"));
    }

    #[test]
    fn truncate_utf8_bytes_preserves_char_boundaries() {
        let text = "abcdéfgh";
        let truncated = truncate_utf8_bytes(text, 5);
        assert_eq!(truncated, "abcd");
    }

    #[test]
    fn count_case_insensitive_occurrences_counts_matches() {
        let hits = count_case_insensitive_occurrences("AxonRunner axonrunner AXIOM", "axonrunner");
        assert_eq!(hits, 3);
    }

    #[test]
    fn normalize_inline_text_collapses_whitespace() {
        let normalized = normalize_inline_text("one \n two\t three");
        assert_eq!(normalized, "one two three");
    }
}
