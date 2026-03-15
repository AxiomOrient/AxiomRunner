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
        Self::acquire_inner(workspace_root, command_name, true)
    }

    fn acquire_inner(
        workspace_root: &Path,
        command_name: &str,
        allow_stale_recovery: bool,
    ) -> Result<Self, String> {
        let path = lock_path(workspace_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "create workspace lock directory '{}' failed: {error}",
                    parent.display()
                )
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
                if allow_stale_recovery && lock_holder_is_stale(&holder) {
                    match fs::remove_file(&path) {
                        Ok(()) => return Self::acquire_inner(workspace_root, command_name, false),
                        Err(remove_error) if remove_error.kind() == ErrorKind::NotFound => {
                            return Self::acquire_inner(workspace_root, command_name, false);
                        }
                        Err(remove_error) => {
                            return Err(format!(
                                "workspace lock stale recovery failed path={} holder={} error={remove_error}",
                                path.display(),
                                holder
                            ));
                        }
                    }
                }
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
    workspace_root.join(".axiomrunner").join("runtime.lock")
}

fn lock_holder_is_stale(holder: &str) -> bool {
    let pid = holder
        .split_whitespace()
        .find_map(|part| part.strip_prefix("pid="))
        .and_then(|raw| raw.parse::<u32>().ok());
    let Some(pid) = pid else {
        return false;
    };
    !process_is_alive(pid)
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(true)
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{WorkspaceLock, lock_holder_is_stale, lock_path};
    use std::fs;

    fn unique_workspace(label: &str) -> std::path::PathBuf {
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiomrunner-workspace-lock-{label}-{}-{tick}",
            std::process::id()
        ))
    }

    #[test]
    fn stale_holder_is_detected_from_dead_pid() {
        assert!(lock_holder_is_stale("pid=999999 command=run"));
        assert!(!lock_holder_is_stale(&format!(
            "pid={} command=run",
            std::process::id()
        )));
    }

    #[test]
    fn acquire_recovers_stale_lock_once() {
        let workspace = unique_workspace("stale");
        fs::create_dir_all(workspace.join(".axiomrunner")).expect("lock dir should exist");
        fs::write(
            workspace.join(".axiomrunner/runtime.lock"),
            "pid=999999 command=run\n",
        )
        .expect("stale lock should exist");

        let lock = WorkspaceLock::acquire(&workspace, "run").expect("stale lock should recover");
        assert!(lock_path(&workspace).exists());
        drop(lock);
        assert!(!lock_path(&workspace).exists());

        let _ = fs::remove_dir_all(workspace);
    }
}
