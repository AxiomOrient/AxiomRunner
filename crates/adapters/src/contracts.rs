use crate::error::AdapterResult;
use serde::{Deserialize, Serialize};
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
        expected_replacements: Option<usize>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunCommandProfile {
    Generic,
    Build,
    Test,
    Lint,
}

impl RunCommandProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Build => "build",
            Self::Test => "test",
            Self::Lint => "lint",
        }
    }
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
    pub scanned_files: usize,
    pub skipped_files: usize,
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
    pub profile: RunCommandProfile,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPackContract {
    pub pack_id: String,
    pub version: String,
    pub description: String,
    pub entry_goal: String,
    pub planner_hints: Vec<String>,
    pub recommended_verifier_flow: Vec<RunCommandProfile>,
    pub allowed_tools: Vec<WorkflowPackAllowedTool>,
    pub verifier_rules: Vec<WorkflowPackVerifierRule>,
    pub risk_policy: WorkflowPackRiskPolicy,
}

impl WorkflowPackContract {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.pack_id.trim().is_empty() {
            return Err("pack_id");
        }
        if self.version.trim().is_empty() {
            return Err("version");
        }
        if self.entry_goal.trim().is_empty() {
            return Err("entry_goal");
        }
        if self.recommended_verifier_flow.is_empty() {
            return Err("recommended_verifier_flow");
        }
        if self.allowed_tools.is_empty() {
            return Err("allowed_tools");
        }
        if self.verifier_rules.is_empty() {
            return Err("verifier_rules");
        }
        if self
            .allowed_tools
            .iter()
            .any(|tool| tool.operation.trim().is_empty() || tool.scope.trim().is_empty())
        {
            return Err("allowed_tools.entry");
        }
        if self.verifier_rules.iter().any(|rule| {
            rule.label.trim().is_empty()
                || rule.command_example.trim().is_empty()
                || rule.artifact_expectation.trim().is_empty()
        }) {
            return Err("verifier_rules.entry");
        }
        if self.risk_policy.approval_mode.trim().is_empty() {
            return Err("risk_policy.approval_mode");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPackAllowedTool {
    pub operation: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPackVerifierRule {
    pub label: String,
    pub profile: RunCommandProfile,
    pub command_example: String,
    pub artifact_expectation: String,
    #[serde(default)]
    pub strength: WorkflowPackVerifierStrength,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPackVerifierStrength {
    #[default]
    Strong,
    Weak,
    Unresolved,
    PackRequired,
}

impl WorkflowPackVerifierStrength {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Weak => "weak",
            Self::Unresolved => "unresolved",
            Self::PackRequired => "pack_required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPackRiskPolicy {
    pub approval_mode: String,
    pub max_mutating_steps: u64,
}
