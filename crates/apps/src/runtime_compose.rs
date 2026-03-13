use crate::async_runtime_host::global_async_runtime_host;
use crate::cli_command::{LegacyIntentTemplate, RunTemplate};
use crate::config_loader::AppConfig;
use crate::env_util::read_env_trimmed;
use axonrunner_adapters::{
    FileMutationEvidence, FileWriteOutput, MemoryAdapter, MemoryTier, ProviderAdapter,
    ProviderRequest, ToolAdapter, ToolPolicy, ToolRequest, ToolResult, WorkspaceTool,
    build_contract_memory, build_contract_provider, provider_registry, resolve_provider_id,
    tiered_memory_key,
};
use axonrunner_core::DecisionOutcome;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod plan;

pub use self::plan::RuntimeRunPlan;

use self::plan::{
    MemoryPlan, ProviderPlan, RuntimeComposePlan, ToolPlan, build_runtime_compose_plan,
    build_runtime_run_plan,
};

const ENV_RUNTIME_MEMORY_PATH: &str = "AXONRUNNER_RUNTIME_MEMORY_PATH";
const ENV_RUNTIME_TOOL_WORKSPACE: &str = "AXONRUNNER_RUNTIME_TOOL_WORKSPACE";
const ENV_RUNTIME_TOOL_LOG_PATH: &str = "AXONRUNNER_RUNTIME_TOOL_LOG_PATH";
const ENV_RUNTIME_PROVIDER: &str = "AXONRUNNER_RUNTIME_PROVIDER";
const ENV_RUNTIME_PROVIDER_MODEL: &str = "AXONRUNNER_RUNTIME_PROVIDER_MODEL";
const ENV_RUNTIME_MAX_TOKENS: &str = "AXONRUNNER_RUNTIME_MAX_TOKENS";
const ENV_RUNTIME_COMMAND_ALLOWLIST: &str = "AXONRUNNER_RUNTIME_COMMAND_ALLOWLIST";

const DEFAULT_TOOL_LOG_PATH: &str = "runtime.log";
const DEFAULT_MAX_TOKENS: usize = 4096;
const TOOL_WRITE_LIMIT_BYTES: usize = 16 * 1024;
const TOOL_READ_LIMIT_BYTES: usize = 64 * 1024;
const TOOL_MAX_SEARCH_RESULTS: usize = 64;
const TOOL_MAX_COMMAND_OUTPUT_BYTES: usize = 32 * 1024;
const TOOL_COMMAND_TIMEOUT_MS: u64 = 5_000;
const HOT_CONTEXT_MAX_CHARS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComposeConfig {
    pub memory_path: Option<PathBuf>,
    pub tool_workspace: Option<PathBuf>,
    pub tool_log_path: String,
    pub provider_id: String,
    pub provider_model: String,
    pub max_tokens: usize,
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
    ApprovalRequired,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunRecord {
    pub plan: RuntimeRunPlan,
    pub verification: RuntimeRunVerification,
    pub repair: RuntimeRunRepair,
    pub phase: RuntimeRunPhase,
    pub outcome: RuntimeRunOutcome,
    pub reason: String,
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

        let mut next = self.clone();
        next.tool = RuntimeComposeStep::Applied;
        next.tool_outputs.extend(repair.tool_outputs.clone());
        next.patch_artifacts.extend(repair.patch_artifacts.clone());
        next
    }
}

impl RuntimeRunRepair {
    pub fn skipped(summary: impl Into<String>) -> Self {
        Self {
            attempted: false,
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
                .map(|home| PathBuf::from(home).join(".axonrunner").join("memory.db"))
        });
        let tool_workspace =
            env_path(ENV_RUNTIME_TOOL_WORKSPACE).or_else(|| config.workspace.clone());

