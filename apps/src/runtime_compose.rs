use crate::cli_command::IntentTemplate;
use crate::identity_bootstrap::{BootstrapContext, BootstrapLoadConfig, load_bootstrap_context};
use crate::parse_util::parse_tools_list;
use axiom_adapters::{ChannelAdapter, MemoryAdapter, ProviderAdapter, ProviderRequest, build_contract_channel, build_contract_memory, build_contract_provider};
use axiom_adapters::{DEFAULT_PROVIDER_ID, resolve_provider_id};
use axiom_adapters::contracts::ContextAdapter;
use axiom_adapters::tool::{WorkspaceTool, ToolPolicy, ToolRequest};
use axiom_core::DecisionOutcome;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const ENV_RUNTIME_MEMORY_PATH: &str = "AXIOM_RUNTIME_MEMORY_PATH";
const ENV_RUNTIME_TOOL_WORKSPACE: &str = "AXIOM_RUNTIME_TOOL_WORKSPACE";
const ENV_RUNTIME_TOOL_LOG_PATH: &str = "AXIOM_RUNTIME_TOOL_LOG_PATH";
const ENV_RUNTIME_PROVIDER: &str = "AXIOM_RUNTIME_PROVIDER";
const ENV_RUNTIME_PROVIDER_MODEL: &str = "AXIOM_RUNTIME_PROVIDER_MODEL";
const ENV_RUNTIME_BOOTSTRAP_ROOT: &str = "AXIOM_RUNTIME_BOOTSTRAP_ROOT";
const ENV_RUNTIME_MAX_TOKENS: &str = "AXIOM_RUNTIME_MAX_TOKENS";
const ENV_RUNTIME_CHANNEL: &str = "AXIOM_RUNTIME_CHANNEL";
const ENV_RUNTIME_TOOLS: &str = "AXIOM_RUNTIME_TOOLS";
const ENV_RUNTIME_CONTEXT_ROOT: &str = "AXIOM_CONTEXT_ROOT";

