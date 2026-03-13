use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct WorkspaceLock {
    path: PathBuf,
    _file: File,
}

impl WorkspaceLock {
    pub fn acquire(workspace_root: &Path, command_name: &str) -> Result<Self, String> {
        let path = lock_path(workspace_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("create workspace lock directory '{}' failed: {error}", parent.display())
            })?;
        }

        let mut file = match OpenOptions::new().create_new(true).write(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let holder = fs::read_to_string(&path)
                    .ok()
                    .map(|raw| raw.trim().to_owned())
                    .filter(|raw| !raw.is_empty())
                    .unwrap_or_else(|| String::from("holder=unknown"));
                return Err(format!(
                    "workspace lock is active path={} {}",
                    path.display(),
                    holder
                ));
            }
            Err(error) => {
                return Err(format!(
                    "open workspace lock '{}' failed: {error}",
                    path.display()
                ));
            }
        };

        let pid = std::process::id();
        writeln!(file, "pid={pid} command={command_name}").map_err(|error| {
            format!("write workspace lock '{}' failed: {error}", path.display())
        })?;

        Ok(Self { path, _file: file })
    }
}

impl Drop for WorkspaceLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn lock_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".axonrunner").join("runtime.lock")
}
