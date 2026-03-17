#![forbid(unsafe_code)]

pub mod contracts;
pub mod error;
#[path = "memory.rs"]
pub mod memory;
pub mod provider_codex_runtime;
pub mod provider_openai;
pub mod provider_registry;
pub mod tool;
pub(crate) mod tool_workspace;
pub(crate) mod tool_write;

pub use contracts::*;
pub use error::*;
pub use memory::{MemoryTier, build_contract_memory, detect_memory_tier, tiered_memory_key};
pub use provider_registry::{
    DEFAULT_PROVIDER_ID, ProviderRegistryEntry, build_contract_provider, provider_registry,
    resolve_provider_id,
};
pub use tool::{
    RunCommandClass, ToolRiskTier, WorkspaceTool, classify_run_command_class,
    classify_tool_request_risk, is_forbidden_shell_program, validate_run_command_policy,
    validate_run_command_spec,
};

pub const RESTORE_MODE_FILE: &str = "restore_file";
pub const RESTORE_MODE_DIR: &str = "restore_dir";
pub const RESTORE_MODE_DELETE_CREATED: &str = "delete_created";

#[cfg(test)]
pub(crate) mod test_util {
    pub(crate) fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should initialize")
            .block_on(future)
    }
}
