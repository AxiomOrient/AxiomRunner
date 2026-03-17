use crate::async_runtime_host::global_async_runtime_host;
use crate::cli_command::RunTemplate;
use crate::config_loader::AppConfig;
use crate::display::outcome_name;
use crate::env_util::read_env_trimmed;
use axiomrunner_adapters::{
    FileMutationEvidence, FileWriteOutput, MemoryAdapter, MemoryTier, ProviderAdapter,
    ProviderRequest, RunCommandClass, RunCommandOutput, ToolAdapter, ToolPolicy, ToolRequest,
    ToolResult, ToolRiskTier, WorkspaceTool, build_contract_memory, build_contract_provider,
    classify_run_command_class, classify_tool_request_risk, provider_registry, resolve_provider_id,
    tiered_memory_key,
};
use axiomrunner_core::{DecisionOutcome, PolicyCode, RunConstraintPolicyKey};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

mod artifacts;
mod plan;

pub use self::plan::RuntimeRunPlan;

use self::plan::{
    MemoryPlan, ProviderPlan, RuntimeComposePlan, ToolCommandPlan, ToolPlan,
    build_runtime_compose_plan, build_runtime_run_plan, goal_verifier_tool_plan,
};

const ENV_RUNTIME_MEMORY_PATH: &str = "AXIOMRUNNER_RUNTIME_MEMORY_PATH";
const ENV_RUNTIME_TOOL_WORKSPACE: &str = "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE";
const ENV_RUNTIME_ARTIFACT_WORKSPACE: &str = "AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE";
const ENV_RUNTIME_GIT_WORKTREE_ISOLATION: &str = "AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION";
const ENV_RUNTIME_TOOL_LOG_PATH: &str = "AXIOMRUNNER_RUNTIME_TOOL_LOG_PATH";
const ENV_RUNTIME_PROVIDER: &str = "AXIOMRUNNER_RUNTIME_PROVIDER";
const ENV_RUNTIME_PROVIDER_MODEL: &str = "AXIOMRUNNER_RUNTIME_PROVIDER_MODEL";
const ENV_RUNTIME_MAX_TOKENS: &str = "AXIOMRUNNER_RUNTIME_MAX_TOKENS";
const ENV_RUNTIME_COMMAND_ALLOWLIST: &str = "AXIOMRUNNER_RUNTIME_COMMAND_ALLOWLIST";
const ENV_RUNTIME_COMMAND_TIMEOUT_MS: &str = "AXIOMRUNNER_RUNTIME_COMMAND_TIMEOUT_MS";

const DEFAULT_TOOL_LOG_PATH: &str = "runtime.log";
const DEFAULT_MAX_TOKENS: usize = 4096;
const DEFAULT_TOOL_COMMAND_TIMEOUT_MS: u64 = 30_000;
const TOOL_WRITE_LIMIT_BYTES: usize = 16 * 1024;
const TOOL_READ_LIMIT_BYTES: usize = 64 * 1024;
const TOOL_MAX_SEARCH_RESULTS: usize = 64;
const TOOL_MAX_COMMAND_OUTPUT_BYTES: usize = 32 * 1024;
const HOT_CONTEXT_MAX_CHARS: usize = 512;
const HOT_CONTEXT_MAX_PATHS: usize = 4;
const HOT_CONTEXT_MAX_OUTPUTS: usize = 3;
const REPO_DOC_MAX_CHARS: usize = 512;
const REPO_DOC_FILENAMES: [(&str, &str); 4] = [
    ("SPEC", "SPEC.md"),
    ("PLAN", "PLAN.md"),
    ("STATUS", "STATUS.md"),
    ("AGENTS", "AGENTS.md"),
];

