use std::fs;
use std::path::{Path, PathBuf};
use crate::env_util::resolve_env_path;
use crate::time_util::unix_now_seconds;
use crate::parse_util::{parse_bool, parse_number};

const ENV_SERVICE_STATE_PATH: &str = "AXIOM_SERVICE_STATE_PATH";
const DEFAULT_SERVICE_STATE_PATH: &str = ".axiom/service/state.db";
const SERVICE_STATE_FORMAT: &str = "format=axiom-service-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceAction {
    Install,
    Start,
    Stop,
    Status,
    Uninstall,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceState {
    pub installed: bool,
    pub running: bool,
    pub install_count: u64,
    pub start_count: u64,
    pub stop_count: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceResult {
    Installed { path: PathBuf, state: ServiceState },
    Started { path: PathBuf, state: ServiceState },
    Stopped { path: PathBuf, state: ServiceState },
    Status { path: PathBuf, state: ServiceState },
    Uninstalled { path: PathBuf, removed: bool },
}

pub fn execute_service_action(action: ServiceAction) -> Result<ServiceResult, String> {
    let path = resolve_env_path(ENV_SERVICE_STATE_PATH, Path::new(DEFAULT_SERVICE_STATE_PATH))?;
    execute_service_action_at(action, &path, unix_now_seconds())
}

fn execute_service_action_at(
    action: ServiceAction,
    path: &Path,
    now: u64,
) -> Result<ServiceResult, String> {
    let state = load_state(path)?;
    match apply_action(state, action, now)? {
        Transition::WriteInstalled(state) => {
            save_state(path, &state)?;
            Ok(ServiceResult::Installed {
                path: path.to_path_buf(),
                state,
            })
        }
        Transition::WriteStarted(state) => {
            save_state(path, &state)?;
            Ok(ServiceResult::Started {
                path: path.to_path_buf(),
                state,
            })
        }
        Transition::WriteStopped(state) => {
            save_state(path, &state)?;
            Ok(ServiceResult::Stopped {
                path: path.to_path_buf(),
                state,
            })
        }
        Transition::ReadStatus(state) => Ok(ServiceResult::Status {
            path: path.to_path_buf(),
            state,
        }),
        Transition::RemoveState => {
            let removed = if path.exists() {
                fs::remove_file(path).map_err(|error| {
                    format!(
                        "failed to remove service state '{}': {error}",
                        path.display()
                    )
                })?;
                true
            } else {
                false
            };
            Ok(ServiceResult::Uninstalled {
                path: path.to_path_buf(),
                removed,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Transition {
    WriteInstalled(ServiceState),
    WriteStarted(ServiceState),
    WriteStopped(ServiceState),
    ReadStatus(ServiceState),
    RemoveState,
}

fn apply_action(
    state: ServiceState,
    action: ServiceAction,
    now: u64,
) -> Result<Transition, String> {
    match action {
        ServiceAction::Install => {
            let next = ServiceState {
                installed: true,
                running: false,
                install_count: state.install_count.saturating_add(1),
                start_count: state.start_count,
                stop_count: state.stop_count,
                updated_at: now,
            };
            Ok(Transition::WriteInstalled(next))
        }
        ServiceAction::Start => {
            if !state.installed {
                return Err(String::from("service is not installed"));
            }

            let next = ServiceState {
                installed: true,
                running: true,
                install_count: state.install_count,
                start_count: state.start_count.saturating_add(1),
                stop_count: state.stop_count,
                updated_at: now,
            };
            Ok(Transition::WriteStarted(next))
        }
        ServiceAction::Stop => {
            if !state.installed {
                return Err(String::from("service is not installed"));
            }

            let next = ServiceState {
                installed: true,
                running: false,
                install_count: state.install_count,
                start_count: state.start_count,
                stop_count: state.stop_count.saturating_add(1),
                updated_at: now,
            };
            Ok(Transition::WriteStopped(next))
        }
        ServiceAction::Status => Ok(Transition::ReadStatus(state)),
        ServiceAction::Uninstall => Ok(Transition::RemoveState),
    }
}


fn load_state(path: &Path) -> Result<ServiceState, String> {
    if !path.exists() {
        return Ok(ServiceState::default());
    }

    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read service state '{}': {error}", path.display()))?;
    parse_state(&contents).map_err(|error| {
        format!(
            "failed to parse service state '{}': {error}",
            path.display()
        )
    })
}

fn save_state(path: &Path, state: &ServiceState) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create service state directory '{}': {error}",
                parent.display()
            )
        })?;
    }

    let body = render_state(state);
    fs::write(path, body).map_err(|error| {
        format!(
            "failed to write service state '{}': {error}",
            path.display()
        )
    })
}

fn render_state(state: &ServiceState) -> String {
    format!(
        "{SERVICE_STATE_FORMAT}\ninstalled={}\nrunning={}\ninstall_count={}\nstart_count={}\nstop_count={}\nupdated_at={}\n",
        state.installed,
        state.running,
        state.install_count,
        state.start_count,
        state.stop_count,
        state.updated_at
    )
}

fn parse_state(contents: &str) -> Result<ServiceState, String> {
    let mut installed = None;
    let mut running = None;
    let mut install_count = None;
    let mut start_count = None;
    let mut stop_count = None;
    let mut updated_at = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line == SERVICE_STATE_FORMAT {
            continue;
        }

        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid key/value line '{line}'"))?;

        match key {
            "installed" => installed = Some(parse_bool(value, "installed")?),
            "running" => running = Some(parse_bool(value, "running")?),
            "install_count" => install_count = Some(parse_number::<u64>(value, "install_count")?),
            "start_count" => start_count = Some(parse_number::<u64>(value, "start_count")?),
            "stop_count" => stop_count = Some(parse_number::<u64>(value, "stop_count")?),
            "updated_at" => updated_at = Some(parse_number::<u64>(value, "updated_at")?),
            _ => return Err(format!("unknown service state key '{key}'")),
        }
    }

    Ok(ServiceState {
        installed: required(installed, "installed")?,
        running: required(running, "running")?,
        install_count: required(install_count, "install_count")?,
        start_count: required(start_count, "start_count")?,
        stop_count: required(stop_count, "stop_count")?,
        updated_at: required(updated_at, "updated_at")?,
    })
}

