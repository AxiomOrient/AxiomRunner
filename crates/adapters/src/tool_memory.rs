use crate::contracts::{
    AdapterFuture, AdapterHealth, MemoryAdapter, MemoryEntry, ToolAdapter, ToolCall, ToolOutput,
};
use crate::error::{AdapterError, AdapterResult, RetryClass};
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryToolAction {
    Store,
    Recall,
    Forget,
}

impl MemoryToolAction {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "memory.store" | "memory_store" => Some(Self::Store),
            "memory.recall" | "memory_recall" => Some(Self::Recall),
            "memory.forget" | "memory_forget" => Some(Self::Forget),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryToolConfig {
    pub max_key_bytes: usize,
    pub max_value_bytes: usize,
    pub max_recall_limit: usize,
}

impl Default for MemoryToolConfig {
    fn default() -> Self {
        Self {
            max_key_bytes: 256,
            max_value_bytes: 4096,
            max_recall_limit: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemoryToolInput {
    action: MemoryToolAction,
    key: Option<String>,
    value: Option<String>,
    query: Option<String>,
    limit: usize,
}

impl MemoryToolInput {
    fn from_call(call: ToolCall, config: MemoryToolConfig) -> AdapterResult<Self> {
        let action = MemoryToolAction::parse(call.name.trim()).ok_or_else(|| {
            AdapterError::not_found(
                "memory_tool_action",
                if call.name.trim().is_empty() {
                    String::from("<empty>")
                } else {
                    call.name.clone()
                },
            )
        })?;

        let key = normalized_optional_arg(&call.args, "key", config.max_key_bytes)?;
        let value = normalized_optional_arg(&call.args, "value", config.max_value_bytes)?;
        let query = normalized_optional_arg(&call.args, "query", config.max_value_bytes)?;

        let limit = parse_limit(&call.args, config.max_recall_limit)?;

        match action {
            MemoryToolAction::Store => {
                if key.is_none() {
                    return Err(AdapterError::invalid_input(
                        "memory.key",
                        "store requires key",
                    ));
                }
                if value.is_none() {
                    return Err(AdapterError::invalid_input(
                        "memory.value",
                        "store requires value",
                    ));
                }
            }
            MemoryToolAction::Recall => {
                if key.is_none() && query.is_none() {
                    return Err(AdapterError::invalid_input(
                        "memory.query",
                        "recall requires key or query",
                    ));
                }
            }
            MemoryToolAction::Forget => {
                if key.is_none() {
                    return Err(AdapterError::invalid_input(
                        "memory.key",
                        "forget requires key",
                    ));
                }
            }
        }

        Ok(Self {
            action,
            key,
            value,
            query,
            limit,
        })
    }
}

/// In-memory implementation of `MemoryAdapter`.
/// Used as a test-double and as the default fallback backend for `MemoryToolAdapter`.
/// Data is not persisted across process restarts.
#[derive(Default)]
pub struct InMemoryMemoryAdapter {
    state: Mutex<InMemoryMemoryState>,
}

#[derive(Default)]
struct InMemoryMemoryState {
    records: BTreeMap<String, MemoryEntry>,
    counter: u64,
}

impl MemoryAdapter for InMemoryMemoryAdapter {
    fn id(&self) -> &str {
        "memory.in-memory"
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn store(&self, key: &str, value: &str) -> AdapterResult<()> {
        let mut state = self.backend_state("memory.store")?;
        state.counter += 1;
        let updated_at = state.counter;
        state.records.insert(
            key.to_string(),
            MemoryEntry {
                key: key.to_string(),
                value: value.to_string(),
                updated_at,
            },
        );
        Ok(())
    }

    fn recall(&self, query: &str, limit: usize) -> AdapterResult<Vec<MemoryEntry>> {
        let state = self.backend_state("memory.recall")?;
        let mut hits: Vec<MemoryEntry> = state
            .records
            .values()
            .filter(|e| e.key.contains(query) || e.value.contains(query))
            .cloned()
            .collect();
        hits.truncate(limit);
        Ok(hits)
    }

    fn get(&self, key: &str) -> AdapterResult<Option<MemoryEntry>> {
        let state = self.backend_state("memory.get")?;
        Ok(state.records.get(key).cloned())
    }

    fn list(&self) -> AdapterResult<Vec<MemoryEntry>> {
        let state = self.backend_state("memory.list")?;
        Ok(state.records.values().cloned().collect())
    }

    fn delete(&self, key: &str) -> AdapterResult<bool> {
        let mut state = self.backend_state("memory.delete")?;
        Ok(state.records.remove(key).is_some())
    }

    fn count(&self) -> AdapterResult<usize> {
        let state = self.backend_state("memory.count")?;
        Ok(state.records.len())
    }
}

impl InMemoryMemoryAdapter {
    fn backend_state(
        &self,
        operation: &'static str,
    ) -> AdapterResult<std::sync::MutexGuard<'_, InMemoryMemoryState>> {
        self.state.lock().map_err(|_| {
            AdapterError::failed(operation, "memory lock poisoned", RetryClass::NonRetryable)
        })
    }
}

pub struct MemoryToolAdapter {
    config: MemoryToolConfig,
    backend: Box<dyn MemoryAdapter>,
}

impl std::fmt::Debug for MemoryToolAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryToolAdapter")
            .field("config", &self.config)
            .field("backend", &"<dyn MemoryAdapter>")
            .finish()
    }
}

impl MemoryToolAdapter {
    /// Create a new `MemoryToolAdapter` with the given config and backend.
    pub fn new(config: MemoryToolConfig, backend: Box<dyn MemoryAdapter>) -> Self {
        Self { config, backend }
    }