const DEFAULT_TOOL_LOG_PATH: &str = ".axiom/runtime-compose.log";
const DEFAULT_MAX_TOKENS: usize = 4096;
const TOOL_WRITE_LIMIT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComposeConfig {
    pub memory_path: Option<PathBuf>,
    pub tool_workspace: Option<PathBuf>,
    pub tool_log_path: String,
    pub provider_id: String,
    pub provider_model: String,
    pub max_tokens: usize,
    pub bootstrap_root: Option<PathBuf>,
    pub channel_id: Option<String>,
    pub tool_ids: Vec<String>,
    /// Root directory for AxiomMe context store. None = RAG disabled.
    pub context_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComposeHealth {
    pub provider_id: String,
    pub provider_model: String,
    pub memory_enabled: bool,
    pub memory_state: &'static str,
    pub memory_detail: String,
    pub tool_enabled: bool,
    pub tool_state: &'static str,
    pub tool_detail: String,
    pub bootstrap_sections: usize,
    pub bootstrap_bytes: usize,
    pub bootstrap_state: &'static str,
    pub bootstrap_detail: String,
    pub channel_enabled: bool,
    pub channel_state: &'static str,
    pub channel_detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeComposeStep {
    Skipped,
    Applied,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComposeExecution {
    pub provider: RuntimeComposeStep,
    pub memory: RuntimeComposeStep,
    pub tool: RuntimeComposeStep,
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
}

impl RuntimeComposeConfig {
    pub fn from_env(default_provider_id: &str) -> Self {
        let tool_log_path = env_string(ENV_RUNTIME_TOOL_LOG_PATH)
            .unwrap_or_else(|| DEFAULT_TOOL_LOG_PATH.to_owned());
        let provider_id =
            env_string(ENV_RUNTIME_PROVIDER).unwrap_or_else(|| default_provider_id.to_owned());
        let provider_model =
            env_string(ENV_RUNTIME_PROVIDER_MODEL).unwrap_or_else(|| provider_id.clone());

        let max_tokens = env_string(ENV_RUNTIME_MAX_TOKENS)
            .and_then(|raw| raw.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_MAX_TOKENS);

        let tool_ids = env_string(ENV_RUNTIME_TOOLS)
            .map(|raw| parse_tools_list(&raw))
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| vec![axiom_adapters::DEFAULT_TOOL_ID.to_owned()]);

        let memory_path = env_path(ENV_RUNTIME_MEMORY_PATH).or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".axiom").join("memory.db"))
        });
        let tool_workspace = env_path(ENV_RUNTIME_TOOL_WORKSPACE).or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".axiom").join("workspace"))
        });

        Self {
            memory_path,
            tool_workspace,
            tool_log_path,
            provider_id,
            provider_model,
            max_tokens,
            bootstrap_root: env_path(ENV_RUNTIME_BOOTSTRAP_ROOT),
            channel_id: env_string(ENV_RUNTIME_CHANNEL),
            tool_ids,
            context_root: std::env::var(ENV_RUNTIME_CONTEXT_ROOT)
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MemoryPlan {
    None,
    Put { key: String, value: String },
    Remove { key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderPlan {
    model: String,
    prompt: String,
    max_tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolPlan {
    path: String,
    line_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeComposePlan {
    provider: Option<ProviderPlan>,
    memory: MemoryPlan,
    tool: Option<ToolPlan>,
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
            RuntimeComposeInitState::Disabled => "disabled",
            RuntimeComposeInitState::Ready(_) => "ready",
            RuntimeComposeInitState::Failed(_) => "failed",
        }
    }

    fn detail(&self) -> String {
        match self {
            RuntimeComposeInitState::Disabled => String::from("not_configured"),
            RuntimeComposeInitState::Ready(detail) => detail.clone(),
            RuntimeComposeInitState::Failed(detail) => detail.clone(),
        }
    }
}

pub struct RuntimeComposeState {
    config: RuntimeComposeConfig,
    memory: Option<Box<dyn MemoryAdapter>>,
    memory_init: RuntimeComposeInitState,
    provider: Box<dyn ProviderAdapter>,
    tool: Option<WorkspaceTool>,
    tool_init: RuntimeComposeInitState,
    bootstrap_context: Option<BootstrapContext>,
    bootstrap_init: RuntimeComposeInitState,
    channel: Option<Box<dyn ChannelAdapter>>,
    channel_init: RuntimeComposeInitState,
    context: Option<Box<dyn ContextAdapter>>,
}

fn try_init_component<T>(
    option: Option<(String, Result<T, String>)>,
) -> (Option<T>, RuntimeComposeInitState) {
    match option {
        Some((detail, Ok(t))) => (Some(t), RuntimeComposeInitState::Ready(detail)),
        Some((detail, Err(e))) => (None, RuntimeComposeInitState::Failed(format!("{detail} error={e}"))),
        None => (None, RuntimeComposeInitState::Disabled),
    }
}

impl RuntimeComposeState {
    pub fn new(mut config: RuntimeComposeConfig) -> Self {
        let selected_provider = match resolve_provider_id(&config.provider_id) {
            Some(id) => {
                config.provider_id = id.to_owned();
                id
            }
            None => {
                config.provider_id = DEFAULT_PROVIDER_ID.to_owned();
                DEFAULT_PROVIDER_ID
            }
        };

        // ~/.axiom/ 디렉토리가 없으면 미리 생성 (sqlite/workspace 둘 다 필요)
        if let Some(path) = &config.memory_path
            && let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
        if let Some(workspace) = &config.tool_workspace {
            let _ = std::fs::create_dir_all(workspace);
        }

        let (memory, memory_init) = try_init_component(
            config.memory_path.as_ref().map(|path| {
                let backend = if path.extension().and_then(|e| e.to_str()) == Some("db") {
                    "sqlite"
                } else {
                    "markdown"
                };
                (format!("path={}", path.display()), build_contract_memory(backend, path))
            }),
        );

        let (tool, tool_init) = try_init_component(
            config.tool_workspace.as_ref().map(|workspace| {
                (format!("workspace={}", workspace.display()), build_tool_adapter(workspace))
            }),
        );

        let provider = build_contract_provider(selected_provider)
            .or_else(|_| build_contract_provider(DEFAULT_PROVIDER_ID))
            .expect("default provider must be in registry");
        let (bootstrap_context, bootstrap_init) = match config.bootstrap_root.as_deref() {
            Some(root) => match load_bootstrap_context(root, &BootstrapLoadConfig::default()) {
                Some(context) => (
                    Some(context),
                    RuntimeComposeInitState::Ready(format!("root={}", root.display())),
                ),
                None => (
                    None,
                    RuntimeComposeInitState::Failed(format!(
                        "root={} no bootstrap content",
                        root.display()
                    )),
                ),
            },
            None => (None, RuntimeComposeInitState::Disabled),
        };

        let (channel, channel_init) = try_init_component(
            config.channel_id.as_deref().map(|channel_name| {
                (format!("channel={channel_name}"), build_contract_channel(channel_name))
            }),
        );

        let context = config.context_root.as_ref().and_then(|root| {
            match axiom_adapters::AxiommeContextAdapter::new(root) {
                Ok(adapter) => Some(Box::new(adapter) as Box<dyn ContextAdapter>),
                Err(e) => {
                    eprintln!("context adapter init failed (RAG disabled): {e}");
                    None
                }
            }
        });

        Self {
            config,
            memory,
            memory_init,
            provider,
            tool,
            tool_init,
            bootstrap_context,
            bootstrap_init,
            channel,
            channel_init,
            context,
        }
    }

    pub fn apply_template(
        &mut self,
        template: &IntentTemplate,
        intent_id: &str,
        outcome: DecisionOutcome,
    ) -> RuntimeComposeExecution {
        let plan = build_runtime_compose_plan(
            template,
            intent_id,
            outcome,
            &self.config.provider_model,
            self.config.max_tokens,
            &self.config.tool_log_path,
            self.bootstrap_context
                .as_ref()
                .map(|context| context.rendered.as_str()),
        );
        self.apply_plan(plan)
    }

    pub fn context(&self) -> Option<&dyn ContextAdapter> {
        self.context.as_ref().map(|b| b.as_ref())
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

    pub fn health(&self) -> RuntimeComposeHealth {
        RuntimeComposeHealth {
            provider_id: self.config.provider_id.clone(),
            provider_model: self.config.provider_model.clone(),
            memory_enabled: self.memory.is_some(),
            memory_state: self.memory_init.state_name(),
            memory_detail: self.memory_init.detail(),
            tool_enabled: self.tool.is_some(),
            tool_state: self.tool_init.state_name(),
            tool_detail: self.tool_init.detail(),
            bootstrap_sections: self
                .bootstrap_context
                .as_ref()
                .map(|context| context.sections.len())
                .unwrap_or(0),
            bootstrap_bytes: self
                .bootstrap_context
                .as_ref()
                .map(|context| context.total_content_bytes)
                .unwrap_or(0),
            bootstrap_state: self.bootstrap_init.state_name(),
            bootstrap_detail: self.bootstrap_init.detail(),
            channel_enabled: self.channel.is_some(),
            channel_state: self.channel_init.state_name(),
            channel_detail: self.channel_init.detail(),
        }
    }

    fn apply_plan(&mut self, plan: RuntimeComposePlan) -> RuntimeComposeExecution {
        let (provider_output, provider) = self.execute_provider(plan.provider);
        let memory = self.execute_memory(plan.memory);
        let tool = self.execute_tool(plan.tool, provider_output.as_deref());
        RuntimeComposeExecution {
            provider,
            memory,
            tool,
        }
    }

    fn execute_provider(&self, plan: Option<ProviderPlan>) -> (Option<String>, RuntimeComposeStep) {
        let Some(plan) = plan else {
            return (None, RuntimeComposeStep::Skipped);
        };

        match self
            .provider
            .complete(ProviderRequest::new(plan.model, plan.prompt, plan.max_tokens))
        {
            Ok(response) => (Some(response.content), RuntimeComposeStep::Applied),
            Err(error) => (None, RuntimeComposeStep::Failed(error.to_string())),
        }
    }

    fn execute_memory(&mut self, plan: MemoryPlan) -> RuntimeComposeStep {
        let Some(memory) = self.memory.as_mut() else {
            return RuntimeComposeStep::Skipped;
        };

        match plan {
            MemoryPlan::None => RuntimeComposeStep::Skipped,
            MemoryPlan::Put { key, value } => {
                if let Err(error) = memory.store(&key, &value) {
                    return RuntimeComposeStep::Failed(error.to_string());
                }
                RuntimeComposeStep::Applied
            }
            MemoryPlan::Remove { key } => {
                if let Err(error) = memory.delete(&key) {
                    return RuntimeComposeStep::Failed(error.to_string());
                }
                RuntimeComposeStep::Applied
            }
        }
    }

    fn execute_tool(
        &self,
        plan: Option<ToolPlan>,
        provider_output: Option<&str>,
    ) -> RuntimeComposeStep {
        let Some(tool) = self.tool.as_ref() else {
            return RuntimeComposeStep::Skipped;
        };
        let Some(plan) = plan else {
            return RuntimeComposeStep::Skipped;
        };

        let line = build_tool_line(&plan.line_prefix, provider_output);
        if let Err(error) = tool.execute(ToolRequest::FileWrite {
            path: &plan.path,
            contents: &line,
            append: true,
        }) {
            return RuntimeComposeStep::Failed(error.to_string());
        }
        RuntimeComposeStep::Applied
    }
}

fn build_runtime_compose_plan(
    template: &IntentTemplate,
    intent_id: &str,
    outcome: DecisionOutcome,
    provider_model: &str,
    max_tokens: usize,
    tool_log_path: &str,
    bootstrap_prompt: Option<&str>,
) -> RuntimeComposePlan {
    if outcome == DecisionOutcome::Rejected {
        return RuntimeComposePlan {
            provider: None,
            memory: MemoryPlan::None,
            tool: None,
        };
    }

    match template {
        IntentTemplate::Write { key, value } => RuntimeComposePlan {
            provider: Some(ProviderPlan {
                model: provider_model.to_owned(),
                prompt: compose_provider_prompt(
                    format!("intent={intent_id} kind=write key={key} value={value}"),
                    bootstrap_prompt,
                ),
                max_tokens,
            }),
            memory: MemoryPlan::Put {
                key: key.clone(),
                value: value.clone(),
            },
            tool: Some(ToolPlan {
                path: tool_log_path.to_owned(),
                line_prefix: format!("intent={intent_id} kind=write key={key}"),
            }),
        },
        IntentTemplate::Remove { key } => RuntimeComposePlan {
            provider: Some(ProviderPlan {
                model: provider_model.to_owned(),
                prompt: compose_provider_prompt(
                    format!("intent={intent_id} kind=remove key={key}"),
                    bootstrap_prompt,
                ),
                max_tokens,
            }),
            memory: MemoryPlan::Remove { key: key.clone() },
            tool: Some(ToolPlan {
                path: tool_log_path.to_owned(),
                line_prefix: format!("intent={intent_id} kind=remove key={key}"),
            }),
        },
        IntentTemplate::Read { .. } | IntentTemplate::Freeze | IntentTemplate::Halt => {
            RuntimeComposePlan {
                provider: None,
                memory: MemoryPlan::None,
                tool: None,
            }
        }
    }
}

fn compose_provider_prompt(base_prompt: String, bootstrap_prompt: Option<&str>) -> String {
    let Some(bootstrap_prompt) = bootstrap_prompt else {
        return base_prompt;
    };

    if bootstrap_prompt.is_empty() {
        return base_prompt;
    }

    format!("{base_prompt}\n\nbootstrap_context:\n{bootstrap_prompt}")
}

fn build_tool_adapter(workspace: &Path) -> Result<WorkspaceTool, String> {
    fs::create_dir_all(workspace)
        .map_err(|error| format!("create workspace '{}' failed: {error}", workspace.display()))?;
    let policy = ToolPolicy {
        allow_shell: false,
        allow_file_read: false,
        allow_file_write: true,
        max_shell_command_bytes: 0,
        max_file_read_bytes: 0,
        max_file_write_bytes: TOOL_WRITE_LIMIT_BYTES,
    };

    WorkspaceTool::new(workspace, policy)
        .map_err(|error| format!("tool adapter init failed: {error}"))
}

fn build_tool_line(prefix: &str, provider_output: Option<&str>) -> String {
    let provider_output = provider_output.unwrap_or("<none>");
    format!("{prefix} provider={provider_output}\n")
}

fn env_path(key: &str) -> Option<PathBuf> {
    env_string(key).map(PathBuf::from)
}

fn env_string(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{
        MemoryPlan, RuntimeComposeConfig, RuntimeComposeState, RuntimeComposeStep,
        build_runtime_compose_plan,
    };
    use crate::cli_command::IntentTemplate;
    use axiom_adapters::{MemoryAdapter, memory::MarkdownMemoryAdapter};
    use axiom_core::DecisionOutcome;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_path(label: &str, extension: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiom-runtime-compose-{label}-{}-{tick}.{extension}",
            std::process::id()
        ))
    }

    #[test]
    fn compose_plan_is_explicit_for_write() {
        let plan = build_runtime_compose_plan(
            &IntentTemplate::Write {
                key: String::from("alpha"),
                value: String::from("42"),
            },
            "cli-1",
            DecisionOutcome::Accepted,
            "mock-local",
            4096,
            "runtime.log",
            None,
        );

        assert!(plan.provider.is_some());
        assert_eq!(
            plan.memory,
            MemoryPlan::Put {
                key: String::from("alpha"),
                value: String::from("42")
            }
        );
        assert!(plan.tool.is_some());
    }

    #[test]
    fn compose_plan_drops_all_effects_when_rejected() {
        let plan = build_runtime_compose_plan(
            &IntentTemplate::Write {
                key: String::from("alpha"),
                value: String::from("42"),
            },
            "cli-1",
            DecisionOutcome::Rejected,
            "mock-local",
            4096,
            "runtime.log",
            None,
        );

        assert!(plan.provider.is_none());
        assert_eq!(plan.memory, MemoryPlan::None);
        assert!(plan.tool.is_none());
    }

    #[test]
    fn runtime_compose_executes_provider_memory_and_tool() {
        let memory_path = unique_path("memory", "md");
        let workspace = unique_path("workspace", "dir");

        let mut state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path: Some(memory_path.clone()),
            tool_workspace: Some(workspace.clone()),
            tool_log_path: String::from("runtime.log"),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 4096,
            bootstrap_root: None,
            channel_id: None,
            tool_ids: vec![axiom_adapters::DEFAULT_TOOL_ID.to_owned()],
            context_root: None,
        });

        let execution = state.apply_template(
            &IntentTemplate::Write {
                key: String::from("alpha"),
                value: String::from("42"),
            },
            "cli-1",
            DecisionOutcome::Accepted,
        );
        assert_eq!(execution.provider, RuntimeComposeStep::Applied);
        assert_eq!(execution.memory, RuntimeComposeStep::Applied);
        assert_eq!(execution.tool, RuntimeComposeStep::Applied);

        let reader = MarkdownMemoryAdapter::new(memory_path.clone()).expect("memory should open");
        let record = reader
            .get("alpha")
            .expect("memory read should succeed")
            .expect("record should exist");
        assert_eq!(record.value, "42");

        let log_path = workspace.join("runtime.log");
        let log = fs::read_to_string(&log_path).expect("tool log should exist");
        assert!(log.contains("intent=cli-1 kind=write key=alpha"));
        assert!(log.contains("provider=intent=cli-1 kind=write key=alpha value=42"));

        let _ = fs::remove_file(memory_path);
        let _ = fs::remove_file(log_path);
        let _ = fs::remove_dir(workspace);
    }

    #[test]
    fn runtime_compose_uses_selected_registry_provider() {
        let workspace = unique_path("provider-workspace", "dir");
        let mut state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path: None,
            tool_workspace: Some(workspace.clone()),
            tool_log_path: String::from("runtime.log"),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 4096,
            bootstrap_root: None,
            channel_id: None,
            tool_ids: vec![axiom_adapters::DEFAULT_TOOL_ID.to_owned()],
            context_root: None,
        });

        let execution = state.apply_template(
            &IntentTemplate::Write {
                key: String::from("beta"),
                value: String::from("7"),
            },
            "cli-2",
            DecisionOutcome::Accepted,
        );
        assert_eq!(execution.provider, RuntimeComposeStep::Applied);
        assert_eq!(execution.memory, RuntimeComposeStep::Skipped);
        assert_eq!(execution.tool, RuntimeComposeStep::Applied);

        let log_path = workspace.join("runtime.log");
        let log = fs::read_to_string(&log_path).expect("tool log should exist");
        assert!(log.contains("provider=intent=cli-2 kind=write key=beta value=7"), "log={log}");

        let _ = fs::remove_file(log_path);
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn runtime_compose_appends_bootstrap_context_from_workspace_files() {
        let workspace = unique_path("bootstrap-workspace", "dir");
        let bootstrap_root = unique_path("bootstrap-root", "dir");
        fs::create_dir_all(&bootstrap_root).expect("bootstrap root should be created");
        fs::write(
            bootstrap_root.join("AGENTS.md"),
            "Respond with strict, deterministic output.",
        )
        .expect("bootstrap file should be writable");

        let mut state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path: None,
            tool_workspace: Some(workspace.clone()),
            tool_log_path: String::from("runtime.log"),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 4096,
            bootstrap_root: Some(bootstrap_root.clone()),
            channel_id: None,
            tool_ids: vec![axiom_adapters::DEFAULT_TOOL_ID.to_owned()],
            context_root: None,
        });

        let execution = state.apply_template(
            &IntentTemplate::Write {
                key: String::from("gamma"),
                value: String::from("9"),
            },
            "cli-3",
            DecisionOutcome::Accepted,
        );
        assert_eq!(execution.provider, RuntimeComposeStep::Applied);
        assert_eq!(execution.memory, RuntimeComposeStep::Skipped);
        assert_eq!(execution.tool, RuntimeComposeStep::Applied);

        let log_path = workspace.join("runtime.log");
        let log = fs::read_to_string(&log_path).expect("tool log should exist");
        assert!(log.contains("provider=intent=cli-3 kind=write key=gamma value=9"));
        assert!(log.contains(
            "bootstrap_context:\n[AGENTS.md]\nRespond with strict, deterministic output."
        ));

        let _ = fs::remove_file(log_path);
        let _ = fs::remove_dir_all(workspace);
        let _ = fs::remove_dir_all(bootstrap_root);
    }

    #[test]
    fn runtime_compose_separates_provider_id_from_model() {
        let workspace = unique_path("provider-model-split", "dir");
        let mut state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path: None,
            tool_workspace: Some(workspace.clone()),
            tool_log_path: String::from("runtime.log"),
            provider_id: String::from("mock-local"),
            provider_model: String::from("status-model"),
            max_tokens: 4096,
            bootstrap_root: None,
            channel_id: None,
            tool_ids: vec![axiom_adapters::DEFAULT_TOOL_ID.to_owned()],
            context_root: None,
        });

        let health = state.health();
        assert_eq!(health.provider_model, "status-model");

        let execution = state.apply_template(
            &IntentTemplate::Write {
                key: String::from("delta"),
                value: String::from("5"),
            },
            "cli-4",
            DecisionOutcome::Accepted,
        );
        assert_eq!(execution.provider, RuntimeComposeStep::Applied);
        assert_eq!(execution.memory, RuntimeComposeStep::Skipped);
        assert_eq!(execution.tool, RuntimeComposeStep::Applied);

        let log_path = workspace.join("runtime.log");
        let log = fs::read_to_string(&log_path).expect("tool log should exist");
        assert!(log.contains("provider=intent=cli-4 kind=write key=delta value=5"), "log={log}");

        let _ = fs::remove_file(log_path);
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn runtime_compose_reports_tool_failure_explicitly() {
        let workspace = unique_path("tool-failure-workspace", "dir");
        fs::create_dir_all(&workspace).expect("workspace should be created");

        let mut state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path: None,
            tool_workspace: Some(workspace.clone()),
            tool_log_path: String::from("../escape.log"),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 4096,
            bootstrap_root: None,
            channel_id: None,
            tool_ids: vec![axiom_adapters::DEFAULT_TOOL_ID.to_owned()],
            context_root: None,
        });

        let execution = state.apply_template(
            &IntentTemplate::Write {
                key: String::from("epsilon"),
                value: String::from("5"),
            },
            "cli-5",
            DecisionOutcome::Accepted,
        );

        assert_eq!(execution.provider, RuntimeComposeStep::Applied);
        assert_eq!(execution.memory, RuntimeComposeStep::Skipped);
        match &execution.tool {
            RuntimeComposeStep::Failed(message) => {
                assert!(
                    message.contains("path escapes workspace boundary"),
                    "message={message}"
                );
            }
            other => panic!("expected tool failure, got {other:?}"),
        }
        assert!(execution.first_failure().is_some());

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn runtime_compose_health_exposes_component_init_failures() {
        let memory_dir = unique_path("memory-init-failure", "dir");
        let tool_file = unique_path("tool-init-failure", "txt");
        let bootstrap_root = unique_path("bootstrap-init-failure", "dir");

        fs::create_dir_all(&memory_dir).expect("memory dir should be creatable");
        fs::write(&tool_file, "not-a-directory").expect("tool file should be writable");

        let state = RuntimeComposeState::new(RuntimeComposeConfig {
            memory_path: Some(memory_dir.clone()),
            tool_workspace: Some(tool_file.clone()),
            tool_log_path: String::from("runtime.log"),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 4096,
            bootstrap_root: Some(bootstrap_root.clone()),
            channel_id: None,
            tool_ids: vec![axiom_adapters::DEFAULT_TOOL_ID.to_owned()],
            context_root: None,
        });

        let health = state.health();
        assert_eq!(health.provider_id, "mock-local");

        assert!(!health.memory_enabled);
        assert_eq!(health.memory_state, "failed");
        assert!(health.memory_detail.contains("error="));

        assert!(!health.tool_enabled);
        assert_eq!(health.tool_state, "failed");
        assert!(health.tool_detail.contains("error="));

        assert_eq!(health.bootstrap_state, "failed");
        assert!(health.bootstrap_detail.contains("no bootstrap content"));

        let _ = fs::remove_dir_all(memory_dir);
        let _ = fs::remove_file(tool_file);
        let _ = fs::remove_dir_all(bootstrap_root);
    }
}
