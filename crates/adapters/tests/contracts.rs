use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axiom_adapters::runtime::{NativeRuntimeAdapter, RuntimeRequest};
use axiom_adapters::{
    AdapterError, AdapterFuture, AdapterHealth, AdapterResult, ChannelAdapter, ChannelMessage,
    ChannelSendReceipt, MemoryAdapter, MemoryEntry, ProviderAdapter, ProviderRequest,
    ProviderResponse, RuntimeAdapter, RuntimeState, RuntimeTick, ToolAdapter, ToolCall, ToolOutput,
    build_contract_memory, build_contract_provider,
};

struct EchoProvider;

impl ProviderAdapter for EchoProvider {
    fn id(&self) -> &str {
        "provider.echo"
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn complete(&self, request: ProviderRequest) -> AdapterFuture<'_, ProviderResponse> {
        Box::pin(async move {
            if request.prompt.trim().is_empty() {
                return Err(AdapterError::invalid_input("prompt", "must not be empty"));
            }

            Ok(ProviderResponse {
                content: format!("echo:{}", request.prompt),
            })
        })
    }
}

#[derive(Default)]
struct InMemoryStore {
    inner: Mutex<HashMap<String, MemoryEntry>>,
}

impl MemoryAdapter for InMemoryStore {
    fn id(&self) -> &str {
        "memory.in_memory"
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn store(&self, key: &str, value: &str) -> AdapterResult<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AdapterError::unavailable("memory", "lock poisoned"))?;
        if key.is_empty() {
            return Err(AdapterError::invalid_input("key", "must not be empty"));
        }
        inner.insert(
            key.to_owned(),
            MemoryEntry {
                key: key.to_owned(),
                value: value.to_owned(),
                updated_at: 0,
            },
        );
        Ok(())
    }

    fn recall(&self, query: &str, limit: usize) -> AdapterResult<Vec<MemoryEntry>> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| AdapterError::unavailable("memory", "lock poisoned"))?;
        let mut entries: Vec<MemoryEntry> = if query.trim().is_empty() {
            inner.values().cloned().collect()
        } else {
            inner
                .values()
                .filter(|e| e.key.contains(query) || e.value.contains(query))
                .cloned()
                .collect()
        };
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        if limit > 0 {
            entries.truncate(limit);
        }
        Ok(entries)
    }

    fn get(&self, key: &str) -> AdapterResult<Option<MemoryEntry>> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| AdapterError::unavailable("memory", "lock poisoned"))?;
        Ok(inner.get(key).cloned())
    }

    fn delete(&self, key: &str) -> AdapterResult<bool> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AdapterError::unavailable("memory", "lock poisoned"))?;
        Ok(inner.remove(key).is_some())
    }

    fn list(&self) -> AdapterResult<Vec<MemoryEntry>> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| AdapterError::unavailable("memory", "lock poisoned"))?;
        let mut entries: Vec<MemoryEntry> = inner.values().cloned().collect();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(entries)
    }

    fn count(&self) -> AdapterResult<usize> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| AdapterError::unavailable("memory", "lock poisoned"))?;
        Ok(inner.len())
    }
}

#[derive(Default)]
struct QueueChannel {
    queue: VecDeque<ChannelMessage>,
    sequence: u64,
}

impl ChannelAdapter for QueueChannel {
    fn id(&self) -> &str {
        "channel.queue"
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn send(&mut self, message: ChannelMessage) -> AdapterFuture<'_, ChannelSendReceipt> {
        Box::pin(async move {
            self.sequence += 1;
            self.queue.push_back(message);

            Ok(ChannelSendReceipt {
                sequence: self.sequence,
                accepted: true,
            })
        })
    }

    fn drain(&mut self) -> AdapterFuture<'_, Vec<ChannelMessage>> {
        Box::pin(async move {
            let mut drained = Vec::with_capacity(self.queue.len());
            while let Some(msg) = self.queue.pop_front() {
                drained.push(msg);
            }
            Ok(drained)
        })
    }
}

struct EchoTool;