    /// Create a `MemoryToolAdapter` backed by an in-process `InMemoryMemoryAdapter`.
    /// Useful as a default / test-double when no persistent backend is available.
    pub fn in_memory(config: MemoryToolConfig) -> Self {
        Self::new(config, Box::new(InMemoryMemoryAdapter::default()))
    }
}

impl ToolAdapter for MemoryToolAdapter {
    fn id(&self) -> &str {
        "tool.memory"
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn execute(&self, call: ToolCall) -> AdapterFuture<'_, ToolOutput> {
        Box::pin(async move {
            let input = MemoryToolInput::from_call(call, self.config)?;

            match input.action {
                MemoryToolAction::Store => {
                    let key = input
                        .key
                        .as_deref()
                        .ok_or_else(|| AdapterError::invalid_input("memory.key", "missing key"))?;
                    let value = input.value.as_deref().ok_or_else(|| {
                        AdapterError::invalid_input("memory.value", "missing value")
                    })?;
                    self.backend.store(key, value).map_err(|e| {
                        AdapterError::failed("memory.store", e.to_string(), RetryClass::Retryable)
                    })?;
                    Ok(ToolOutput {
                        content: format!("memory.store key={key} ok"),
                    })
                }
                MemoryToolAction::Recall => {
                    if let Some(q) = input.query.as_deref() {
                        let hits = self.backend.recall(q, input.limit).map_err(|e| {
                            AdapterError::failed(
                                "memory.recall",
                                e.to_string(),
                                RetryClass::Retryable,
                            )
                        })?;
                        if hits.is_empty() {
                            return Ok(ToolOutput {
                                content: format!("memory.recall query={q} hits=0"),
                            });
                        }
                        let items = hits
                            .iter()
                            .map(|e| format!("{}:{}", e.key, e.value))
                            .collect::<Vec<_>>()
                            .join("|");
                        Ok(ToolOutput {
                            content: format!(
                                "memory.recall query={q} hits={} items={items}",
                                hits.len()
                            ),
                        })
                    } else {
                        let key = input.key.as_deref().unwrap_or_default();
                        match self.backend.get(key).map_err(|e| {
                            AdapterError::failed("memory.get", e.to_string(), RetryClass::Retryable)
                        })? {
                            Some(entry) => Ok(ToolOutput {
                                content: format!("memory.recall key={key} value={}", entry.value),
                            }),
                            None => Ok(ToolOutput {
                                content: format!("memory.recall key={key} hits=0"),
                            }),
                        }
                    }
                }
                MemoryToolAction::Forget => {
                    let key = input
                        .key
                        .as_deref()
                        .ok_or_else(|| AdapterError::invalid_input("memory.key", "missing key"))?;
                    let removed = self.backend.delete(key).map_err(|e| {
                        AdapterError::failed("memory.forget", e.to_string(), RetryClass::Retryable)
                    })?;
                    Ok(ToolOutput {
                        content: format!("memory.forget key={key} removed={removed}"),
                    })
                }
            }
        })
    }
}

fn normalized_optional_arg(
    args: &BTreeMap<String, String>,
    key: &'static str,
    max_bytes: usize,
) -> AdapterResult<Option<String>> {
    let Some(raw) = args.get(key) else {
        return Ok(None);
    };

    let value = raw.trim();
    if value.is_empty() {
        return Err(AdapterError::invalid_input(key, "must not be empty"));
    }
    if value.len() > max_bytes {
        return Err(AdapterError::invalid_input(key, "value exceeds byte limit"));
    }

    Ok(Some(value.to_string()))
}

fn parse_limit(args: &BTreeMap<String, String>, max_limit: usize) -> AdapterResult<usize> {
    let Some(raw) = args.get("limit") else {
        return Ok(max_limit.min(5));
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AdapterError::invalid_input("limit", "must not be empty"));
    }

