use std::collections::BTreeMap;

use axiom_adapters::ToolCall;
use axiom_adapters::error::AdapterError;
use axiom_adapters::tool::{
    DEFAULT_TOOL_ID, ToolRegistryKind, build_contract_tool, resolve_tool_id, tool_registry,
};

fn args(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (key, value) in entries {
        map.insert((*key).to_string(), (*value).to_string());
    }
    map
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
}

#[test]
fn memory_tool_roundtrip_store_recall_forget() {
    let tool = build_contract_tool("memory").expect("memory tool should build");

    let store = tool
        .execute(ToolCall::new(
            "memory.store",
            args(&[("key", "alpha"), ("value", "42")]),
        ))
        .expect("store should succeed");
    assert!(store.content.contains("memory.store key=alpha"));

    let recall = tool
        .execute(ToolCall::new(
            "memory.recall",
            args(&[("query", "alpha"), ("limit", "5")]),
        ))
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

    let forget = tool
        .execute(ToolCall::new("memory.forget", args(&[("key", "alpha")])))
        .expect("forget should succeed");
    assert!(forget.content.contains("removed=true"));

    let after_forget = tool
        .execute(ToolCall::new("memory.recall", args(&[("query", "alpha")])))
        .expect("recall should succeed after delete");
    assert!(after_forget.content.contains("hits=0"));
}

#[test]
fn memory_tool_rejects_invalid_inputs() {
    let tool = build_contract_tool("memory").expect("memory tool should build");

    let err = tool
        .execute(ToolCall::new("memory.store", args(&[("key", "alpha")])))
        .expect_err("missing value should fail");
    assert_eq!(
        err,
        AdapterError::invalid_input("memory.value", "store requires value")
    );

    let err = tool
        .execute(ToolCall::new(
            "memory.recall",
            args(&[("query", "alpha"), ("limit", "0")]),
        ))
        .expect_err("zero limit should fail");
    assert_eq!(
        err,
        AdapterError::invalid_input("limit", "must be greater than zero")
    );
}

#[test]
fn browser_tool_opens_allowed_url_and_tracks_current() {
    let tool = build_contract_tool("browser").expect("browser tool should build");

    let opened = tool
        .execute(ToolCall::new(
            "browser.open",
            args(&[("url", "https://example.com/docs")]),
        ))
        .expect("open should succeed for allowed host");
    assert!(opened.content.contains("browser.open"));
    assert!(opened.content.contains("host=example.com"));

    let current = tool
        .execute(ToolCall::new("browser.current", BTreeMap::new()))
        .expect("current should succeed");
    assert!(
        current.content.contains("https://example.com/docs"),
        "content={}",
        current.content
    );
}

#[test]
fn browser_tool_rejects_disallowed_or_non_https_urls() {
    let tool = build_contract_tool("browser").expect("browser tool should build");

    let disallowed = tool
        .execute(ToolCall::new(
            "browser.open",
            args(&[("url", "https://blocked.com")]),
        ))
        .expect_err("disallowed host should fail");
    assert_eq!(
        disallowed,
        AdapterError::invalid_input("browser.url", "host is not in allowlist")
    );

    let non_https = tool
        .execute(ToolCall::new(
            "browser.open",
            args(&[("url", "http://example.com")]),
        ))
        .expect_err("http url should fail");
    assert_eq!(
        non_https,
        AdapterError::invalid_input("browser.url", "https scheme is required")
    );
}