fn required<T>(value: Option<T>, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("missing field '{field}'"))
}

#[cfg(test)]
mod tests {
    use super::{ServiceAction, ServiceResult, execute_service_action_at};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_path(label: &str, extension: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiom-service-{label}-{}-{tick}.{extension}",
            std::process::id()
        ))
    }

    #[test]
    fn service_lifecycle_install_start_stop_status_uninstall() {
        let path = unique_path("lifecycle", "db");

        let installed =
            execute_service_action_at(ServiceAction::Install, &path, 10).expect("install");
        match installed {
            ServiceResult::Installed { state, .. } => {
                assert!(state.installed);
                assert!(!state.running);
                assert_eq!(state.install_count, 1);
            }
            _ => panic!("expected ServiceResult::Installed"),
        }

        let started = execute_service_action_at(ServiceAction::Start, &path, 20).expect("start");
        match started {
            ServiceResult::Started { state, .. } => {
                assert!(state.installed);
                assert!(state.running);
                assert_eq!(state.start_count, 1);
            }
            _ => panic!("expected ServiceResult::Started"),
        }

        let stopped = execute_service_action_at(ServiceAction::Stop, &path, 30).expect("stop");
        match stopped {
            ServiceResult::Stopped { state, .. } => {
                assert!(state.installed);
                assert!(!state.running);
                assert_eq!(state.stop_count, 1);
            }
            _ => panic!("expected ServiceResult::Stopped"),
        }

        let status = execute_service_action_at(ServiceAction::Status, &path, 40).expect("status");
        match status {
            ServiceResult::Status { state, .. } => {
                assert!(state.installed);
                assert!(!state.running);
                assert_eq!(state.install_count, 1);
                assert_eq!(state.start_count, 1);
                assert_eq!(state.stop_count, 1);
            }
            _ => panic!("expected ServiceResult::Status"),
        }

        let uninstalled =
            execute_service_action_at(ServiceAction::Uninstall, &path, 50).expect("uninstall");
        match uninstalled {
            ServiceResult::Uninstalled { removed, .. } => assert!(removed),
            _ => panic!("expected ServiceResult::Uninstalled"),
        }

        assert!(!path.exists());
    }

    #[test]
    fn start_requires_installation() {
        let path = unique_path("start-before-install", "db");
        let err = execute_service_action_at(ServiceAction::Start, &path, 10)
            .expect_err("start should fail before install");
        assert!(err.contains("service is not installed"));

        let _ = fs::remove_file(path);
    }
}
