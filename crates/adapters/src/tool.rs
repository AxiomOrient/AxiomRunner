use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolPolicy {
    pub max_file_write_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolRequest<'a> {
    FileWrite {
        path: &'a str,
        contents: &'a str,
        append: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolResult {
    FileWrite(FileWriteOutput),
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
        limit: usize,
        actual: usize,
    },
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
            ToolError::InputTooLarge { limit, actual } => {
                write!(f, "file_write exceeds limit ({actual} > {limit})")
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
            let cwd = std::env::current_dir().map_err(|error| ToolError::Io {
                operation: "read_current_dir",
                source: error,
            })?;
            root = cwd.join(root);
        }

        let canonical_root = fs::canonicalize(&root).map_err(|error| ToolError::Io {
            operation: "canonicalize_workspace_root",
            source: error,
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

    pub fn execute(&self, request: ToolRequest<'_>) -> Result<ToolResult, ToolError> {
        match request {
            ToolRequest::FileWrite {
                path,
                contents,
                append,
            } => self.execute_file_write(path, contents, append),
        }
    }

    fn execute_file_write(
        &self,
        path: &str,
        contents: &str,
        append: bool,
    ) -> Result<ToolResult, ToolError> {
        if path.trim().is_empty() {
            return Err(ToolError::InvalidInput("file path"));
        }
        if path.contains('\0') || contents.contains('\0') {
            return Err(ToolError::InvalidInput("nul byte is not allowed"));
        }
        if contents.len() > self.policy.max_file_write_bytes {
            return Err(ToolError::InputTooLarge {
                limit: self.policy.max_file_write_bytes,
                actual: contents.len(),
            });
        }

        let resolved_path = self.resolve_workspace_path(path)?;
        if let Some(parent) = resolved_path.parent() {
            fs::create_dir_all(parent).map_err(|error| ToolError::Io {
                operation: "create_parent_directories",
                source: error,
            })?;
        }

        let mut options = OpenOptions::new();
        options.create(true);
        if append {
            options.append(true);
        } else {
            options.write(true).truncate(true);
        }

        let mut file = options
            .open(&resolved_path)
            .map_err(|error| ToolError::Io {
                operation: "open_file_for_write",
                source: error,
            })?;
        file.write_all(contents.as_bytes())
            .map_err(|error| ToolError::Io {
                operation: "write_file",
                source: error,
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
        let mut existing_ancestor = normalized.clone();
        while !existing_ancestor.exists() {
            if !existing_ancestor.pop() {
                return Err(ToolError::WorkspaceEscape {
                    requested: requested_path.to_owned(),
                });
            }
        }

        let canonical_ancestor =
            fs::canonicalize(&existing_ancestor).map_err(|error| ToolError::Io {
                operation: "canonicalize_existing_ancestor",
                source: error,
            })?;

        if !canonical_ancestor.starts_with(&self.workspace_root) {
            return Err(ToolError::WorkspaceEscape {
                requested: requested_path.to_owned(),
            });
        }

        Ok(normalized)
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}