impl ToolAdapter for EchoTool {
    fn id(&self) -> &str {
        "tool.echo"
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn execute(&self, call: ToolCall) -> AdapterFuture<'_, ToolOutput> {
        Box::pin(async move {
            if call.name != "echo" {
                return Err(AdapterError::not_found("tool", call.name));
            }

            let payload = call.args.get("payload").cloned().unwrap_or_default();

            Ok(ToolOutput {
                content: format!("ok:{payload}"),
            })
        })
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime should initialize")
        .block_on(future)
}

struct DeterministicRuntime {
    state: RuntimeState,
    step: u64,
}

impl Default for DeterministicRuntime {
    fn default() -> Self {
        Self {
            state: RuntimeState::Stopped,
            step: 0,
        }
    }
}

impl RuntimeAdapter for DeterministicRuntime {
    fn id(&self) -> &str {
        "runtime.deterministic"
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn start(&mut self) -> AdapterResult<()> {
        self.state = RuntimeState::Running;
        self.step = 0;
        Ok(())
    }

    fn tick(&mut self) -> AdapterResult<RuntimeTick> {
        if self.state != RuntimeState::Running {
            return Err(AdapterError::unavailable("runtime", "is not running"));
        }

        self.step += 1;
        Ok(RuntimeTick {
            step: self.step,
            state: self.state,
        })
    }

    fn stop(&mut self) -> AdapterResult<()> {
        self.state = RuntimeState::Stopped;
        Ok(())
    }

    fn state(&self) -> RuntimeState {
        self.state
    }
}

#[test]
fn contracts_provider_trait_object_smoke() {
    let provider: Box<dyn ProviderAdapter> = Box::new(EchoProvider);
    assert_eq!(provider.id(), "provider.echo");
    assert_eq!(provider.health(), AdapterHealth::Healthy);

    let response = provider.complete(ProviderRequest::new("test-model", "ping", 8));
    let response = block_on(response).expect("provider call should succeed");
    assert_eq!(response.content, "echo:ping");

    let error = provider.complete(ProviderRequest::new("test-model", " ", 8));
    let error = block_on(error).expect_err("empty prompt should be rejected");
    assert_eq!(
        error,
        AdapterError::invalid_input("prompt", "must not be empty")
    );
}

#[test]
fn contracts_memory_trait_object_roundtrip_semantics() {
    let memory: Box<dyn MemoryAdapter> = Box::new(InMemoryStore::default());
    assert_eq!(memory.id(), "memory.in_memory");
    assert_eq!(memory.health(), AdapterHealth::Healthy);

    memory.store("alpha", "42").expect("store should succeed");
    let found = memory.get("alpha").expect("get should succeed");
    assert_eq!(found.map(|e| e.value), Some(String::from("42")));

    let deleted = memory.delete("alpha").expect("delete should succeed");
    assert!(deleted);
    let after_delete = memory
        .get("alpha")
        .expect("get after delete should succeed");
    assert_eq!(after_delete, None);

    let err = memory.store("", "x").expect_err("empty key rejected");
    assert_eq!(err, AdapterError::invalid_input("key", "must not be empty"));
}

#[test]
fn contracts_memory_list_returns_all_entries() {
    let memory: Box<dyn MemoryAdapter> = Box::new(InMemoryStore::default());
    memory.store("alpha", "1").unwrap();
    memory.store("beta", "2").unwrap();

    let entries = memory.list().expect("list should succeed");
    assert_eq!(entries.len(), 2);
    // sorted by key
    assert_eq!(entries[0].key, "alpha");
    assert_eq!(entries[1].key, "beta");

    memory.delete("alpha").unwrap();
    let after = memory.list().unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].key, "beta");
}

#[test]
fn contracts_channel_trait_object_queue_semantics() {
    let mut channel: Box<dyn ChannelAdapter> = Box::new(QueueChannel::default());
    assert_eq!(channel.id(), "channel.queue");
    assert_eq!(channel.health(), AdapterHealth::Healthy);

    let first = channel.send(ChannelMessage::new("t", "m1"));
    let first = block_on(first).expect("first send should succeed");
    let second = channel.send(ChannelMessage::new("t", "m2"));
    let second = block_on(second).expect("second send should succeed");
    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 2);

    let drained = block_on(channel.drain()).expect("drain should succeed");
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].body, "m1");
    assert_eq!(drained[1].body, "m2");
    assert!(
        block_on(channel.drain())
            .expect("second drain should succeed")
            .is_empty()
    );
}