    let parsed = trimmed
        .parse::<usize>()
        .map_err(|_| AdapterError::invalid_input("limit", "must be a positive integer"))?;
    if parsed == 0 {
        return Err(AdapterError::invalid_input(
            "limit",
            "must be greater than zero",
        ));
    }
    if parsed > max_limit {
        return Err(AdapterError::invalid_input(
            "limit",
            "exceeds allowed maximum",
        ));
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::ToolCall;
    use std::collections::BTreeMap;
    use std::future::Future;

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should initialize")
            .block_on(future)
    }

    fn call(name: &str, args: &[(&str, &str)]) -> ToolCall {
        ToolCall::new(
            name,
            args.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    #[test]
    fn store_delegates_to_backend_and_recall_finds_entry() {
        let adapter = MemoryToolAdapter::in_memory(MemoryToolConfig::default());
        block_on(adapter.execute(call(
            "memory.store",
            &[("key", "greeting"), ("value", "hello world")],
        )))
        .expect("store ok");
        let out = block_on(adapter.execute(call("memory.recall", &[("key", "greeting")])))
            .expect("recall ok");
        assert!(
            out.content.contains("hello world"),
            "content={}",
            out.content
        );
    }

    #[test]
    fn forget_removes_entry_from_backend() {
        let adapter = MemoryToolAdapter::in_memory(MemoryToolConfig::default());
        block_on(adapter.execute(call("memory.store", &[("key", "tmp"), ("value", "data")])))
            .expect("store ok");
        let out =
            block_on(adapter.execute(call("memory.forget", &[("key", "tmp")]))).expect("forget ok");
        assert!(
            out.content.contains("removed=true"),
            "content={}",
            out.content
        );
    }

    #[test]
    fn recall_by_query_searches_all_entries() {
        let adapter = MemoryToolAdapter::in_memory(MemoryToolConfig::default());
        block_on(adapter.execute(call(
            "memory.store",
            &[("key", "a"), ("value", "apple pie")],
        )))
        .expect("ok");
        block_on(adapter.execute(call("memory.store", &[("key", "b"), ("value", "banana")])))
            .expect("ok");
        let out =
            block_on(adapter.execute(call("memory.recall", &[("query", "apple")]))).expect("ok");
        assert!(out.content.contains("apple"), "content={}", out.content);
        assert!(
            !out.content.contains("banana"),
            "should not match banana: {}",
            out.content
        );
    }

    #[test]
    fn custom_backend_receives_store_calls() {
        let backend = Box::new(InMemoryMemoryAdapter::default());
        let adapter = MemoryToolAdapter::new(MemoryToolConfig::default(), backend);
        block_on(adapter.execute(call("memory.store", &[("key", "x"), ("value", "42")])))
            .expect("ok");
        let out = block_on(adapter.execute(call("memory.recall", &[("key", "x")]))).expect("ok");
        assert!(out.content.contains("42"), "content={}", out.content);
    }
}
