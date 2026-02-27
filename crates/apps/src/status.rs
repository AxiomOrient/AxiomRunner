use crate::display::mode_name;
use crate::doctor::{DAEMON_HEALTH_ENV, DaemonHealthInput};
use axiom_core::ExecutionMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateStatusInput {
    pub revision: u64,
    pub mode: ExecutionMode,
    pub facts: usize,
    pub denied: u64,
    pub audit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStatusInput {
    pub provider_id: String,
    pub provider_model: String,
    pub memory_enabled: bool,
    pub memory_state: String,
    pub tool_enabled: bool,
    pub tool_state: String,
    pub bootstrap_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusInput {
    pub state: StateStatusInput,
    pub runtime: RuntimeStatusInput,
    pub daemon_health: DaemonHealthInput,
    pub channels: ChannelStatusInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelStatusInput {
    Listed { total: usize, running: usize },
    Error { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub state: StateSnapshot,
    pub runtime: RuntimeSnapshot,
    pub daemon: DaemonSnapshot,
    pub channels: ChannelSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSnapshot {
    pub revision: u64,
    pub mode: ExecutionMode,
    pub facts: usize,
    pub denied: u64,
    pub audit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    pub provider_id: String,
    pub provider_model: String,
    pub memory_enabled: bool,
    pub memory_state: String,
    pub tool_enabled: bool,
    pub tool_state: String,
    pub bootstrap_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonSnapshot {
    MissingEnv,
    InvalidEnv {
        reason: &'static str,
    },
    ReadError {
        path: String,
        kind: &'static str,
    },
    ParseError {
        path: String,
        detail: String,
    },
    Snapshot {
        tick: u64,
        state: String,
        in_flight: String,
        queue_depth: u64,
        completed: u64,
        failed: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelSnapshot {
    Listed { total: usize, running: usize },
    Error { detail: String },
}

impl From<StatusInput> for StatusSnapshot {
    fn from(input: StatusInput) -> Self {
        StatusSnapshot {
            state: StateSnapshot {
                revision: input.state.revision,
                mode: input.state.mode,
                facts: input.state.facts,
                denied: input.state.denied,
                audit: input.state.audit,
            },
            runtime: RuntimeSnapshot {
                provider_id: input.runtime.provider_id,
                provider_model: input.runtime.provider_model,
                memory_enabled: input.runtime.memory_enabled,
                memory_state: input.runtime.memory_state,
                tool_enabled: input.runtime.tool_enabled,
                tool_state: input.runtime.tool_state,
                bootstrap_state: input.runtime.bootstrap_state,
            },
            daemon: daemon_snapshot(input.daemon_health),
            channels: match input.channels {
                ChannelStatusInput::Listed { total, running } => {
                    ChannelSnapshot::Listed { total, running }
                }
                ChannelStatusInput::Error { detail } => ChannelSnapshot::Error { detail },
            },
        }
    }
}

pub fn render_status_lines(snapshot: &StatusSnapshot) -> Vec<String> {
    let mut lines = Vec::with_capacity(4);
    lines.push(format!(
        "status revision={} mode={} facts={} denied={} audit={}",
        snapshot.state.revision,
        mode_name(snapshot.state.mode),
        snapshot.state.facts,
        snapshot.state.denied,
        snapshot.state.audit
    ));
    lines.push(format!(
        "status runtime provider_id={} provider_model={} memory_enabled={} memory_state={} tool_enabled={} tool_state={} bootstrap_state={}",
        snapshot.runtime.provider_id,
        snapshot.runtime.provider_model,
        snapshot.runtime.memory_enabled,
        snapshot.runtime.memory_state,
        snapshot.runtime.tool_enabled,
        snapshot.runtime.tool_state,
        snapshot.runtime.bootstrap_state
    ));
    lines.push(render_daemon_line(&snapshot.daemon));
    lines.push(render_channels_line(&snapshot.channels));
    lines
}

fn daemon_snapshot(input: DaemonHealthInput) -> DaemonSnapshot {
    match input {
        DaemonHealthInput::MissingEnv => DaemonSnapshot::MissingEnv,
        DaemonHealthInput::InvalidEnvValue { reason } => DaemonSnapshot::InvalidEnv { reason },
        DaemonHealthInput::ReadError { path, kind } => DaemonSnapshot::ReadError {
            path,
            kind: kind.as_str(),
        },
        DaemonHealthInput::ParseError { path, error } => DaemonSnapshot::ParseError {
            path,
            detail: error.as_detail(),
        },
        DaemonHealthInput::Snapshot { snapshot, .. } => DaemonSnapshot::Snapshot {
            tick: snapshot.tick,
            state: snapshot.state,
            in_flight: snapshot.in_flight,
            queue_depth: snapshot.queue_depth,
            completed: snapshot.completed,
            failed: snapshot.failed,
        },
    }
}

fn render_daemon_line(daemon: &DaemonSnapshot) -> String {
    match daemon {
        DaemonSnapshot::MissingEnv => {
            format!("status daemon state=missing env={DAEMON_HEALTH_ENV}")
        }
        DaemonSnapshot::InvalidEnv { reason } => {
            format!("status daemon state=invalid_env reason={reason}")
        }
        DaemonSnapshot::ReadError { path, kind } => {
            format!("status daemon state=read_error kind={kind} path={path}")
        }
        DaemonSnapshot::ParseError { path, detail } => {
            format!("status daemon state=parse_error detail={detail} path={path}")
        }
        DaemonSnapshot::Snapshot {
            tick,
            state,
            in_flight,
            queue_depth,
            completed,
            failed,
        } => format!(
            "status daemon state=ok tick={tick} daemon_state={state} in_flight={in_flight} queue_depth={queue_depth} completed={completed} failed={failed}"
        ),
    }
}

fn render_channels_line(channels: &ChannelSnapshot) -> String {
    match channels {
        ChannelSnapshot::Listed { total, running } => {
            format!("status channels total={total} running={running}")
        }
        ChannelSnapshot::Error { detail } => {
            format!("status channels state=error detail={detail}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChannelStatusInput, RuntimeStatusInput, StateStatusInput, StatusInput, StatusSnapshot,
        render_status_lines,
    };
    use crate::doctor::{DaemonHealthInput, DaemonHealthSnapshot};
    use axiom_core::ExecutionMode;

    #[test]
    fn status_render_includes_runtime_daemon_and_channels_summary() {
        let snapshot = StatusSnapshot::from(StatusInput {
            state: StateStatusInput {
                revision: 7,
                mode: ExecutionMode::Active,
                facts: 2,
                denied: 1,
                audit: 3,
            },
            runtime: RuntimeStatusInput {
                provider_id: String::from("mock-local"),
                provider_model: String::from("mock-local"),
                memory_enabled: true,
                memory_state: String::from("ready"),
                tool_enabled: false,
                tool_state: String::from("disabled"),
                bootstrap_state: String::from("disabled"),
            },
            daemon_health: DaemonHealthInput::MissingEnv,
            channels: ChannelStatusInput::Listed {
                total: 4,
                running: 2,
            },
        });

        let lines = render_status_lines(&snapshot);
        assert_eq!(
            lines[0],
            "status revision=7 mode=active facts=2 denied=1 audit=3"
        );
        assert_eq!(
            lines[1],
            "status runtime provider_id=mock-local provider_model=mock-local memory_enabled=true memory_state=ready tool_enabled=false tool_state=disabled bootstrap_state=disabled"
        );
        assert_eq!(
            lines[2],
            "status daemon state=missing env=AXIOM_DAEMON_HEALTH_PATH"
        );
        assert_eq!(lines[3], "status channels total=4 running=2");
    }

    #[test]
    fn status_render_projects_daemon_snapshot() {
        let snapshot = StatusSnapshot::from(StatusInput {
            state: StateStatusInput {
                revision: 0,
                mode: ExecutionMode::Active,
                facts: 0,
                denied: 0,
                audit: 0,
            },
            runtime: RuntimeStatusInput {
                provider_id: String::from("openrouter"),
                provider_model: String::from("model"),
                memory_enabled: false,
                memory_state: String::from("disabled"),
                tool_enabled: false,
                tool_state: String::from("failed"),
                bootstrap_state: String::from("ready"),
            },
            daemon_health: DaemonHealthInput::Snapshot {
                path: String::from("/tmp/health.status"),
                snapshot: DaemonHealthSnapshot {
                    tick: 9,
                    state: String::from("running"),
                    state_detail: String::from("item=sync attempt=1"),
                    reason: String::from("-"),
                    in_flight: String::from("sync"),
                    in_flight_attempt: 1,
                    queue_depth: 3,
                    completed: 4,
                    failed: 1,
                },
            },
            channels: ChannelStatusInput::Error {
                detail: String::from("store missing"),
            },
        });

        let lines = render_status_lines(&snapshot);
        assert_eq!(
            lines[2],
            "status daemon state=ok tick=9 daemon_state=running in_flight=sync queue_depth=3 completed=4 failed=1"
        );
        assert_eq!(lines[3], "status channels state=error detail=store missing");
    }
}