#[test]
fn contracts_tool_trait_object_execution_semantics() {
    let tool: Box<dyn ToolAdapter> = Box::new(EchoTool);
    assert_eq!(tool.id(), "tool.echo");
    assert_eq!(tool.health(), AdapterHealth::Healthy);

    let mut args = BTreeMap::new();
    args.insert(String::from("payload"), String::from("hello"));
    let ok = tool.execute(ToolCall::new("echo", args));
    let ok = block_on(ok).expect("known tool should execute");
    assert_eq!(ok.content, "ok:hello");

    let err = tool.execute(ToolCall::new("unknown", BTreeMap::new()));
    let err = block_on(err).expect_err("unknown tool should fail");
    assert_eq!(
        err,
        AdapterError::not_found("tool", String::from("unknown"))
    );
}

#[test]
fn contracts_runtime_trait_object_lifecycle_semantics() {
    let mut runtime: Box<dyn RuntimeAdapter> = Box::new(DeterministicRuntime::default());
    assert_eq!(runtime.id(), "runtime.deterministic");
    assert_eq!(runtime.health(), AdapterHealth::Healthy);
    assert_eq!(runtime.state(), RuntimeState::Stopped);

    let not_running = runtime.tick().expect_err("tick before start must fail");
    assert_eq!(
        not_running,
        AdapterError::unavailable("runtime", "is not running")
    );

    runtime.start().expect("runtime should start");
    assert_eq!(runtime.state(), RuntimeState::Running);

    let first = runtime.tick().expect("first tick should succeed");
    let second = runtime.tick().expect("second tick should succeed");
    assert_eq!(first.step, 1);
    assert_eq!(second.step, 2);

    runtime.stop().expect("runtime should stop");
    assert_eq!(runtime.state(), RuntimeState::Stopped);
}

fn unique_path(label: &str, extension: &str) -> PathBuf {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!(
        "axiom-contracts-{label}-{}-{tick}.{extension}",
        std::process::id()
    ))
}

#[test]
fn contracts_real_provider_registry_implements_provider_contract() {
    let provider = build_contract_provider("mock-local").expect("provider should build");
    assert_eq!(provider.id(), "mock-local");
    assert_eq!(provider.health(), AdapterHealth::Healthy);

    let response = provider.complete(ProviderRequest::new("test-model", "ping", 16));
    let response = block_on(response).expect("provider call should succeed");
    assert_eq!(response.content, "ping");

    let error = provider.complete(ProviderRequest::new("test-model", "ping", 0));
    let error = block_on(error).expect_err("zero max_tokens should be rejected");
    assert_eq!(
        error,
        AdapterError::invalid_input("max_tokens", "must be greater than zero")
    );
}

#[test]
fn contracts_real_markdown_memory_implements_memory_contract() {
    let path = unique_path("memory-markdown", "md");
    let memory =
        build_contract_memory("markdown", path.clone()).expect("markdown memory should initialize");

    memory.store("alpha", "42").expect("store should succeed");
    let found = memory.get("alpha").expect("get should succeed");
    assert_eq!(found.map(|e| e.value), Some(String::from("42")));
    let removed = memory.delete("alpha").expect("delete should succeed");
    assert!(removed);

    let _ = fs::remove_file(path);
}

#[cfg(unix)]
#[test]
fn contracts_real_runtime_native_adapter_implements_runtime_contract() {
    let mut runtime: Box<dyn RuntimeAdapter> = Box::new(NativeRuntimeAdapter::new(
        RuntimeRequest::new("sh")
            .with_args(["-c", "sleep 0.2"])
            .with_timeout(Duration::from_millis(500)),
    ));

    assert_eq!(runtime.id(), "runtime.native");
    assert_eq!(runtime.state(), RuntimeState::Stopped);

    runtime.start().expect("runtime should start");
    let first = runtime.tick().expect("tick should succeed");
    assert_eq!(first.state, RuntimeState::Running);

    runtime.stop().expect("runtime should stop");
    assert_eq!(runtime.state(), RuntimeState::Stopped);
}
