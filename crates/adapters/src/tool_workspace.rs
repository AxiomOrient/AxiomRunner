use ignore::WalkBuilder;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) enum WorkspacePathError {
    InvalidInput(&'static str),
    WorkspaceEscape {
        requested: String,
    },
    Io {
        operation: &'static str,
        source: io::Error,
    },
}

pub(crate) fn canonicalize_workspace_root(
    workspace_root: impl Into<PathBuf>,
) -> Result<PathBuf, WorkspacePathError> {
    let mut root = workspace_root.into();
    if root.as_os_str().is_empty() {
        return Err(WorkspacePathError::InvalidInput(
            "workspace root is required",
        ));
    }

    if !root.is_absolute() {
        let cwd = std::env::current_dir().map_err(|error| WorkspacePathError::Io {
            operation: "read_current_dir",
            source: error,
        })?;
        root = cwd.join(root);
    }

    let canonical_root = fs::canonicalize(&root).map_err(|error| WorkspacePathError::Io {
        operation: "canonicalize_workspace_root",
        source: error,
    })?;
    if !canonical_root.is_dir() {
        return Err(WorkspacePathError::InvalidInput(
            "workspace root must be a directory",
        ));
    }

    Ok(canonical_root)
}

pub(crate) fn resolve_workspace_path(
    workspace_root: &Path,
    requested_path: &str,
) -> Result<PathBuf, WorkspacePathError> {
    let requested = Path::new(requested_path);
    let joined = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        workspace_root.join(requested)
    };

    let normalized = normalize_path(&joined);
    let mut existing_ancestor = normalized.clone();
    while !existing_ancestor.exists() {
        if !existing_ancestor.pop() {
            return Err(WorkspacePathError::WorkspaceEscape {
                requested: requested_path.to_owned(),
            });
        }
    }

    let canonical_ancestor =
        fs::canonicalize(&existing_ancestor).map_err(|error| WorkspacePathError::Io {
            operation: "canonicalize_existing_ancestor",
            source: error,
        })?;

    if !canonical_ancestor.starts_with(workspace_root) {
        return Err(WorkspacePathError::WorkspaceEscape {
            requested: requested_path.to_owned(),
        });
    }

    Ok(normalized)
}

pub(crate) fn collect_files_respecting_gitignore(base: &Path) -> Result<Vec<PathBuf>, io::Error> {
    let mut files = Vec::new();
    let mut first_io_error = None;
    let internal_root = base.join(".axonrunner");

    let walker = WalkBuilder::new(base)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .require_git(false)
        .follow_links(false)
        .filter_entry(move |entry| !entry.path().starts_with(&internal_root))
        .build();

    for result in walker {
        match result {
            Ok(entry) => {
                if entry.file_type().is_some_and(|kind| kind.is_file()) {
                    files.push(entry.into_path());
                }
            }
            Err(error) => {
                if let Some(source) = error.io_error() {
                    first_io_error = Some(io::Error::new(source.kind(), source.to_string()));
                    break;
                }
            }
        }
    }

    if let Some(error) = first_io_error {
        return Err(error);
    }

    files.sort();
    Ok(files)
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
