use crate::error::AdapterResult;
use std::future::Future;
use std::pin::Pin;

pub type AdapterFuture<'a, T> = Pin<Box<dyn Future<Output = AdapterResult<T>> + Send + 'a>>;

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

pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn health(&self) -> AdapterHealth;
    fn complete(&self, request: ProviderRequest) -> AdapterFuture<'_, ProviderResponse>;
}

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
