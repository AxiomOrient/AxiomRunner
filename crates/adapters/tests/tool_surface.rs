use std::collections::BTreeMap;
use std::future::Future;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use axonrunner_adapters::error::AdapterError;
use axonrunner_adapters::tool::{
    DEFAULT_TOOL_ID, ToolRegistryKind, build_contract_tool, resolve_tool_adapter_id,
    resolve_tool_id, tool_registry,
};
use axonrunner_adapters::tool_browser::{BrowserToolAdapter, BrowserToolConfig};
use axonrunner_adapters::{ToolAdapter, ToolCall};

fn args(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (key, value) in entries {
        map.insert((*key).to_string(), (*value).to_string());
    }
    map
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime should initialize")
        .block_on(future)
}

fn spawn_test_html_server(body: &'static str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let endpoint = format!("http://{addr}");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("server should accept one request");
        let mut request_buf = [0u8; 2048];
        let _ = stream.read(&mut request_buf);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("server should write response");
    });
    (endpoint, handle)
}

#[test]
fn tool_registry_includes_memory_and_browser_entries() {
    let ids: Vec<&str> = tool_registry().iter().map(|entry| entry.id).collect();

    assert!(ids.contains(&"memory"), "ids={ids:?}");
    assert!(ids.contains(&"browser"), "ids={ids:?}");

    let memory = tool_registry()
        .iter()
        .find(|entry| entry.id == "memory")
        .expect("memory entry should exist");
    let browser = tool_registry()
        .iter()
        .find(|entry| entry.id == "browser")
        .expect("browser entry should exist");

    assert_eq!(DEFAULT_TOOL_ID, "memory");
    assert_eq!(memory.kind, ToolRegistryKind::Memory);
    assert_eq!(browser.kind, ToolRegistryKind::Browser);
}

#[test]
fn tool_registry_resolves_aliases() {
    assert_eq!(resolve_tool_id("tool.memory"), Some("memory"));
    assert_eq!(resolve_tool_id("browser_open"), Some("browser"));
    assert_eq!(resolve_tool_id("tool.browser"), Some("browser"));
    assert_eq!(resolve_tool_adapter_id("memory"), Some("tool.memory"));
    assert_eq!(
        resolve_tool_adapter_id("tool.delegate"),
        Some("tool.delegate")
    );
    assert_eq!(resolve_tool_adapter_id("unknown"), None);
}

#[test]
fn memory_tool_roundtrip_store_recall_forget() {
    let tool = build_contract_tool("memory").expect("memory tool should build");

    let store = block_on(tool.execute(ToolCall::new(
        "memory.store",
        args(&[("key", "alpha"), ("value", "42")]),
    )))
    .expect("store should succeed");
    assert!(store.content.contains("memory.store key=alpha"));

    let recall = block_on(tool.execute(ToolCall::new(
        "memory.recall",
        args(&[("query", "alpha"), ("limit", "5")]),
    )))
    .expect("recall should succeed");
    assert!(
        recall.content.contains("hits=1"),
        "content={}",
        recall.content
    );
    assert!(
        recall.content.contains("alpha:42"),
        "content={}",
        recall.content
    );

    let forget = block_on(tool.execute(ToolCall::new("memory.forget", args(&[("key", "alpha")]))))
        .expect("forget should succeed");
    assert!(forget.content.contains("removed=true"));

    let after_forget =
        block_on(tool.execute(ToolCall::new("memory.recall", args(&[("query", "alpha")]))))
            .expect("recall should succeed after delete");
    assert!(after_forget.content.contains("hits=0"));
}

#[test]
fn memory_tool_rejects_invalid_inputs() {
    let tool = build_contract_tool("memory").expect("memory tool should build");

    let err = block_on(tool.execute(ToolCall::new("memory.store", args(&[("key", "alpha")]))))
        .expect_err("missing value should fail");
    assert_eq!(
        err,
        AdapterError::invalid_input("memory.value", "store requires value")
    );

    let err = block_on(tool.execute(ToolCall::new(
        "memory.recall",
        args(&[("query", "alpha"), ("limit", "0")]),
    )))
    .expect_err("zero limit should fail");
    assert_eq!(
        err,
        AdapterError::invalid_input("limit", "must be greater than zero")
    );
}

#[test]
fn browser_tool_opens_fetches_and_tracks_current() {
    let (endpoint, handle) = spawn_test_html_server(
        "<html><head><title>AxonRunner Browser Test</title></head><body>AxonRunner docs and axonrunner guide</body></html>",
    );
    let tool = BrowserToolAdapter::new(BrowserToolConfig {
        allowed_hosts: vec![String::from("127.0.0.1")],
        require_https: false,
        ..BrowserToolConfig::default()
    });

    let mut open_args = BTreeMap::new();
    open_args.insert(String::from("url"), format!("{endpoint}/docs"));
    let opened = block_on(tool.execute(ToolCall::new("browser.open", open_args)))
        .expect("open should fetch page");
    assert!(
        opened.content.contains("browser.open"),
        "{}",
        opened.content
    );
    assert!(opened.content.contains("status=200"), "{}", opened.content);
    assert!(
        opened.content.contains("title=AxonRunner Browser Test"),
        "{}",
        opened.content
    );

    let current = block_on(tool.execute(ToolCall::new("browser.current", BTreeMap::new())))
        .expect("current should succeed");
    assert!(
        current.content.contains(&format!("{endpoint}/docs")),
        "content={}",
        current.content
    );
    assert!(
        current.content.contains("status=200"),
        "{}",
        current.content
    );

    let find = block_on(tool.execute(ToolCall::new(
        "browser.find",
        args(&[("query", "axonrunner")]),
    )))
    .expect("find should work on loaded page");
    assert!(find.content.contains("hits=3"), "{}", find.content);

    handle.join().expect("server thread should complete");
}

#[test]
fn browser_tool_find_requires_loaded_page() {
    let tool = BrowserToolAdapter::new(BrowserToolConfig {
        allowed_hosts: vec![String::from("example.com")],
        require_https: true,
        ..BrowserToolConfig::default()
    });

    let err = block_on(tool.execute(ToolCall::new(
        "browser.find",
        args(&[("query", "anything")]),
    )))
    .expect_err("find should fail without a loaded page");
    assert_eq!(
        err,
        AdapterError::invalid_input("browser.current", "no page is currently loaded")
    );
}

#[test]
fn browser_tool_rejects_disallowed_or_non_https_urls() {
    let tool = build_contract_tool("browser").expect("browser tool should build");

    let disallowed = block_on(tool.execute(ToolCall::new(
        "browser.open",
        args(&[("url", "https://blocked.com")]),
    )))
    .expect_err("disallowed host should fail");
    assert_eq!(
        disallowed,
        AdapterError::invalid_input("browser.url", "host is not in allowlist")
    );

    let non_https = block_on(tool.execute(ToolCall::new(
        "browser.open",
        args(&[("url", "http://example.com")]),
    )))
    .expect_err("http url should fail");
    assert_eq!(
        non_https,
        AdapterError::invalid_input("browser.url", "https scheme is required")
    );
}
