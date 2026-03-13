use crate::error::AdapterResult;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

pub type AdapterFuture<'a, T> = Pin<Box<dyn Future<Output = AdapterResult<T>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterHealth {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderHealthStatus {
    Ready,
    Degraded,
    Blocked,
}

impl ProviderHealthStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderHealthReport {
    pub status: ProviderHealthStatus,
    pub detail: String,
}

impl ProviderHealthReport {
    pub fn ready(detail: impl Into<String>) -> Self {
        Self {
            status: ProviderHealthStatus::Ready,
            detail: detail.into(),
        }
    }

    pub fn degraded(detail: impl Into<String>) -> Self {
        Self {
            status: ProviderHealthStatus::Degraded,
            detail: detail.into(),
        }
    }

    pub fn blocked(detail: impl Into<String>) -> Self {
        Self {
            status: ProviderHealthStatus::Blocked,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRequest {
    pub model: String,
    pub prompt: String,
    pub max_tokens: usize,
    pub cwd: String,
}

impl ProviderRequest {
    pub fn new(
        model: impl Into<String>,
        prompt: impl Into<String>,
        max_tokens: usize,
        cwd: impl Into<String>,
    ) -> Self {
        Self {
            model: model.into(),
            prompt: prompt.into(),
            max_tokens,
            cwd: cwd.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponse {
    pub content: String,
}

pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn health(&self) -> AdapterFuture<'_, ProviderHealthReport>;
    fn complete(&self, request: ProviderRequest) -> AdapterFuture<'_, ProviderResponse>;
    fn shutdown(&self) -> AdapterFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPolicy {
    pub max_file_write_bytes: usize,
    pub max_file_read_bytes: usize,
    pub max_search_results: usize,
    pub max_command_output_bytes: usize,
    pub command_timeout_ms: u64,
    pub command_allowlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolRequest {
    ListFiles {
        path: String,
    },
    ReadFile {
        path: String,
    },
    SearchFiles {
        path: String,
        needle: String,
        mode: SearchMode,
    },
    FileWrite {
        path: String,
        contents: String,
        append: bool,
    },
    ReplaceInFile {
        path: String,
        needle: String,
        replacement: String,
    },
    RemovePath {
        path: String,
    },
    RunCommand {
        program: String,
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Substring,
    Regex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolResult {
    ListFiles(ListFilesOutput),
    ReadFile(ReadFileOutput),
    SearchFiles(SearchFilesOutput),
    FileWrite(FileWriteOutput),
    ReplaceInFile(ReplaceInFileOutput),
    RemovePath(RemovePathOutput),
    RunCommand(RunCommandOutput),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListFilesOutput {
    pub base: PathBuf,
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadFileOutput {
    pub path: PathBuf,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub path: PathBuf,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFilesOutput {
    pub base: PathBuf,
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMutationEvidence {
    pub operation: String,
    pub artifact_path: PathBuf,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
    pub before_excerpt: Option<String>,
    pub after_excerpt: Option<String>,
    pub unified_diff: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWriteOutput {
    pub path: PathBuf,
    pub bytes_written: usize,
    pub evidence: FileMutationEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceInFileOutput {
    pub path: PathBuf,
    pub replacements: usize,
    pub evidence: FileMutationEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovePathOutput {
    pub path: PathBuf,
    pub removed: bool,
    pub evidence: Option<FileMutationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCommandOutput {
    pub program: String,
    pub args: Vec<String>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub artifact_path: PathBuf,
}

pub trait ToolAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn health(&self) -> AdapterHealth;
    fn execute(&self, request: ToolRequest) -> AdapterResult<ToolResult>;
}
