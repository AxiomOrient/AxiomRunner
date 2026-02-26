use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCapability {
    Shell,
    FileRead,
    FileWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolPolicy {
    pub allow_shell: bool,
    pub allow_file_read: bool,
    pub allow_file_write: bool,
    pub max_shell_command_bytes: usize,
    pub max_file_read_bytes: usize,
    pub max_file_write_bytes: usize,
}

impl ToolPolicy {
    pub const fn deny_all() -> Self {
        Self {
            allow_shell: false,
            allow_file_read: false,
            allow_file_write: false,
            max_shell_command_bytes: 0,
            max_file_read_bytes: 0,
            max_file_write_bytes: 0,
        }
    }
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self::deny_all()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolRequest<'a> {
    Shell {
        command: &'a str,
    },
    FileRead {
        path: &'a str,
    },
    FileWrite {
        path: &'a str,
        contents: &'a str,
        append: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolResult {
    Shell(ShellOutput),
    FileRead(FileReadOutput),
    FileWrite(FileWriteOutput),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellOutput {
    pub status_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReadOutput {
    pub path: PathBuf,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWriteOutput {
    pub path: PathBuf,
    pub bytes_written: usize,
}

#[derive(Debug)]
pub enum ToolError {
    InvalidInput(&'static str),
    InputTooLarge {
        field: &'static str,
        limit: usize,
        actual: usize,
    },
    PolicyDenied(ToolCapability),
    WorkspaceEscape {
        requested: String,
    },
    Io {
        operation: &'static str,
        source: io::Error,
    },
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::InvalidInput(reason) => write!(f, "invalid tool input: {reason}"),
            ToolError::InputTooLarge {
                field,
                limit,
                actual,
            } => write!(f, "{field} exceeds limit ({actual} > {limit})"),
            ToolError::PolicyDenied(capability) => {
                write!(f, "tool policy denied capability {capability:?}")
            }
            ToolError::WorkspaceEscape { requested } => {
                write!(f, "path escapes workspace boundary: {requested}")
            }
            ToolError::Io { operation, source } => {
                write!(f, "{operation} failed: {source}")
            }
        }
    }
}

impl std::error::Error for ToolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ToolError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceTool {
    workspace_root: PathBuf,
    policy: ToolPolicy,
}

impl WorkspaceTool {
    pub fn new(workspace_root: impl Into<PathBuf>, policy: ToolPolicy) -> Result<Self, ToolError> {
        let mut root = workspace_root.into();
        if root.as_os_str().is_empty() {
            return Err(ToolError::InvalidInput("workspace root is required"));
        }

        if !root.is_absolute() {
            let cwd = std::env::current_dir().map_err(|err| ToolError::Io {
                operation: "read_current_dir",
                source: err,
            })?;
            root = cwd.join(root);
        }

        let canonical_root = fs::canonicalize(&root).map_err(|err| ToolError::Io {
            operation: "canonicalize_workspace_root",
            source: err,
        })?;
        if !canonical_root.is_dir() {
            return Err(ToolError::InvalidInput(
                "workspace root must be a directory",
            ));
        }

        Ok(Self {
            workspace_root: canonical_root,
            policy,
        })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn execute(&self, request: ToolRequest<'_>) -> Result<ToolResult, ToolError> {
        match request {
            ToolRequest::Shell { command } => self.execute_shell(command),
            ToolRequest::FileRead { path } => self.execute_file_read(path),
            ToolRequest::FileWrite {
                path,
                contents,
                append,
            } => self.execute_file_write(path, contents, append),
        }
    }

    fn execute_shell(&self, command: &str) -> Result<ToolResult, ToolError> {
        if !self.policy.allow_shell {
            return Err(ToolError::PolicyDenied(ToolCapability::Shell));
        }
        validate_non_empty("shell command", command)?;
        validate_no_nul("shell command", command)?;
        validate_size(
            "shell_command",
            self.policy.max_shell_command_bytes,
            command.len(),
        )?;

        if contains_shell_metachar(command) {
            return Err(ToolError::PolicyDenied(ToolCapability::Shell));
        }

        let mut parts = command.split_ascii_whitespace();
        let binary = parts.next().ok_or(ToolError::InvalidInput("shell command"))?;
        if !ALLOWED_SHELL_PROGRAMS.contains(&binary) {
            return Err(ToolError::PolicyDenied(ToolCapability::Shell));
        }
        let rest_args: Vec<&str> = parts.collect();

        let output = Command::new(binary)
            .args(&rest_args)
            .output()
            .map_err(|err| ToolError::Io {
                operation: "execute_shell",
                source: err,
            })?;

        Ok(ToolResult::Shell(ShellOutput {
            status_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }))
    }

    fn execute_file_read(&self, path: &str) -> Result<ToolResult, ToolError> {
        if !self.policy.allow_file_read {
            return Err(ToolError::PolicyDenied(ToolCapability::FileRead));
        }
        validate_non_empty("file path", path)?;
        validate_no_nul("file path", path)?;

        let resolved_path = self.resolve_workspace_path(path)?;
        let metadata = fs::metadata(&resolved_path).map_err(|err| ToolError::Io {
            operation: "read_metadata",
            source: err,
        })?;
        let size = metadata.len() as usize;
        validate_size("file_read", self.policy.max_file_read_bytes, size)?;

        let contents = fs::read_to_string(&resolved_path).map_err(|err| ToolError::Io {
            operation: "read_file",
            source: err,
        })?;

        Ok(ToolResult::FileRead(FileReadOutput {
            path: resolved_path,
            contents,
        }))
    }

    fn execute_file_write(
        &self,
        path: &str,
        contents: &str,
        append: bool,
    ) -> Result<ToolResult, ToolError> {
        if !self.policy.allow_file_write {
            return Err(ToolError::PolicyDenied(ToolCapability::FileWrite));
        }
        validate_non_empty("file path", path)?;
        validate_no_nul("file path", path)?;
        validate_no_nul("file contents", contents)?;
        validate_size(
            "file_write",
            self.policy.max_file_write_bytes,
            contents.len(),
        )?;

        let resolved_path = self.resolve_workspace_path(path)?;
        if let Some(parent) = resolved_path.parent() {
            fs::create_dir_all(parent).map_err(|err| ToolError::Io {
                operation: "create_parent_directories",
                source: err,
            })?;
        }

        let mut options = OpenOptions::new();
        options.create(true);
        if append {
            options.append(true);
        } else {
            options.write(true).truncate(true);
        }

        let mut file = options.open(&resolved_path).map_err(|err| ToolError::Io {
            operation: "open_file_for_write",
            source: err,
        })?;
        file.write_all(contents.as_bytes())
            .map_err(|err| ToolError::Io {
                operation: "write_file",
                source: err,
            })?;

        Ok(ToolResult::FileWrite(FileWriteOutput {
            path: resolved_path,
            bytes_written: contents.len(),
        }))
    }

    fn resolve_workspace_path(&self, requested_path: &str) -> Result<PathBuf, ToolError> {
        let requested = Path::new(requested_path);
        let joined = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.workspace_root.join(requested)
        };
        let normalized = normalize_path(&joined);
        if !normalized.starts_with(&self.workspace_root) {
            return Err(ToolError::WorkspaceEscape {
                requested: requested_path.to_owned(),
            });
        }

        if normalized.exists() {
            let canonical = fs::canonicalize(&normalized).map_err(|err| ToolError::Io {
                operation: "canonicalize_existing_path",
                source: err,
            })?;
            if !canonical.starts_with(&self.workspace_root) {
                return Err(ToolError::WorkspaceEscape {
                    requested: requested_path.to_owned(),
                });
            }
            return Ok(canonical);
        }

        let mut existing_ancestor = normalized.clone();
        while !existing_ancestor.exists() {
            if !existing_ancestor.pop() {
                return Err(ToolError::WorkspaceEscape {
                    requested: requested_path.to_owned(),
                });
            }
        }

        let canonical_ancestor =
            fs::canonicalize(&existing_ancestor).map_err(|err| ToolError::Io {
                operation: "canonicalize_existing_ancestor",
                source: err,
            })?;
        if !canonical_ancestor.starts_with(&self.workspace_root) {
            return Err(ToolError::WorkspaceEscape {
                requested: requested_path.to_owned(),
            });
        }

        let suffix = normalized
            .strip_prefix(&existing_ancestor)
            .map_err(|_| ToolError::InvalidInput("invalid file path"))?;
        let resolved = canonical_ancestor.join(suffix);
        if !resolved.starts_with(&self.workspace_root) {
            return Err(ToolError::WorkspaceEscape {
                requested: requested_path.to_owned(),
            });
        }

        Ok(resolved)
    }
}

// removed: env/printenv expose process environment variables including API keys
const ALLOWED_SHELL_PROGRAMS: &[&str] = &["echo", "cat", "ls", "pwd", "date", "printf"];

fn contains_shell_metachar(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(
            c,
            '|' | '&' | ';' | '$' | '`' | '>' | '<' | '(' | ')' | '{' | '}' | '!' | '\n' | '\r'
        )
    })
}

fn validate_non_empty(field: &'static str, input: &str) -> Result<(), ToolError> {
    if input.trim().is_empty() {
        return Err(ToolError::InvalidInput(field));
    }
    Ok(())
}

fn validate_no_nul(field: &'static str, input: &str) -> Result<(), ToolError> {
    if input.contains('\0') {
        return Err(ToolError::InvalidInput(field));
    }
    Ok(())
}

fn validate_size(field: &'static str, limit: usize, actual: usize) -> Result<(), ToolError> {
    if actual > limit {
        return Err(ToolError::InputTooLarge {
            field,
            limit,
            actual,
        });
    }
    Ok(())
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRegistryKind {
    Memory,
    Browser,
    Delegate,
    Composio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolRegistryEntry {
    pub id: &'static str,
    pub aliases: &'static [&'static str],
    pub label: &'static str,
    pub kind: ToolRegistryKind,
}

pub const DEFAULT_TOOL_ID: &str = "memory";

const MEMORY_ALIASES: [&str; 1] = ["tool.memory"];
const BROWSER_ALIASES: [&str; 2] = ["tool.browser", "browser_open"];
const DELEGATE_ALIASES: [&str; 1] = ["tool.delegate"];
const COMPOSIO_ALIASES: [&str; 1] = ["tool.composio"];

const TOOL_REGISTRY: [ToolRegistryEntry; 4] = [
    ToolRegistryEntry {
        id: DEFAULT_TOOL_ID,
        aliases: &MEMORY_ALIASES,
        label: "In-process memory store/recall/forget tool",
        kind: ToolRegistryKind::Memory,
    },
    ToolRegistryEntry {
        id: "browser",
        aliases: &BROWSER_ALIASES,
        label: "Allowlisted browser open/current tool",
        kind: ToolRegistryKind::Browser,
    },
    ToolRegistryEntry {
        id: "delegate",
        aliases: &DELEGATE_ALIASES,
        label: "Sub-agent delegation tool with depth limit",
        kind: ToolRegistryKind::Delegate,
    },
    ToolRegistryEntry {
        id: "composio",
        aliases: &COMPOSIO_ALIASES,
        label: "Execute Composio actions via API",
        kind: ToolRegistryKind::Composio,
    },
];

pub fn tool_registry() -> &'static [ToolRegistryEntry] {
    &TOOL_REGISTRY
}

pub fn resolve_tool_id(name: &str) -> Option<&'static str> {
    resolve_tool_entry(name).map(|entry| entry.id)
}

pub fn build_contract_tool(name: &str) -> Result<Box<dyn crate::contracts::ToolAdapter>, String> {
    let entry = resolve_tool_entry(name).ok_or_else(|| {
        let available = tool_registry()
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>()
            .join(", ");
        format!("unsupported tool '{name}'. supported tools: {available}")
    })?;

    let tool: Box<dyn crate::contracts::ToolAdapter> = match entry.kind {
        ToolRegistryKind::Memory => Box::new(crate::tool_memory::MemoryToolAdapter::in_memory(
            crate::tool_memory::MemoryToolConfig::default(),
        )),
        ToolRegistryKind::Browser => Box::new(crate::tool_browser::BrowserToolAdapter::new(
            crate::tool_browser::BrowserToolConfig::default(),
        )),
        ToolRegistryKind::Delegate => {
            let adapter = crate::tool_delegate::DelegateToolAdapter::new(
                crate::tool_delegate::DEFAULT_MAX_DELEGATE_DEPTH,
            )
            .map_err(|e| format!("failed to build delegate tool: {e}"))?;
            Box::new(adapter)
        }
        ToolRegistryKind::Composio => {
            let adapter = crate::tool_composio::ComposioToolAdapter::new()
                .map_err(|e| format!("failed to build composio tool: {e}"))?;
            Box::new(adapter)
        }
    };

    Ok(tool)
}

fn resolve_tool_entry(name: &str) -> Option<&'static ToolRegistryEntry> {
    let key = name.trim();
    if key.is_empty() {
        return None;
    }

    tool_registry().iter().find(|entry| {
        entry.id.eq_ignore_ascii_case(key)
            || entry
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(key))
    })
}
