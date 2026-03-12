use crate::error::AdapterResult;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

pub type AdapterFuture<'a, T> = Pin<Box<dyn Future<Output = AdapterResult<T>> + Send + 'a>>;

/// Shared health states used by all adapter contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterHealth {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRequest {
    pub model: String,
    pub prompt: String,
    pub max_tokens: usize,
}

impl ProviderRequest {
    pub fn new(model: impl Into<String>, prompt: impl Into<String>, max_tokens: usize) -> Self {
        Self {
            model: model.into(),
            prompt: prompt.into(),
            max_tokens,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponse {
    pub content: String,
}

/// Provider contract for D checks:
/// stable identity, health probe, and request/response invocation.
pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn health(&self) -> AdapterHealth;
    fn complete(&self, request: ProviderRequest) -> AdapterFuture<'_, ProviderResponse>;
}

/// Memory contract for D checks:
/// stable identity, health probe, and key/value round-trip APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub updated_at: u64,
}

pub trait MemoryAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn health(&self) -> AdapterHealth;
    fn store(&self, key: &str, value: &str) -> AdapterResult<()>;
    fn recall(&self, query: &str, limit: usize) -> AdapterResult<Vec<MemoryEntry>>;
    fn get(&self, key: &str) -> AdapterResult<Option<MemoryEntry>>;
    fn list(&self) -> AdapterResult<Vec<MemoryEntry>>;
    fn delete(&self, key: &str) -> AdapterResult<bool>;
    fn count(&self) -> AdapterResult<usize>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMessage {
    pub topic: String,
    pub body: String,
}

impl ChannelMessage {
    pub fn new(topic: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            body: body.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSendReceipt {
    pub sequence: u64,
    pub accepted: bool,
}

/// Channel contract for D checks:
/// stable identity, health probe, and send/drain behavior.
pub trait ChannelAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn health(&self) -> AdapterHealth;
    fn send(&mut self, message: ChannelMessage) -> AdapterFuture<'_, ChannelSendReceipt>;
    fn drain(&mut self) -> AdapterFuture<'_, Vec<ChannelMessage>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub name: String,
    pub args: BTreeMap<String, String>,
}

impl ToolCall {
    pub fn new(name: impl Into<String>, args: BTreeMap<String, String>) -> Self {
        Self {
            name: name.into(),
            args,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    pub content: String,
}

/// Tool contract for D checks:
/// stable identity, health probe, and deterministic execution API.
pub trait ToolAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn health(&self) -> AdapterHealth;
    fn execute(&self, call: ToolCall) -> AdapterFuture<'_, ToolOutput>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRequest {
    pub cwd: String,
    pub prompt: String,
    pub model: Option<String>,
}

impl AgentRequest {
    pub fn new(cwd: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            prompt: prompt.into(),
            model: None,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResponse {
    pub content: String,
}

/// Agent contract for full agentic capabilities:
/// file system access, shell execution, tool use via codex app-server (coclai).
pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn health(&self) -> AdapterHealth;
    fn run(&self, request: AgentRequest) -> AdapterResult<AgentResponse>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Stopped,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTick {
    pub step: u64,
    pub state: RuntimeState,
}

/// Runtime contract for D checks:
/// stable identity, health probe, and lifecycle/tick semantics.
pub trait RuntimeAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn health(&self) -> AdapterHealth;
    fn start(&mut self) -> AdapterResult<()>;
    fn tick(&mut self) -> AdapterResult<RuntimeTick>;
    fn stop(&mut self) -> AdapterResult<()>;
    fn state(&self) -> RuntimeState;
}

/// A semantic context hit from AxiomSync search results.
/// Exposes score, snippet, and full URI — preserving AxiomSync's data model.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextHit {
    pub uri: String,
    pub score: f32,
    pub snippet: String, // short excerpt (maps to AxiomSync's abstract_text)
    pub content: String, // full document content (may be empty if not fetched)
}

/// A full context document stored under an AxiomSync URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextDocument {
    pub uri: String,
    pub content: String,
}

/// High-level context adapter: exposes AxiomSync's URI model and semantic search.
/// Distinct from MemoryAdapter (key/value) — this models DOCUMENTS and SESSIONS.
/// All methods are synchronous (AxiomSync has no async API).
pub trait ContextAdapter: Send + Sync {
    fn id(&self) -> &str;

    /// Store a document at the given URI.
    /// uri example: "axonrunner://agent/memory/my-fact"
    fn store_document(&self, uri: &str, content: &str) -> AdapterResult<()>;

    /// Semantic search over the given scope URI.
    /// Returns ranked hits with scores and snippets.
    fn semantic_search(
        &self,
        query: &str,
        scope_uri: &str,
        limit: usize,
    ) -> AdapterResult<Vec<ContextHit>>;

    /// Retrieve a full document by URI. Returns None if not found.
    fn get_document(&self, uri: &str) -> AdapterResult<Option<ContextDocument>>;

    /// Delete a document. Returns true if it existed.
    fn remove_document(&self, uri: &str) -> AdapterResult<bool>;

    /// Store a session message for conversation history tracking.
    /// uri_prefix example: "axonrunner://session/sess-001"
    fn store_session_message(
        &self,
        session_prefix_uri: &str,
        role: &str,
        content: &str,
    ) -> AdapterResult<()>;

    /// Semantic search within a specific session's context.
    fn recall_with_session(
        &self,
        query: &str,
        session_prefix_uri: &str,
        limit: usize,
    ) -> AdapterResult<Vec<ContextHit>>;
}