pub const APPROVAL_STATE_REQUIRED: &str = "required";
pub const APPROVAL_STATE_NOT_REQUIRED: &str = "not_required";
pub const RUN_REASON_OPERATOR_ABORT: &str = "operator_abort";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComposeConfig {
    pub memory_path: Option<PathBuf>,
    pub tool_workspace: Option<PathBuf>,
    pub artifact_workspace: Option<PathBuf>,
    pub git_worktree_isolation: bool,
    pub tool_log_path: String,
    pub provider_id: String,
    pub provider_model: String,
    pub max_tokens: usize,
    pub command_timeout_ms: u64,
    pub command_allowlist: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComposeHealth {
    pub provider_id: String,
    pub provider_model: String,
    pub provider: RuntimeComposeComponentHealth,
    pub memory: RuntimeComposeComponentHealth,
    pub tool: RuntimeComposeComponentHealth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComposeComponentHealth {
    pub enabled: bool,
    pub state: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeComposeStep {
    Skipped,
    Applied,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComposeExecution {
    pub provider_output: Option<String>,
    pub provider_cwd: String,
    pub provider: RuntimeComposeStep,
    pub memory: RuntimeComposeStep,
    pub tool: RuntimeComposeStep,
    pub tool_outputs: Vec<String>,
    pub patch_artifacts: Vec<RuntimeComposePatchArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunVerification {
    pub status: &'static str,
    pub summary: String,
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunRepair {
    pub attempted: bool,
    pub attempts: usize,
    pub status: &'static str,
    pub summary: String,
    pub tool: RuntimeComposeStep,
    pub tool_outputs: Vec<String>,
    pub patch_artifacts: Vec<RuntimeComposePatchArtifact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRunPhase {
    Planning,
    ExecutingStep,
    Verifying,
    Repairing,
    WaitingApproval,
    Blocked,
    Completed,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRunOutcome {
    Success,
    Blocked,
    BudgetExhausted,
    ApprovalRequired,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunRecord {
    pub plan: RuntimeRunPlan,
    pub step_journal: Vec<RuntimeRunStepRecord>,
    pub verification: RuntimeRunVerification,
    pub repair: RuntimeRunRepair,
    pub checkpoint: Option<RuntimeRunCheckpointMetadata>,
    pub rollback: Option<RuntimeRunRollbackMetadata>,
    pub elapsed_ms: u64,
    pub phase: RuntimeRunPhase,
    pub outcome: RuntimeRunOutcome,
    pub reason: String,
    pub reason_code: String,
    pub reason_detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunCheckpointMetadata {
    pub metadata_path: String,
    pub restore_path: String,
    pub execution_workspace: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunRollbackMetadata {
    pub metadata_path: String,
    pub restore_path: String,
    pub cleanup_path: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunStepRecord {
    pub id: String,
    pub label: String,
    pub phase: String,
    pub status: String,
    pub evidence: String,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComposePatchArtifact {
    pub operation: String,
    pub target_path: String,
    pub artifact_path: String,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
    pub before_excerpt: Option<String>,
    pub after_excerpt: Option<String>,
    pub unified_diff: Option<String>,
}

pub struct ReportWriteInput<'a> {
    pub intent_id: &'a str,
    pub outcome: DecisionOutcome,
    pub policy_code: &'a str,
    pub effect_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintPolicyViolation {
    pub code: PolicyCode,
    pub reason: String,
}

impl RuntimeComposeExecution {
    pub fn first_failure(&self) -> Option<(&'static str, &str)> {
        match &self.provider {
            RuntimeComposeStep::Failed(message) => Some(("provider", message.as_str())),
            _ => match &self.memory {
                RuntimeComposeStep::Failed(message) => Some(("memory", message.as_str())),
                _ => match &self.tool {
                    RuntimeComposeStep::Failed(message) => Some(("tool", message.as_str())),
                    _ => None,
                },
            },
        }
    }

    pub fn with_repair(&self, repair: &RuntimeRunRepair) -> Self {
        if !repair.attempted || !matches!(repair.tool, RuntimeComposeStep::Applied) {
            return self.clone();
        }

        Self {
            tool: RuntimeComposeStep::Applied,
            tool_outputs: self
                .tool_outputs
                .iter()
                .chain(&repair.tool_outputs)
                .cloned()
                .collect(),
            patch_artifacts: self
                .patch_artifacts
                .iter()
                .chain(&repair.patch_artifacts)
                .cloned()
                .collect(),
            ..self.clone()
        }
    }
}

impl RuntimeRunRepair {
    pub fn skipped(summary: impl Into<String>) -> Self {
        Self {
            attempted: false,
            attempts: 0,
            status: "skipped",
            summary: summary.into(),
            tool: RuntimeComposeStep::Skipped,
            tool_outputs: Vec::new(),
            patch_artifacts: Vec::new(),
        }
    }
}

impl RuntimeComposeConfig {
    pub fn from_app_config(config: &AppConfig) -> Self {
        let provider_id =
            env_string(ENV_RUNTIME_PROVIDER).unwrap_or_else(|| config.provider.clone());
        let provider_model = env_string(ENV_RUNTIME_PROVIDER_MODEL)
            .or_else(|| config.provider_model.clone())
            .unwrap_or_else(|| provider_id.clone());

        let memory_path = env_path(ENV_RUNTIME_MEMORY_PATH).or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".axiomrunner").join("memory.db"))
        });
        let tool_workspace =
            env_path(ENV_RUNTIME_TOOL_WORKSPACE).or_else(|| config.workspace.clone());
        let artifact_workspace =
            env_path(ENV_RUNTIME_ARTIFACT_WORKSPACE).or_else(|| tool_workspace.clone());
        let git_worktree_isolation = env_bool(ENV_RUNTIME_GIT_WORKTREE_ISOLATION);

        let tool_log_path = env_string(ENV_RUNTIME_TOOL_LOG_PATH).unwrap_or_else(|| {
            artifact_workspace
                .as_ref()
                .or(tool_workspace.as_ref())
                .map(|workspace| workspace.join(DEFAULT_TOOL_LOG_PATH).display().to_string())
                .unwrap_or_else(|| DEFAULT_TOOL_LOG_PATH.to_owned())
        });
        let max_tokens = env_string(ENV_RUNTIME_MAX_TOKENS)
            .and_then(|raw| raw.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_TOKENS);
        let command_timeout_ms = env_string(ENV_RUNTIME_COMMAND_TIMEOUT_MS)
            .and_then(|raw| raw.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_TOOL_COMMAND_TIMEOUT_MS);
        let command_allowlist = env_string(ENV_RUNTIME_COMMAND_ALLOWLIST)
            .and_then(|raw| parse_command_allowlist(&raw))
            .or_else(|| config.command_allowlist.clone());
        Self {
            memory_path,
            tool_workspace,
            artifact_workspace,
            git_worktree_isolation,
            tool_log_path,
            provider_id,
            provider_model,
            max_tokens,
            command_timeout_ms,
            command_allowlist,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeComposeInitState {
    Disabled,
    Ready(String),
    Failed(String),
}

impl RuntimeComposeInitState {
    fn state_name(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Ready(_) => "ready",
            Self::Failed(_) => "failed",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Disabled => String::from("not_configured"),
            Self::Ready(detail) | Self::Failed(detail) => detail.clone(),
        }
    }
}

pub struct RuntimeComposeState {
    config: RuntimeComposeConfig,
    base_tool_workspace: Option<PathBuf>,
    memory: Option<Box<dyn MemoryAdapter>>,
    memory_init: RuntimeComposeInitState,
    provider: Arc<dyn ProviderAdapter>,
    tool: Option<Arc<dyn ToolAdapter>>,
    artifact_tool: Option<Arc<dyn ToolAdapter>>,
    tool_init: RuntimeComposeInitState,
    agents_path: Option<PathBuf>,
    repo_docs: RepoDocStack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepoDocEntry {
    label: &'static str,
    path: PathBuf,
    contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct RepoDocStack {
    entries: Vec<RepoDocEntry>,
}

fn try_init_component<T>(
    option: Option<(String, Result<T, String>)>,
) -> (Option<T>, RuntimeComposeInitState) {
    match option {
        Some((detail, Ok(value))) => (Some(value), RuntimeComposeInitState::Ready(detail)),
        Some((detail, Err(error))) => (
            None,
            RuntimeComposeInitState::Failed(format!("{detail} error={error}")),
        ),
        None => (None, RuntimeComposeInitState::Disabled),
    }
}

impl RuntimeComposeState {
    pub fn new(mut config: RuntimeComposeConfig) -> Result<Self, String> {
        let provider_id = resolve_provider_id(&config.provider_id).ok_or_else(|| {
            let supported = provider_registry()
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "unknown runtime provider '{}'. set AXIOMRUNNER_RUNTIME_PROVIDER to one of: {supported}",
                config.provider_id.trim()
            )
        })?;
        config.provider_id = provider_id.to_owned();

        if config.tool_workspace.is_none() {
            return Err(String::from(
                "runtime tool workspace is not configured. set --workspace or AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
            ));
        }

        if let Some(path) = &config.memory_path
            && let Some(parent) = path.parent()
        {
            let _ = fs::create_dir_all(parent);
        }
        if let Some(workspace) = &config.tool_workspace {
            let _ = fs::create_dir_all(workspace);
        }
        if let Some(workspace) = &config.artifact_workspace {
            let _ = fs::create_dir_all(workspace);
        }
        if let Some(workspace) = config.tool_workspace.as_mut()
            && let Ok(canonical) = fs::canonicalize(&*workspace)
        {
            *workspace = canonical;
        }
        if let Some(workspace) = config.artifact_workspace.as_mut()
            && let Ok(canonical) = fs::canonicalize(&*workspace)
        {
            *workspace = canonical;
        }

        let (memory, memory_init) = try_init_component(config.memory_path.as_ref().map(|path| {
            let backend = if path.extension().and_then(|ext| ext.to_str()) == Some("db") {
                "sqlite"
            } else {
                "markdown"
            };
            (
                format!("path={}", path.display()),
                build_contract_memory(backend, path),
            )
        }));

        let (tool, artifact_tool, tool_init) = match (
            config.tool_workspace.as_ref(),
            config.artifact_workspace.as_ref(),
        ) {
            (Some(workspace), Some(artifact_workspace)) => {
                let detail = if workspace == artifact_workspace {
                    format!(
                        "workspace={},command_timeout_ms={}",
                        workspace.display(),
                        config.command_timeout_ms
                    )
                } else {
                    format!(
                        "workspace={},artifact_workspace={},command_timeout_ms={}",
                        workspace.display(),
                        artifact_workspace.display(),
                        config.command_timeout_ms
                    )
                };
                match build_tool_adapter(
                    workspace,
                    artifact_workspace,
                    config.command_timeout_ms,
                    config.command_allowlist.clone(),
                ) {
                    Ok(tool) => match build_tool_adapter(
                        artifact_workspace,
                        artifact_workspace,
                        config.command_timeout_ms,
                        config.command_allowlist.clone(),
                    ) {
                        Ok(artifact_tool) => (
                            Some(tool),
                            Some(artifact_tool),
                            RuntimeComposeInitState::Ready(detail),
                        ),
                        Err(error) => (
                            None,
                            None,
                            RuntimeComposeInitState::Failed(format!("{detail} error={error}")),
                        ),
                    },
                    Err(error) => (
                        None,
                        None,
                        RuntimeComposeInitState::Failed(format!("{detail} error={error}")),
                    ),
                }
            }
            _ => (None, None, RuntimeComposeInitState::Disabled),
        };

        let provider = Arc::from(
            build_contract_provider(provider_id)
                .map_err(|error| format!("provider init failed for '{provider_id}': {error}"))?,
        );
        let agents_path = find_agents_md(config.tool_workspace.as_deref());
        let repo_docs = ingest_repo_docs_stack(config.tool_workspace.as_deref());

        let base_tool_workspace = config.tool_workspace.clone();

        Ok(Self {
            config,
            base_tool_workspace,
            memory,
            memory_init,
            provider,
            tool: tool.map(|tool| Arc::new(tool) as Arc<dyn ToolAdapter>),
            artifact_tool: artifact_tool.map(|tool| Arc::new(tool) as Arc<dyn ToolAdapter>),
            tool_init,
            agents_path,
            repo_docs,
        })
    }

    pub fn apply_template(
        &mut self,
        template: &RunTemplate,
        outcome: DecisionOutcome,
    ) -> RuntimeComposeExecution {
        self.apply_plan(build_runtime_compose_plan(template, outcome))
    }

    pub fn constraint_policy_violation(
        &self,
        template: &RunTemplate,
    ) -> Option<ConstraintPolicyViolation> {
        constraint_policy_violation(template)
    }

    pub fn plan_template(
        &self,
        template: &RunTemplate,
        run_id: &str,
        intent_id: &str,
    ) -> RuntimeRunPlan {
        build_runtime_run_plan(template, run_id, intent_id)
    }

    pub fn prepare_execution_workspace(&mut self, run_id: &str) -> Result<(), String> {
        let Some(base_workspace) = self.base_tool_workspace.clone() else {
            return Ok(());
        };
        if !self.config.git_worktree_isolation {
            return self.rebind_execution_workspace(base_workspace);
        }

        let Some(repo_root) = discover_git_toplevel(&base_workspace)? else {
            return self.rebind_execution_workspace(base_workspace);
        };
        let relative_workspace = base_workspace
            .strip_prefix(&repo_root)
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let worktree_root =
            isolated_worktree_root(run_id, &repo_root, self.config.artifact_workspace.as_ref());
        ensure_git_worktree(&repo_root, &worktree_root)?;
        let execution_workspace = if relative_workspace.as_os_str().is_empty() {
            worktree_root
        } else {
            worktree_root.join(relative_workspace)
        };
        self.rebind_execution_workspace(execution_workspace)
    }

    pub fn repair_template(
        &self,
        template: &RunTemplate,
        _intent_id: &str,
        outcome: DecisionOutcome,
        prior: &RuntimeComposeExecution,
    ) -> RuntimeRunRepair {
        if !matches!(prior.tool, RuntimeComposeStep::Failed(_)) {
            return RuntimeRunRepair::skipped("tool_step_not_failed");
        }

        let plan = build_runtime_compose_plan(template, outcome);
        let goal_tool = goal_verifier_tool_plan(&plan);
        if goal_tool.is_none() && plan.tool.is_none() {
            return RuntimeRunRepair::skipped("no_tool_plan");
        }

        let (tool, tool_outputs, patch_artifacts) =
            self.execute_tool(goal_tool.or(plan.tool), prior.provider_output.as_deref());
        let status = match &tool {
            RuntimeComposeStep::Applied => "repaired",
            RuntimeComposeStep::Failed(_) => "failed",
            RuntimeComposeStep::Skipped => "skipped",
        };
        let summary = match &tool {
            RuntimeComposeStep::Applied => String::from("tool_step_retried_successfully"),
            RuntimeComposeStep::Failed(message) => format!("tool_repair_failed:{message}"),
            RuntimeComposeStep::Skipped => String::from("tool_repair_skipped"),
        };

        RuntimeRunRepair {
            attempted: true,
            attempts: 1,
            status,
            summary,
            tool,
            tool_outputs,
            patch_artifacts,
        }
    }

    pub fn remember_run_summary(
        &mut self,
        run: &RuntimeRunRecord,
        execution: &RuntimeComposeExecution,
        intent_id: &str,
    ) -> Result<(), String> {
        let Some(memory) = self.memory.as_mut() else {
            return Ok(());
        };

        let key = tiered_memory_key(MemoryTier::Recall, &format!("last_run/{intent_id}"));
        let value = Self::compact_hot_context(
            &artifact_aware_hot_context_head(run, execution, intent_id),
            &artifact_aware_hot_context_tail(
                run,
                self.agents_path.as_ref(),
                self.repo_docs.summary(),
            ),
        );
        memory
            .store(&key, &value)
            .map_err(|error| format!("store recall summary failed: {error}"))
    }

    fn compact_hot_context(head: &[String], tail: &[String]) -> String {
        let mut segments = head.to_vec();
        let mut remaining = HOT_CONTEXT_MAX_CHARS.saturating_sub(16);

        for entry in &segments {
            remaining = remaining.saturating_sub(entry.chars().count() + 3);
        }

        let mut omitted = 0usize;
        for entry in tail {
            let entry_len = entry.chars().count() + 3;
            if entry_len <= remaining {
                segments.push(entry.clone());
                remaining = remaining.saturating_sub(entry_len);
            } else {
                omitted = omitted.saturating_add(1);
            }
        }

        let mut joined = segments.join(" | ");
        if omitted > 0 {
            if !joined.is_empty() {
                joined.push_str(" | ");
            }
            joined.push_str(&format!("...<compacted:{}>", omitted));
        }
        if joined.chars().count() <= HOT_CONTEXT_MAX_CHARS + 20 {
            return joined;
        }

        let mut compact = joined
            .chars()
            .take(HOT_CONTEXT_MAX_CHARS.saturating_sub(16))
            .collect::<String>();
        compact.push_str("...<compacted>");
        compact
    }

    pub fn health(&self) -> RuntimeComposeHealth {
        let provider = probe_provider_health(Arc::clone(&self.provider));
        RuntimeComposeHealth {
            provider_id: self.config.provider_id.clone(),
            provider_model: self.config.provider_model.clone(),
            provider,
            memory: RuntimeComposeComponentHealth {
                enabled: self.memory.is_some(),
                state: self.memory_init.state_name(),
                detail: self.memory_init.detail(),
            },
            tool: RuntimeComposeComponentHealth {
                enabled: self.tool.is_some(),
                state: self.tool_init.state_name(),
                detail: match &self.agents_path {
                    Some(path) => format!("{},agents={}", self.tool_init.detail(), path.display()),
                    None => self.tool_init.detail(),
                },
            },
        }
    }

    pub fn workspace_root(&self) -> Option<&PathBuf> {
        self.base_tool_workspace
            .as_ref()
            .or(self.config.tool_workspace.as_ref())
    }

    pub fn max_tokens(&self) -> usize {
        self.config.max_tokens
    }

    pub fn write_report(
        &self,
        template: &RunTemplate,
        input: &ReportWriteInput<'_>,
        execution: &RuntimeComposeExecution,
        run: &RuntimeRunRecord,
    ) -> Result<Vec<RuntimeComposePatchArtifact>, String> {
        artifacts::write_report(self, template, input, execution, run)
    }

    pub fn idle_execution(&self) -> RuntimeComposeExecution {
        RuntimeComposeExecution {
            provider_output: None,
            provider_cwd: self.provider_cwd(),
            provider: RuntimeComposeStep::Skipped,
            memory: RuntimeComposeStep::Skipped,
            tool: RuntimeComposeStep::Skipped,
            tool_outputs: Vec::new(),
            patch_artifacts: Vec::new(),
        }
    }

    pub fn write_checkpoint_metadata(
        &self,
        intent_id: &str,
        run_id: &str,
    ) -> Result<Option<RuntimeRunCheckpointMetadata>, String> {
        artifacts::write_checkpoint_metadata(self, intent_id, run_id)
    }

    pub fn write_rollback_metadata(
        &self,
        intent_id: &str,
        execution: &RuntimeComposeExecution,
        run: &RuntimeRunRecord,
    ) -> Result<Option<RuntimeRunRollbackMetadata>, String> {
        artifacts::write_rollback_metadata(self, intent_id, execution, run)
    }

    fn apply_plan(&mut self, plan: RuntimeComposePlan) -> RuntimeComposeExecution {
        let goal_tool = goal_verifier_tool_plan(&plan);
        let (provider_output, provider) = self.execute_provider(plan.provider);
        if matches!(provider, RuntimeComposeStep::Failed(_)) {
            return RuntimeComposeExecution {
                provider_output,
                provider_cwd: self.provider_cwd(),
                provider,
                memory: RuntimeComposeStep::Skipped,
                tool: RuntimeComposeStep::Skipped,
                tool_outputs: Vec::new(),
                patch_artifacts: Vec::new(),
            };
        }

        let memory = self.execute_memory(plan.memory);
        if matches!(memory, RuntimeComposeStep::Failed(_)) {
            return RuntimeComposeExecution {
                provider_output,
                provider_cwd: self.provider_cwd(),
                provider,
                memory,
                tool: RuntimeComposeStep::Skipped,
                tool_outputs: Vec::new(),
                patch_artifacts: Vec::new(),
            };
        }

        let (tool, tool_outputs, patch_artifacts) =
            self.execute_tool(goal_tool.or(plan.tool), provider_output.as_deref());
        RuntimeComposeExecution {
            provider_output,
            provider_cwd: self.provider_cwd(),
            provider,
            memory,
            tool,
            tool_outputs,
            patch_artifacts,
        }
    }

    fn execute_provider(&self, plan: Option<ProviderPlan>) -> (Option<String>, RuntimeComposeStep) {
        let Some(plan) = plan else {
            return (None, RuntimeComposeStep::Skipped);
        };

        let provider = Arc::clone(&self.provider);
        let prompt = self.repo_docs.enrich_prompt(plan.prompt);
        let request =
            ProviderRequest::new(plan.model, prompt, plan.max_tokens, self.provider_cwd());
        match complete_provider_request(provider, request) {
            Ok(content) => (Some(content), RuntimeComposeStep::Applied),
            Err(error) => (None, RuntimeComposeStep::Failed(error)),
        }
    }

    fn execute_memory(&mut self, plan: MemoryPlan) -> RuntimeComposeStep {
        match plan {
            MemoryPlan::None => RuntimeComposeStep::Skipped,
        }
    }

    fn execute_tool(
        &self,
        plan: Option<ToolPlan>,
        _provider_output: Option<&str>,
    ) -> (
        RuntimeComposeStep,
        Vec<String>,
        Vec<RuntimeComposePatchArtifact>,
    ) {
        let Some(plan) = plan else {
            return (RuntimeComposeStep::Skipped, Vec::new(), Vec::new());
        };
        match plan {
            ToolPlan::RunCommands { commands } => {
                let Some(tool) = self.tool.as_ref() else {
                    return (RuntimeComposeStep::Skipped, Vec::new(), Vec::new());
                };
                let mut outputs = Vec::new();
                for command in commands {
                    match execute_verifier_command(tool.as_ref(), command) {
                        Ok(output) => outputs.push(output),
                        Err((failure, output)) => {
                            if let Some(output) = output {
                                outputs.push(output);
                            }
                            return (RuntimeComposeStep::Failed(failure), outputs, Vec::new());
                        }
                    }
                }
                (RuntimeComposeStep::Applied, outputs, Vec::new())
            }
        }
    }

    pub fn shutdown(&self) -> Result<(), String> {
        shutdown_provider(Arc::clone(&self.provider))
    }

    fn provider_cwd(&self) -> String {
        self.config
            .tool_workspace
            .as_ref()
            .and_then(|path| path.to_str().map(str::to_owned))
            .unwrap_or_default()
    }

    fn rebind_execution_workspace(&mut self, workspace: PathBuf) -> Result<(), String> {
        if self.config.tool_workspace.as_ref() == Some(&workspace) {
            return Ok(());
        }
        let artifact_workspace = self
            .config
            .artifact_workspace
            .clone()
            .unwrap_or_else(|| workspace.clone());
        let tool = build_tool_adapter(
            &workspace,
            &artifact_workspace,
            self.config.command_timeout_ms,
            self.config.command_allowlist.clone(),
        )?;
        self.config.tool_workspace = Some(workspace);
        self.tool = Some(Arc::new(tool) as Arc<dyn ToolAdapter>);
        Ok(())
    }
}

fn execute_verifier_command(
    tool: &dyn ToolAdapter,
    command: ToolCommandPlan,
) -> Result<String, (String, Option<String>)> {
    match tool.execute(ToolRequest::RunCommand {
        program: command.program.clone(),
        args: command.args.clone(),
    }) {
        Ok(ToolResult::RunCommand(output)) if output.exit_code == 0 => {
            Ok(render_verifier_evidence(&command, &output))
        }
        Ok(ToolResult::RunCommand(output)) => {
            let output_line = render_verifier_evidence(&command, &output);
            Err((
                format!(
                    "runtime_compose.tool.verifier_failed: label={} exit_code={} artifact={}",
                    command.label,
                    output.exit_code,
                    output.artifact_path.display()
                ),
                Some(output_line),
            ))
        }
        Ok(_) => Ok(String::new()),
        Err(error) => Err((format!("runtime_compose.tool.run_command: {error}"), None)),
    }
}

fn render_verifier_evidence(command: &ToolCommandPlan, output: &RunCommandOutput) -> String {
    serde_json::json!({
        "label": command.label,
        "profile": output.profile.as_str(),
        "strength": command.strength.as_str(),
        "exit_code": output.exit_code,
        "command": render_tool_command_line(&command.program, &command.args),
        "artifact_path": output.artifact_path.display().to_string(),
        "expectation": command.expectation,
        "stdout_summary": compact_verifier_output(&output.stdout),
        "stderr_summary": compact_verifier_output(&output.stderr),
        "stdout_truncated": output.stdout_truncated,
        "stderr_truncated": output.stderr_truncated,
    })
    .to_string()
}

fn compact_verifier_output(output: &str) -> String {
    let normalized = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\\n");
    if normalized.is_empty() {
        return String::from("none");
    }
    const MAX_CHARS: usize = 160;
    if normalized.chars().count() <= MAX_CHARS {
        return normalized;
    }
    let mut compact = normalized.chars().take(MAX_CHARS).collect::<String>();
    compact.push_str("...");
    compact
}

fn render_tool_command_line(program: &str, args: &[String]) -> String {
    if args.is_empty() {
        return program.to_owned();
    }
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_tool_adapter(
    workspace: &Path,
    artifact_workspace: &Path,
    command_timeout_ms: u64,
    command_allowlist: Option<Vec<String>>,
) -> Result<WorkspaceTool, String> {
    fs::create_dir_all(workspace)
        .map_err(|error| format!("create workspace '{}' failed: {error}", workspace.display()))?;
    fs::create_dir_all(artifact_workspace).map_err(|error| {
        format!(
            "create artifact workspace '{}' failed: {error}",
            artifact_workspace.display()
        )
    })?;

    WorkspaceTool::new(
        workspace,
        artifact_workspace,
        ToolPolicy {
            max_file_write_bytes: TOOL_WRITE_LIMIT_BYTES,
            max_file_read_bytes: TOOL_READ_LIMIT_BYTES,
            max_search_results: TOOL_MAX_SEARCH_RESULTS,
            max_command_output_bytes: TOOL_MAX_COMMAND_OUTPUT_BYTES,
            command_timeout_ms,
            command_allowlist: command_allowlist.unwrap_or_else(default_command_allowlist),
        },
    )
    .map_err(|error| format!("tool adapter init failed: {error}"))
}

fn constraint_policy_violation(template: &RunTemplate) -> Option<ConstraintPolicyViolation> {
    let plan = build_runtime_compose_plan(template, DecisionOutcome::Accepted);
    let Some(ToolPlan::RunCommands { commands }) = goal_verifier_tool_plan(&plan) else {
        return None;
    };

    for constraint in &template.goal.constraints {
        let Some(policy_key) = constraint.policy_key() else {
            continue;
        };
        match policy_key {
            RunConstraintPolicyKey::PathScope => {
                let allowed = parse_path_scope_constraint(&constraint.detail);
                for command in &commands {
                    if !path_scope_allows_command(command, &allowed) {
                        return Some(ConstraintPolicyViolation {
                            code: PolicyCode::ConstraintPathScope,
                            reason: format!(
                                "path_scope blocks verifier `{}` outside {}",
                                command.label,
                                allowed.join(",")
                            ),
                        });
                    }
                }
            }
            RunConstraintPolicyKey::DestructiveCommandClass => {
                if constraint.detail.trim().eq_ignore_ascii_case("deny") {
                    for command in &commands {
                        if classify_run_command_class(&command.program)
                            == RunCommandClass::Destructive
                        {
                            return Some(ConstraintPolicyViolation {
                                code: PolicyCode::ConstraintDestructiveCommands,
                                reason: format!(
                                    "destructive_commands blocks verifier `{}` program `{}`",
                                    command.label, command.program
                                ),
                            });
                        }
                    }
                }
            }
            RunConstraintPolicyKey::ExternalCommandClass => {
                if constraint.detail.trim().eq_ignore_ascii_case("deny") {
                    for command in &commands {
                        if classify_run_command_class(&command.program) == RunCommandClass::External
                        {
                            return Some(ConstraintPolicyViolation {
                                code: PolicyCode::ConstraintExternalCommands,
                                reason: format!(
                                    "external_commands blocks verifier `{}` program `{}`",
                                    command.label, command.program
                                ),
                            });
                        }
                    }
                }
            }
            RunConstraintPolicyKey::ApprovalEscalation => {}
        }
    }

    None
}

pub fn constraint_requires_pre_execution_approval(template: &RunTemplate) -> bool {
    let plan = build_runtime_compose_plan(template, DecisionOutcome::Accepted);
    let Some(ToolPlan::RunCommands { commands }) = goal_verifier_tool_plan(&plan) else {
        return false;
    };
    let escalation_required = template.goal.constraints.iter().any(|constraint| {
        matches!(
            constraint.policy_key(),
            Some(RunConstraintPolicyKey::ApprovalEscalation)
        ) && constraint.detail.trim().eq_ignore_ascii_case("required")
    });
    escalation_required
        && commands.iter().any(|command| {
            classify_tool_request_risk(&ToolRequest::RunCommand {
                program: command.program.clone(),
                args: command.args.clone(),
            }) == ToolRiskTier::High
        })
}

fn parse_path_scope_constraint(detail: &str) -> Vec<String> {
    let values = detail
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value == "workspace" {
                String::from(".")
            } else {
                value.trim_matches('/').to_owned()
            }
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        vec![String::from(".")]
    } else {
        values
    }
}

fn path_scope_allows_command(command: &ToolCommandPlan, allowed: &[String]) -> bool {
    command_scope_candidates(command)
        .into_iter()
        .all(|candidate| {
            allowed.iter().any(|prefix| {
                prefix == "."
                    || candidate == "."
                    || candidate == *prefix
                    || candidate.starts_with(&format!("{prefix}/"))
            })
        })
}

fn command_scope_candidates(command: &ToolCommandPlan) -> Vec<String> {
    let explicit = match command.program.as_str() {
        "ls" | "cat" | "rg" => command
            .args
            .iter()
            .filter(|arg| !arg.starts_with('-'))
            .map(|arg| arg.trim_matches('/').to_owned())
            .collect::<Vec<_>>(),
        _ if command.program.starts_with("./") || command.program.starts_with("../") => {
            vec![command.program.trim_matches('/').to_owned()]
        }
        _ => Vec::new(),
    };

    if explicit.is_empty() {
        vec![String::from(".")]
    } else {
        explicit
    }
}

fn discover_git_toplevel(workspace: &Path) -> Result<Option<PathBuf>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| {
            format!(
                "probe git workspace '{}' failed: {error}",
                workspace.display()
            )
        })?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("git top-level output was not utf8: {error}"))?;
    let path = stdout.trim();
    if path.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(path)))
    }
}

fn isolated_worktree_root(
    run_id: &str,
    repo_root: &Path,
    artifact_workspace: Option<&PathBuf>,
) -> PathBuf {
    let parent = artifact_workspace
        .filter(|path| !path.starts_with(repo_root))
        .map(|path| path.join(".axiomrunner").join("worktrees"))
        .unwrap_or_else(|| std::env::temp_dir().join("axiomrunner-worktrees"));
    parent.join(sanitize_worktree_segment(run_id))
}

fn sanitize_worktree_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect()
}

fn ensure_git_worktree(repo_root: &Path, worktree_root: &Path) -> Result<(), String> {
    if worktree_root.join(".git").exists() {
        return Ok(());
    }
    if worktree_root.exists() {
        fs::remove_dir_all(worktree_root).map_err(|error| {
            format!(
                "remove stale worktree '{}' failed: {error}",
                worktree_root.display()
            )
        })?;
    }
    if let Some(parent) = worktree_root.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create worktree parent '{}' failed: {error}",
                parent.display()
            )
        })?;
    }

    let worktree_arg = worktree_root.to_string_lossy().into_owned();
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["worktree", "add", "--detach", "--force"])
        .arg(&worktree_arg)
        .arg("HEAD")
        .output()
        .map_err(|error| {
            format!(
                "create git worktree '{}' failed: {error}",
                worktree_root.display()
            )
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git worktree add failed repo={} worktree={} stderr={}",
            repo_root.display(),
            worktree_root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn find_agents_md(start: Option<&Path>) -> Option<PathBuf> {
    find_repo_doc(start, "AGENTS.md")
}

fn find_repo_doc(start: Option<&Path>, name: &str) -> Option<PathBuf> {
    let mut current = start?.to_path_buf();
    loop {
        let candidate = current.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn ingest_repo_docs_stack(start: Option<&Path>) -> RepoDocStack {
    let entries = REPO_DOC_FILENAMES
        .iter()
        .filter_map(|(label, filename)| {
            let path = find_repo_doc(start, filename)?;
            let contents = fs::read_to_string(&path).ok()?;
            let contents = compact_repo_doc(&contents);
            if contents.is_empty() {
                None
            } else {
                Some(RepoDocEntry {
                    label,
                    path,
                    contents,
                })
            }
        })
        .collect::<Vec<_>>();
    RepoDocStack { entries }
}

fn compact_repo_doc(contents: &str) -> String {
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() <= REPO_DOC_MAX_CHARS {
        return trimmed.to_owned();
    }

    let mut compact = trimmed
        .chars()
        .take(REPO_DOC_MAX_CHARS.saturating_sub(16))
        .collect::<String>();
    compact.push_str("...<compacted>");
    compact
}

fn escape_json_string(value: &str) -> String {
    value.chars().flat_map(|ch| ch.escape_default()).collect()
}

fn artifact_aware_hot_context_head(
    run: &RuntimeRunRecord,
    execution: &RuntimeComposeExecution,
    intent_id: &str,
) -> Vec<String> {
    let mut head = vec![
        format!("run_id={}", run.plan.run_id),
        format!("phase={}", run_phase_name(run.phase)),
        format!("outcome={}", run_outcome_name(run.outcome)),
        format!("reason={}", run.reason),
        format!(
            "artifacts=plan=.axiomrunner/artifacts/{intent_id}.plan.md,verify=.axiomrunner/artifacts/{intent_id}.verify.md,report=.axiomrunner/artifacts/{intent_id}.report.md"
        ),
        format!(
            "changed_paths={}",
            compact_string_list(
                &execution
                    .patch_artifacts
                    .iter()
                    .map(|artifact| artifact.target_path.clone())
                    .collect::<Vec<_>>(),
                HOT_CONTEXT_MAX_PATHS,
            )
        ),
    ];

    if !execution.tool_outputs.is_empty() {
        head.push(format!(
            "tool_outputs={}",
            compact_string_list(&execution.tool_outputs, HOT_CONTEXT_MAX_OUTPUTS)
        ));
    }
    if let Some((stage, message)) = execution.first_failure() {
        head.push(format!("first_failure={stage}:{message}"));
    }
    if let Some(rollback) = &run.rollback {
        head.push(format!(
            "rollback=restore:{},metadata:{}",
            rollback.restore_path, rollback.metadata_path
        ));
    }

    head
}

fn artifact_aware_hot_context_tail(
    run: &RuntimeRunRecord,
    agents_path: Option<&PathBuf>,
    repo_docs_summary: String,
) -> Vec<String> {
    vec![
        format!("goal={}", run.plan.goal),
        format!("summary={}", run.plan.summary),
        format!(
            "agents={}",
            agents_path
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| String::from("none"))
        ),
        format!("repo_docs={repo_docs_summary}"),
    ]
}

fn compact_string_list(items: &[String], max_items: usize) -> String {
    if items.is_empty() {
        return String::from("none");
    }
    let mut parts = items.iter().take(max_items).cloned().collect::<Vec<_>>();
    if items.len() > max_items {
        parts.push(format!("+{}more", items.len() - max_items));
    }
    parts.join(",")
}

impl RepoDocStack {
    fn summary(&self) -> String {
        if self.entries.is_empty() {
            return String::from("none");
        }

        self.entries
            .iter()
            .map(|entry| format!("{}={}", entry.label, entry.path.display()))
            .collect::<Vec<_>>()
            .join(" | ")
    }

    fn enrich_prompt(&self, prompt: String) -> String {
        if self.entries.is_empty() {
            return prompt;
        }

        let guidance = self
            .entries
            .iter()
            .map(|entry| {
                format!(
                    "[{} path={}]\n{}",
                    entry.label,
                    entry.path.display(),
                    entry.contents
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        format!("repo_docs:\n{guidance}\n\nrequest:\n{prompt}")
    }
}

fn default_command_allowlist() -> Vec<String> {
    vec![
        String::from("pwd"),
        String::from("git"),
        String::from("cargo"),
        String::from("npm"),
        String::from("node"),
        String::from("python3"),
        String::from("rg"),
        String::from("ls"),
        String::from("cat"),
    ]
}

fn parse_command_allowlist(raw: &str) -> Option<Vec<String>> {
    let values = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn complete_provider_request(
    provider: Arc<dyn ProviderAdapter>,
    request: ProviderRequest,
) -> Result<String, String> {
    global_async_runtime_host()?.block_on_async("runtime_compose.provider.complete", async move {
        provider
            .complete(request)
            .await
            .map(|response| response.content)
            .map_err(|error| format!("runtime_compose.provider.complete: {error}"))
    })
}

fn shutdown_provider(provider: Arc<dyn ProviderAdapter>) -> Result<(), String> {
    global_async_runtime_host()?.block_on_async("runtime_compose.provider.shutdown", async move {
        provider
            .shutdown()
            .await
            .map_err(|error| format!("runtime_compose.provider.shutdown: {error}"))
    })
}

fn probe_provider_health(provider: Arc<dyn ProviderAdapter>) -> RuntimeComposeComponentHealth {
    match global_async_runtime_host().and_then(|host| {
        host.block_on_async("runtime_compose.provider.health", async move {
            provider
                .health()
                .await
                .map_err(|error| format!("runtime_compose.provider.health: {error}"))
        })
    }) {
        Ok(report) => RuntimeComposeComponentHealth {
            enabled: true,
            state: report.status.as_str(),
            detail: report.detail,
        },
        Err(error) => RuntimeComposeComponentHealth {
            enabled: true,
            state: "blocked",
            detail: error,
        },
    }
}

fn template_kind(_template: &RunTemplate) -> &'static str {
    "goal"
}

pub fn run_phase_name(phase: RuntimeRunPhase) -> &'static str {
    match phase {
        RuntimeRunPhase::Planning => "planning",
        RuntimeRunPhase::ExecutingStep => "executing_step",
        RuntimeRunPhase::Verifying => "verifying",
        RuntimeRunPhase::Repairing => "repairing",
        RuntimeRunPhase::WaitingApproval => "waiting_approval",
        RuntimeRunPhase::Blocked => "blocked",
        RuntimeRunPhase::Completed => "completed",
        RuntimeRunPhase::Failed => "failed",
        RuntimeRunPhase::Aborted => "aborted",
    }
}

pub fn run_outcome_name(outcome: RuntimeRunOutcome) -> &'static str {
    match outcome {
        RuntimeRunOutcome::Success => "success",
        RuntimeRunOutcome::Blocked => "blocked",
        RuntimeRunOutcome::BudgetExhausted => "budget_exhausted",
        RuntimeRunOutcome::ApprovalRequired => "approval_required",
        RuntimeRunOutcome::Failed => "failed",
        RuntimeRunOutcome::Aborted => "aborted",
    }
}

pub fn render_run_reason(code: &str, detail: &str) -> String {
    if detail == "none" || detail.is_empty() {
        code.to_owned()
    } else {
        format!("{code}:{detail}")
    }
}

/// Returns the verifier strength label for a given `verification.status` value.
///
/// In this runtime `verification.status` encodes both execution outcome and verifier quality
/// in a single vocabulary ("passed", "verification_weak", "pack_required", …).  The report
/// artifact and the operator output therefore carry the same value under the `verifier_strength`
/// key.  This function makes that derivation explicit at every call site so that future
/// divergence (e.g. mapping "passed" → "strong") can be applied in one place.
pub fn verifier_strength_label(verification_status: &str) -> &str {
    verification_status
}

pub fn runtime_run_reason(code: &str, detail: impl Into<String>) -> (String, String, String) {
    let detail = detail.into();
    let rendered = render_run_reason(code, &detail);
    (rendered, code.to_owned(), detail)
}

/// Maps a terminal outcome string to an operator-facing next action hint.
///
/// This is the single authoritative mapping for outcome → next_action used by
/// both the report artifact and operator replay output.
pub fn next_action_for_outcome(outcome: &str) -> &'static str {
    match outcome {
        "success" => "review report and replay evidence",
        "approval_required" => "approve and resume the pending run",
        "budget_exhausted" => "raise budget or reduce planned scope",
        "blocked" => "inspect verifier summary and unblock the run",
        "failed" => "inspect failure boundary and repair before retry",
        "aborted" => "decide whether to restart with a new run",
        _ => "inspect replay evidence",
    }
}

pub fn run_next_action(run: &RuntimeRunRecord) -> &'static str {
    next_action_for_outcome(run_outcome_name(run.outcome))
}

pub(crate) fn step_name(step: &RuntimeComposeStep) -> &'static str {
    match step {
        RuntimeComposeStep::Skipped => "skipped",
        RuntimeComposeStep::Applied => "applied",
        RuntimeComposeStep::Failed(_) => "failed",
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    env_string(key).map(PathBuf::from)
}

fn env_bool(key: &str) -> bool {
    matches!(
        env_string(key).as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
    )
}

fn env_string(key: &str) -> Option<String> {
    read_env_trimmed(key).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_TOOL_COMMAND_TIMEOUT_MS, RuntimeComposeConfig, RuntimeComposeExecution,
        RuntimeComposeState, RuntimeComposeStep, build_tool_adapter, run_outcome_name,
        run_phase_name,
    };
    use crate::cli_command::GoalFileTemplate;
    use crate::config_loader::AppConfig;
    use axiomrunner_adapters::{ToolAdapter, ToolRequest};
    use axiomrunner_core::{
        DecisionOutcome, DoneCondition, DoneConditionEvidence, RunApprovalMode, RunBudget,
        RunGoal, VerificationCheck,
    };
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_dir(label: &str) -> std::path::PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiomrunner-runtime-compose-test-{label}-{}-{tick}",
            std::process::id()
        ))
    }

    fn sample_goal_template() -> GoalFileTemplate {
        GoalFileTemplate {
            path: String::from("GOAL.json"),
            goal: RunGoal {
                summary: String::from("goal"),
                workspace_root: String::from("/workspace"),
                constraints: Vec::new(),
                done_conditions: vec![DoneCondition {
                    label: String::from("report"),
                    evidence: DoneConditionEvidence::ReportArtifactExists,
                }],
                verification_checks: vec![VerificationCheck {
                    label: String::from("workspace"),
                    detail: String::from("ls ."),
                }],
                budget: RunBudget::bounded(5, 10, 8000),
                approval_mode: RunApprovalMode::Never,
            },
            workflow_pack: None,
        }
    }

    #[test]
    fn runtime_compose_config_carries_command_allowlist_from_app_config() {
        let config = AppConfig {
            profile: String::from("prod"),
            provider: String::from("mock-local"),
            provider_model: Some(String::from("mock-local")),
            workspace: None,
            state_path: None,
            command_allowlist: Some(vec![String::from("git"), String::from("cargo")]),
        };

        let compose = RuntimeComposeConfig::from_app_config(&config);
        assert_eq!(
            compose.command_allowlist,
            Some(vec![String::from("git"), String::from("cargo")])
        );
    }

    #[test]
    fn build_tool_adapter_respects_configured_command_allowlist() {
        let workspace = unique_dir("allowlist");
        fs::create_dir_all(&workspace).expect("workspace should exist");

        let tool = build_tool_adapter(
            &workspace,
            &workspace,
            DEFAULT_TOOL_COMMAND_TIMEOUT_MS,
            Some(vec![String::from("pwd")]),
        )
        .expect("tool adapter should build");
        let pwd = tool.execute(ToolRequest::RunCommand {
            program: String::from("pwd"),
            args: Vec::new(),
        });
        let git = tool.execute(ToolRequest::RunCommand {
            program: String::from("git"),
            args: vec![String::from("status")],
        });

        assert!(pwd.is_ok());
        assert!(git.is_err());

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn runtime_compose_repair_retries_failed_tool_step() {
        let workspace = unique_dir("repair");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        let state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path: None,
            tool_workspace: Some(workspace.clone()),
            artifact_workspace: Some(workspace.clone()),
            git_worktree_isolation: false,
            tool_log_path: workspace.join("runtime.log").display().to_string(),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 256,
            command_timeout_ms: DEFAULT_TOOL_COMMAND_TIMEOUT_MS,
            command_allowlist: None,
        })
        .expect("runtime compose state should init");

        let prior = RuntimeComposeExecution {
            provider_output: Some(String::from("provider-ok")),
            provider_cwd: String::from("/tmp/workspace"),
            provider: RuntimeComposeStep::Applied,
            memory: RuntimeComposeStep::Applied,
            tool: RuntimeComposeStep::Failed(String::from("boom")),
            tool_outputs: Vec::new(),
            patch_artifacts: Vec::new(),
        };
        let repair = state.repair_template(
            &sample_goal_template(),
            "cli-repair",
            DecisionOutcome::Accepted,
            &prior,
        );

        assert!(repair.attempted);
        assert_eq!(repair.status, "repaired");
        assert!(matches!(repair.tool, RuntimeComposeStep::Applied));
        assert!(!repair.tool_outputs.is_empty());
        assert!(repair.patch_artifacts.is_empty());

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn runtime_compose_repair_skips_non_tool_failures() {
        let workspace = unique_dir("repair-skip");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        let state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path: None,
            tool_workspace: Some(workspace.clone()),
            artifact_workspace: Some(workspace.clone()),
            git_worktree_isolation: false,
            tool_log_path: workspace.join("runtime.log").display().to_string(),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 256,
            command_timeout_ms: DEFAULT_TOOL_COMMAND_TIMEOUT_MS,
            command_allowlist: None,
        })
        .expect("runtime compose state should init");

        let prior = RuntimeComposeExecution {
            provider_output: None,
            provider_cwd: String::from("/tmp/workspace"),
            provider: RuntimeComposeStep::Failed(String::from("provider-down")),
            memory: RuntimeComposeStep::Skipped,
            tool: RuntimeComposeStep::Skipped,
            tool_outputs: Vec::new(),
            patch_artifacts: Vec::new(),
        };
        let repair = state.repair_template(
            &sample_goal_template(),
            "cli-repair-skip",
            DecisionOutcome::Accepted,
            &prior,
        );

        assert!(!repair.attempted);
        assert_eq!(repair.status, "skipped");
        assert_eq!(repair.summary, "tool_step_not_failed");

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn runtime_compose_execution_carries_canonical_provider_workspace_binding() {
        let workspace = unique_dir("provider-cwd");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        let canonical_workspace = fs::canonicalize(&workspace).unwrap_or(workspace.clone());
        let mut state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path: None,
            tool_workspace: Some(workspace.clone()),
            artifact_workspace: Some(workspace.clone()),
            git_worktree_isolation: false,
            tool_log_path: workspace.join("runtime.log").display().to_string(),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 256,
            command_timeout_ms: DEFAULT_TOOL_COMMAND_TIMEOUT_MS,
            command_allowlist: None,
        })
        .expect("runtime compose state should init");

        let execution = state.apply_template(&sample_goal_template(), DecisionOutcome::Accepted);

        assert_eq!(
            execution.provider_cwd,
            canonical_workspace.display().to_string()
        );

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn run_phase_and_outcome_names_cover_terminal_values() {
        assert_eq!(
            run_phase_name(super::RuntimeRunPhase::Completed),
            "completed"
        );
        assert_eq!(
            run_phase_name(super::RuntimeRunPhase::WaitingApproval),
            "waiting_approval"
        );
        assert_eq!(
            run_outcome_name(super::RuntimeRunOutcome::Success),
            "success"
        );
        assert_eq!(
            run_outcome_name(super::RuntimeRunOutcome::ApprovalRequired),
            "approval_required"
        );
        assert_eq!(
            run_outcome_name(super::RuntimeRunOutcome::Aborted),
            "aborted"
        );
    }

    #[test]
    fn run_reason_schema_extracts_code_and_detail() {
        let (reason, code, detail) = super::runtime_run_reason("verification_passed", "none");
        assert_eq!(reason, "verification_passed");
        assert_eq!(code, "verification_passed");
        assert_eq!(detail, "none");

        let (reason, code, detail) =
            super::runtime_run_reason("repair_budget_exhausted", "attempts=1/1");
        assert_eq!(reason, "repair_budget_exhausted:attempts=1/1");
        assert_eq!(code, "repair_budget_exhausted");
        assert_eq!(detail, "attempts=1/1");
    }

    #[test]
    fn compact_hot_context_keeps_long_summaries_bounded() {
        let head = vec![
            String::from("run_id=run-1"),
            String::from("artifacts=report"),
        ];
        let tail = (0..64)
            .map(|index| format!("entry-{index}-{}", "x".repeat(32)))
            .collect::<Vec<_>>();
        let compact = RuntimeComposeState::compact_hot_context(&head, &tail);

        assert!(compact.len() <= super::HOT_CONTEXT_MAX_CHARS + 16);
        assert!(compact.contains("run_id=run-1"));
        assert!(compact.contains("artifacts=report"));
        assert!(compact.contains("...<compacted"));
    }

    #[test]
    fn compact_hot_context_keeps_artifact_pointers_before_tail_context() {
        let compact = RuntimeComposeState::compact_hot_context(
            &[
                String::from("run_id=run-1"),
                String::from(
                    "artifacts=plan=.axiomrunner/artifacts/cli-1.plan.md,verify=.axiomrunner/artifacts/cli-1.verify.md,report=.axiomrunner/artifacts/cli-1.report.md",
                ),
                String::from("changed_paths=src/lib.rs,src/main.rs,+4more"),
            ],
            &[
                format!("summary={}", "x".repeat(512)),
                String::from("repo_docs=SPEC.md"),
            ],
        );

        assert!(compact.contains(".report.md"));
        assert!(compact.contains("changed_paths=src/lib.rs"));
        assert!(compact.contains("...<compacted"));
    }

    #[test]
    fn runtime_compose_finds_agents_guidance_in_parent_workspace() {
        let root = unique_dir("agents-root");
        let nested = root.join("workspace").join("inner");
        fs::create_dir_all(&nested).expect("nested workspace should exist");
        fs::write(root.join("AGENTS.md"), "repo guidance").expect("agents file should exist");

        let found = super::find_agents_md(Some(&nested)).expect("agents path should be found");

        assert_eq!(found, root.join("AGENTS.md"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_compose_repo_doc_ingest_uses_parent_workspace_files() {
        let root = unique_dir("repo-doc-parent");
        let nested = root.join("workspace").join("inner");
        fs::create_dir_all(&nested).expect("nested workspace should exist");
        fs::write(root.join("SPEC.md"), "shared spec").expect("spec doc should exist");
        fs::write(root.join("AGENTS.md"), "shared agents").expect("agents doc should exist");

        let stack = super::ingest_repo_docs_stack(Some(&nested));

        assert_eq!(stack.entries.len(), 2);
        assert_eq!(stack.entries[0].label, "SPEC");
        assert_eq!(stack.entries[0].path, root.join("SPEC.md"));
        assert_eq!(stack.entries[1].label, "AGENTS");
        assert_eq!(stack.entries[1].path, root.join("AGENTS.md"));

        let _ = fs::remove_dir_all(root);
    }
}