        let tool_log_path = env_string(ENV_RUNTIME_TOOL_LOG_PATH).unwrap_or_else(|| {
            tool_workspace
                .as_ref()
                .map(|workspace| workspace.join(DEFAULT_TOOL_LOG_PATH).display().to_string())
                .unwrap_or_else(|| DEFAULT_TOOL_LOG_PATH.to_owned())
        });
        let max_tokens = env_string(ENV_RUNTIME_MAX_TOKENS)
            .and_then(|raw| raw.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_TOKENS);
        let command_allowlist = env_string(ENV_RUNTIME_COMMAND_ALLOWLIST)
            .and_then(|raw| parse_command_allowlist(&raw))
            .or_else(|| config.command_allowlist.clone());
        Self {
            memory_path,
            tool_workspace,
            tool_log_path,
            provider_id,
            provider_model,
            max_tokens,
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
    memory: Option<Box<dyn MemoryAdapter>>,
    memory_init: RuntimeComposeInitState,
    provider: Arc<dyn ProviderAdapter>,
    tool: Option<Arc<dyn ToolAdapter>>,
    tool_init: RuntimeComposeInitState,
    agents_path: Option<PathBuf>,
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
                "unknown runtime provider '{}'. set AXONRUNNER_RUNTIME_PROVIDER to one of: {supported}",
                config.provider_id.trim()
            )
        })?;
        config.provider_id = provider_id.to_owned();

        if config.tool_workspace.is_none() {
            return Err(String::from(
                "runtime tool workspace is not configured. set --workspace or AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
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
        if let Some(workspace) = config.tool_workspace.as_mut()
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

        let (tool, tool_init) =
            try_init_component(config.tool_workspace.as_ref().map(|workspace| {
                (
                    format!("workspace={}", workspace.display()),
                    build_tool_adapter(workspace, config.command_allowlist.clone()),
                )
            }));

        let provider = Arc::from(
            build_contract_provider(provider_id)
                .map_err(|error| format!("provider init failed for '{provider_id}': {error}"))?,
        );
        let agents_path = find_agents_md(config.tool_workspace.as_deref());

        Ok(Self {
            config,
            memory,
            memory_init,
            provider,
            tool: tool.map(|tool| Arc::new(tool) as Arc<dyn ToolAdapter>),
            tool_init,
            agents_path,
        })
    }

    pub fn apply_template(
        &mut self,
        template: &RunTemplate,
        intent_id: &str,
        outcome: DecisionOutcome,
    ) -> RuntimeComposeExecution {
        self.apply_plan(build_runtime_compose_plan(
            template,
            intent_id,
            outcome,
            &self.config.provider_model,
            self.config.max_tokens,
            &self.config.tool_log_path,
        ))
    }

    pub fn plan_template(
        &self,
        template: &RunTemplate,
        run_id: &str,
        intent_id: &str,
        outcome: DecisionOutcome,
    ) -> RuntimeRunPlan {
        build_runtime_run_plan(template, run_id, intent_id, outcome)
    }

    pub fn repair_template(
        &self,
        template: &RunTemplate,
        intent_id: &str,
        outcome: DecisionOutcome,
        prior: &RuntimeComposeExecution,
    ) -> RuntimeRunRepair {
        if !matches!(prior.tool, RuntimeComposeStep::Failed(_)) {
            return RuntimeRunRepair::skipped("tool_step_not_failed");
        }

        let plan = build_runtime_compose_plan(
            template,
            intent_id,
            outcome,
            &self.config.provider_model,
            self.config.max_tokens,
            &self.config.tool_log_path,
        );
        if plan.tool.is_none() {
            return RuntimeRunRepair::skipped("no_tool_plan");
        }

        let (tool, tool_outputs, patch_artifacts) =
            self.execute_tool(plan.tool, prior.provider_output.as_deref());
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
            status,
            summary,
            tool,
            tool_outputs,
            patch_artifacts,
        }
    }

    pub fn clear(&mut self) -> Result<usize, String> {
        let Some(memory) = self.memory.as_mut() else {
            return Ok(0);
        };

        let entries = memory
            .list()
            .map_err(|error| format!("list memory records failed: {error}"))?;
        let mut removed = 0usize;

        for entry in entries {
            let deleted = memory
                .delete(&entry.key)
                .map_err(|error| format!("clear key '{}' failed: {error}", entry.key))?;
            if deleted {
                removed = removed.saturating_add(1);
            }
        }

        Ok(removed)
    }

    pub fn remember_run_summary(
        &mut self,
        run: &RuntimeRunRecord,
        intent_id: &str,
    ) -> Result<(), String> {
        let Some(memory) = self.memory.as_mut() else {
            return Ok(());
        };

        let key = tiered_memory_key(MemoryTier::Recall, &format!("last_run/{intent_id}"));
        let value = Self::compact_hot_context(&[
            format!("run_id={}", run.plan.run_id),
            format!("phase={}", run_phase_name(run.phase)),
            format!("outcome={}", run_outcome_name(run.outcome)),
            format!("reason={}", run.reason),
            format!("goal={}", run.plan.goal),
            format!("summary={}", run.plan.summary),
            format!(
                "agents={}",
                self.agents_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| String::from("none"))
            ),
        ]);
        memory
            .store(&key, &value)
            .map_err(|error| format!("store recall summary failed: {error}"))
    }

    fn compact_hot_context(entries: &[String]) -> String {
        let joined = entries.join(" | ");
        if joined.chars().count() <= HOT_CONTEXT_MAX_CHARS {
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

    pub fn write_report(
        &self,
        template: &RunTemplate,
        intent_id: &str,
        outcome: DecisionOutcome,
        policy_code: &str,
        effect_count: usize,
        execution: &RuntimeComposeExecution,
        run: &RuntimeRunRecord,
    ) -> Result<Vec<RuntimeComposePatchArtifact>, String> {
        let Some(tool) = self.tool.as_ref() else {
            return Ok(Vec::new());
        };

        let base = format!(".axonrunner/artifacts/{intent_id}");
        let files = [
            (
                format!("{base}.plan.md"),
                format!(
                    "# Plan\n\nintent_id={intent_id}\nkind={}\noutcome={}\npolicy={policy_code}\ngoal={}\nsummary={}\ndone_when={}\nplanned_steps={}\nsteps={}\n",
                    template_kind(template),
                    outcome_name(outcome),
                    run.plan.goal,
                    run.plan.summary,
                    run.plan.done_when,
                    run.plan.planned_steps,
                    run.plan
                        .steps
                        .iter()
                        .map(|step| format!("{}:{}:{}", step.phase, step.label, step.done_when))
                        .collect::<Vec<_>>()
                        .join(" | "),
                ),
            ),
            (
                format!("{base}.apply.md"),
                format!(
                    "# Apply\n\nphase={}\nprovider={}\nmemory={}\ntool={}\neffects={effect_count}\nprovider_cwd={}\nprovider_output={}\n",
                    run_phase_name(RuntimeRunPhase::ExecutingStep),
                    step_name(&execution.provider),
                    step_name(&execution.memory),
                    step_name(&execution.tool),
                    execution.provider_cwd,
                    execution.provider_output.as_deref().unwrap_or("<none>"),
                ),
            ),
            (
                format!("{base}.verify.md"),
                format!(
                    "# Verify\n\nphase={}\nstatus={}\nsummary={}\nchecks={}\nrepair_attempted={}\nrepair_status={}\nrepair_summary={}\nfirst_failure={}\n",
                    run_phase_name(RuntimeRunPhase::Verifying),
                    run.verification.status,
                    run.verification.summary,
                    if run.verification.checks.is_empty() {
                        String::from("none")
                    } else {
                        run.verification.checks.join(" | ")
                    },
                    run.repair.attempted,
                    run.repair.status,
                    run.repair.summary,
                    execution
                        .first_failure()
                        .map(|(stage, message)| format!("{stage}:{message}"))
                        .unwrap_or_else(|| String::from("none"))
                ),
            ),
            (
                format!("{base}.report.md"),
                format!(
                    "# Report\n\nintent_id={intent_id}\nkind={}\noutcome={}\npolicy={policy_code}\nrun_phase={}\nrun_outcome={}\nrun_reason={}\nprovider={}\nprovider_cwd={}\nmemory={}\ntool={}\noutputs={}\nchanged_paths={}\nevidence={}\n",
                    template_kind(template),
                    outcome_name(outcome),
                    run_phase_name(run.phase),
                    run_outcome_name(run.outcome),
                    run.reason,
                    step_name(&execution.provider),
                    execution.provider_cwd,
                    step_name(&execution.memory),
                    step_name(&execution.tool),
                    if execution.tool_outputs.is_empty() {
                        String::from("none")
                    } else {
                        execution.tool_outputs.join(" | ")
                    },
                    if execution.patch_artifacts.is_empty() {
                        String::from("none")
                    } else {
                        execution
                            .patch_artifacts
                            .iter()
                            .map(|artifact| artifact.target_path.as_str())
                            .collect::<Vec<_>>()
                            .join(" | ")
                    },
                    if execution.patch_artifacts.is_empty() {
                        String::from("none")
                    } else {
                        execution
                            .patch_artifacts
                            .iter()
                            .map(|artifact| {
                                format!(
                                    "{}:{}:{}",
                                    artifact.operation,
                                    artifact.target_path,
                                    artifact.after_excerpt.as_deref().unwrap_or("no_excerpt")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(" | ")
                    }
                ),
            ),
        ];

        let mut patch_artifacts = Vec::new();
        for (path, contents) in files {
            let result = tool
                .execute(ToolRequest::FileWrite {
                    path,
                    contents,
                    append: false,
                })
                .map_err(|error| format!("runtime_compose.report: {error}"))?;
            let ToolResult::FileWrite(FileWriteOutput { path, evidence, .. }) = result else {
                return Err(String::from(
                    "runtime_compose.report: unexpected non-file-write result",
                ));
            };
            patch_artifacts.push(patch_artifact_from_write_output(path, evidence));
        }

        Ok(patch_artifacts)
    }

    fn apply_plan(&mut self, plan: RuntimeComposePlan) -> RuntimeComposeExecution {
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
            self.execute_tool(plan.tool, provider_output.as_deref());
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
        let request = ProviderRequest::new(
            plan.model,
            plan.prompt,
            plan.max_tokens,
            self.provider_cwd(),
        );
        match complete_provider_request(provider, request) {
            Ok(content) => (Some(content), RuntimeComposeStep::Applied),
            Err(error) => (None, RuntimeComposeStep::Failed(error)),
        }
    }

    fn execute_memory(&mut self, plan: MemoryPlan) -> RuntimeComposeStep {
        let Some(memory) = self.memory.as_mut() else {
            return RuntimeComposeStep::Skipped;
        };

        match plan {
            MemoryPlan::None => RuntimeComposeStep::Skipped,
            MemoryPlan::Put { key, value } => match memory.store(&key, &value) {
                Ok(()) => RuntimeComposeStep::Applied,
                Err(error) => RuntimeComposeStep::Failed(error.to_string()),
            },
            MemoryPlan::Remove { key } => match memory.delete(&key) {
                Ok(_) => RuntimeComposeStep::Applied,
                Err(error) => RuntimeComposeStep::Failed(error.to_string()),
            },
        }
    }

    fn execute_tool(
        &self,
        plan: Option<ToolPlan>,
        provider_output: Option<&str>,
    ) -> (
        RuntimeComposeStep,
        Vec<String>,
        Vec<RuntimeComposePatchArtifact>,
    ) {
        let Some(plan) = plan else {
            return (RuntimeComposeStep::Skipped, Vec::new(), Vec::new());
        };
        let Some(tool) = self.tool.as_ref() else {
            return (RuntimeComposeStep::Skipped, Vec::new(), Vec::new());
        };

        let line = format!(
            "{} provider={}\n",
            plan.line_prefix,
            provider_output.unwrap_or("<none>")
        );
        match tool.execute(ToolRequest::FileWrite {
            path: plan.path,
            contents: line,
            append: true,
        }) {
            Ok(ToolResult::FileWrite(FileWriteOutput { path, evidence, .. })) => (
                RuntimeComposeStep::Applied,
                vec![format!("log={}", path.display())],
                vec![patch_artifact_from_write_output(path, evidence)],
            ),
            Ok(_) => (RuntimeComposeStep::Applied, Vec::new(), Vec::new()),
            Err(error) => (
                RuntimeComposeStep::Failed(format!("runtime_compose.tool.file_write: {error}")),
                Vec::new(),
                Vec::new(),
            ),
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
}

fn build_tool_adapter(
    workspace: &Path,
    command_allowlist: Option<Vec<String>>,
) -> Result<WorkspaceTool, String> {
    fs::create_dir_all(workspace)
        .map_err(|error| format!("create workspace '{}' failed: {error}", workspace.display()))?;

    WorkspaceTool::new(
        workspace,
        ToolPolicy {
            max_file_write_bytes: TOOL_WRITE_LIMIT_BYTES,
            max_file_read_bytes: TOOL_READ_LIMIT_BYTES,
            max_search_results: TOOL_MAX_SEARCH_RESULTS,
            max_command_output_bytes: TOOL_MAX_COMMAND_OUTPUT_BYTES,
            command_timeout_ms: TOOL_COMMAND_TIMEOUT_MS,
            command_allowlist: command_allowlist.unwrap_or_else(default_command_allowlist),
        },
    )
    .map_err(|error| format!("tool adapter init failed: {error}"))
}

fn find_agents_md(start: Option<&Path>) -> Option<PathBuf> {
    let mut current = start?.to_path_buf();
    loop {
        let candidate = current.join("AGENTS.md");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn default_command_allowlist() -> Vec<String> {
    vec![
        String::from("pwd"),
        String::from("git"),
        String::from("cargo"),
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

fn patch_artifact_from_write_output(
    path: std::path::PathBuf,
    evidence: FileMutationEvidence,
) -> RuntimeComposePatchArtifact {
    RuntimeComposePatchArtifact {
        operation: evidence.operation,
        target_path: path.display().to_string(),
        artifact_path: evidence.artifact_path.display().to_string(),
        before_digest: evidence.before_digest,
        after_digest: evidence.after_digest,
        before_excerpt: evidence.before_excerpt,
        after_excerpt: evidence.after_excerpt,
        unified_diff: evidence.unified_diff,
    }
}

fn complete_provider_request(
    provider: Arc<dyn ProviderAdapter>,
    request: ProviderRequest,
) -> Result<String, String> {
    global_async_runtime_host().block_on_async("runtime_compose.provider.complete", async move {
        provider
            .complete(request)
            .await
            .map(|response| response.content)
            .map_err(|error| format!("runtime_compose.provider.complete: {error}"))
    })
}

fn shutdown_provider(provider: Arc<dyn ProviderAdapter>) -> Result<(), String> {
    global_async_runtime_host().block_on_async("runtime_compose.provider.shutdown", async move {
        provider
            .shutdown()
            .await
            .map_err(|error| format!("runtime_compose.provider.shutdown: {error}"))
    })
}

fn probe_provider_health(provider: Arc<dyn ProviderAdapter>) -> RuntimeComposeComponentHealth {
    match global_async_runtime_host().block_on_async(
        "runtime_compose.provider.health",
        async move {
            provider
                .health()
                .await
                .map_err(|error| format!("runtime_compose.provider.health: {error}"))
        },
    ) {
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

fn template_kind(template: &RunTemplate) -> &'static str {
    match template.legacy_intent() {
        LegacyIntentTemplate::Read { .. } => "read",
        LegacyIntentTemplate::Write { .. } => "write",
        LegacyIntentTemplate::Remove { .. } => "remove",
        LegacyIntentTemplate::Freeze => "freeze",
        LegacyIntentTemplate::Halt => "halt",
    }
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
        RuntimeRunOutcome::ApprovalRequired => "approval_required",
        RuntimeRunOutcome::Failed => "failed",
        RuntimeRunOutcome::Aborted => "aborted",
    }
}

pub fn verifier_profile_name(program: &str, args: &[String]) -> &'static str {
    match (program, args.first().map(String::as_str), args.get(1).map(String::as_str)) {
        ("cargo", Some("build"), _) => "build",
        ("cargo", Some("test"), _) => "test",
        ("cargo", Some("clippy"), _) => "lint",
        ("cargo", Some("fmt"), Some("--check")) => "lint",
        ("cargo", Some("fmt"), _) => "lint",
        ("npm", Some("test"), _) | ("pnpm", Some("test"), _) | ("yarn", Some("test"), _) => {
            "test"
        }
        ("npm", Some("run"), Some("lint"))
        | ("pnpm", Some("run"), Some("lint"))
        | ("yarn", Some("lint"), _) => "lint",
        ("npm", Some("run"), Some("build"))
        | ("pnpm", Some("run"), Some("build"))
        | ("yarn", Some("build"), _) => "build",
        _ => "generic",
    }
}

fn step_name(step: &RuntimeComposeStep) -> &'static str {
    match step {
        RuntimeComposeStep::Skipped => "skipped",
        RuntimeComposeStep::Applied => "applied",
        RuntimeComposeStep::Failed(_) => "failed",
    }
}

fn outcome_name(outcome: DecisionOutcome) -> &'static str {
    match outcome {
        DecisionOutcome::Accepted => "accepted",
        DecisionOutcome::Rejected => "rejected",
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    env_string(key).map(PathBuf::from)
}

fn env_string(key: &str) -> Option<String> {
    read_env_trimmed(key).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeComposeConfig, RuntimeComposeExecution, RuntimeComposeState, RuntimeComposeStep,
        build_tool_adapter, run_outcome_name, run_phase_name,
        verifier_profile_name,
    };
    use crate::cli_command::{LegacyIntentTemplate, RunTemplate};
    use crate::config_loader::AppConfig;
    use axonrunner_core::DecisionOutcome;
    use axonrunner_adapters::{ToolAdapter, ToolRequest};
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_dir(label: &str) -> std::path::PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axonrunner-runtime-compose-test-{label}-{}-{tick}",
            std::process::id()
        ))
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

        let tool = build_tool_adapter(&workspace, Some(vec![String::from("pwd")]))
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
            tool_log_path: workspace.join("runtime.log").display().to_string(),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 256,
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
            &RunTemplate::LegacyIntent(LegacyIntentTemplate::Write {
                key: String::from("alpha"),
                value: String::from("42"),
            }),
            "cli-repair",
            DecisionOutcome::Accepted,
            &prior,
        );

        assert!(repair.attempted);
        assert_eq!(repair.status, "repaired");
        assert!(matches!(repair.tool, RuntimeComposeStep::Applied));
        assert!(!repair.tool_outputs.is_empty());
        assert!(!repair.patch_artifacts.is_empty());

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
            tool_log_path: workspace.join("runtime.log").display().to_string(),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 256,
            command_allowlist: None,
        })
        .expect("runtime compose state should init");

        let execution = state.apply_template(
            &RunTemplate::LegacyIntent(LegacyIntentTemplate::Write {
                key: String::from("alpha"),
                value: String::from("42"),
            }),
            "cli-cwd",
            DecisionOutcome::Accepted,
        );

        assert_eq!(
            execution.provider_cwd,
            canonical_workspace.display().to_string()
        );

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn run_phase_and_outcome_names_cover_terminal_values() {
        assert_eq!(run_phase_name(super::RuntimeRunPhase::Completed), "completed");
        assert_eq!(
            run_phase_name(super::RuntimeRunPhase::WaitingApproval),
            "waiting_approval"
        );
        assert_eq!(run_outcome_name(super::RuntimeRunOutcome::Success), "success");
        assert_eq!(
            run_outcome_name(super::RuntimeRunOutcome::ApprovalRequired),
            "approval_required"
        );
        assert_eq!(run_outcome_name(super::RuntimeRunOutcome::Aborted), "aborted");
    }

    #[test]
    fn verifier_profile_name_matches_standard_command_profiles() {
        assert_eq!(verifier_profile_name("cargo", &[String::from("build")]), "build");
        assert_eq!(verifier_profile_name("cargo", &[String::from("test")]), "test");
        assert_eq!(verifier_profile_name("cargo", &[String::from("clippy")]), "lint");
        assert_eq!(
            verifier_profile_name("npm", &[String::from("run"), String::from("build")]),
            "build"
        );
        assert_eq!(verifier_profile_name("pwd", &[]), "generic");
    }

    #[test]
    fn compact_hot_context_keeps_long_summaries_bounded() {
        let entries = (0..64)
            .map(|index| format!("entry-{index}-{}", "x".repeat(32)))
            .collect::<Vec<_>>();
        let compact = RuntimeComposeState::compact_hot_context(&entries);

        assert!(compact.len() <= super::HOT_CONTEXT_MAX_CHARS + 16);
        assert!(compact.ends_with("...<compacted>"));
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
}
