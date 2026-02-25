use crate::display::mode_name;
use axiom_core::ExecutionMode;

pub const DAEMON_HEALTH_ENV: &str = "AXIOM_DAEMON_HEALTH_PATH";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorInput<'a> {
    pub profile: &'a str,
    pub endpoint: &'a str,
    pub mode: ExecutionMode,
    pub revision: u64,
    pub provider_model: &'a str,
    pub memory_enabled: bool,
    pub tool_enabled: bool,
    pub daemon_health: DaemonHealthInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorCheckLevel {
    Pass,
    Warn,
    Info,
}

impl DoctorCheckLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            DoctorCheckLevel::Pass => "pass",
            DoctorCheckLevel::Warn => "warn",
            DoctorCheckLevel::Info => "info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    pub name: &'static str,
    pub level: DoctorCheckLevel,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub ok: bool,
    pub profile: String,
    pub endpoint: String,
    pub mode: ExecutionMode,
    pub revision: u64,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonHealthSnapshot {
    pub tick: u64,
    pub state: String,
    pub state_detail: String,
    pub reason: String,
    pub in_flight: String,
    pub in_flight_attempt: u32,
    pub queue_depth: u64,
    pub completed: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonHealthParseError {
    MissingField(&'static str),
    InvalidNumber { field: &'static str, value: String },
}

impl DaemonHealthParseError {
    pub fn as_detail(&self) -> String {
        match self {
            DaemonHealthParseError::MissingField(field) => format!("missing_field:{field}"),
            DaemonHealthParseError::InvalidNumber { field, value } => {
                format!("invalid_number:{field}:{value}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonHealthReadErrorKind {
    NotFound,
    PermissionDenied,
    InvalidData,
    Other,
}

impl DaemonHealthReadErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DaemonHealthReadErrorKind::NotFound => "not_found",
            DaemonHealthReadErrorKind::PermissionDenied => "permission_denied",
            DaemonHealthReadErrorKind::InvalidData => "invalid_data",
            DaemonHealthReadErrorKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonHealthInput {
    MissingEnv,
    InvalidEnvValue {
        reason: &'static str,
    },
    ReadError {
        path: String,
        kind: DaemonHealthReadErrorKind,
    },
    ParseError {
        path: String,
        error: DaemonHealthParseError,
    },
    Snapshot {
        path: String,
        snapshot: DaemonHealthSnapshot,
    },
}

pub fn parse_daemon_health(contents: &str) -> Result<DaemonHealthSnapshot, DaemonHealthParseError> {
    let mut tick: Option<u64> = None;
    let mut state: Option<String> = None;
    let mut state_detail: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut in_flight: Option<String> = None;
    let mut in_flight_attempt: Option<u32> = None;
    let mut queue_depth: Option<u64> = None;
    let mut completed: Option<u64> = None;
    let mut failed: Option<u64> = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        match key {
            "tick" => tick = Some(parse_number_field("tick", value)?),
            "state" => state = Some(value.to_owned()),
            "state_detail" => state_detail = Some(value.to_owned()),
            "reason" => reason = Some(value.to_owned()),
            "in_flight" => in_flight = Some(value.to_owned()),
            "in_flight_attempt" => {
                in_flight_attempt = Some(parse_number_field("in_flight_attempt", value)?);
            }
            "queue_depth" => queue_depth = Some(parse_number_field("queue_depth", value)?),
            "completed" => completed = Some(parse_number_field("completed", value)?),
            "failed" => failed = Some(parse_number_field("failed", value)?),
            _ => {}
        }
    }

    Ok(DaemonHealthSnapshot {
        tick: required_field(tick, "tick")?,
        state: required_field(state, "state")?,
        state_detail: required_field(state_detail, "state_detail")?,
        reason: required_field(reason, "reason")?,
        in_flight: required_field(in_flight, "in_flight")?,
        in_flight_attempt: required_field(in_flight_attempt, "in_flight_attempt")?,
        queue_depth: required_field(queue_depth, "queue_depth")?,
        completed: required_field(completed, "completed")?,
        failed: required_field(failed, "failed")?,
    })
}

pub fn build_doctor_report(input: DoctorInput<'_>) -> DoctorReport {
    let mut checks = Vec::with_capacity(6);
    let mut ok = true;

    let endpoint_level = if has_http_scheme(input.endpoint) {
        DoctorCheckLevel::Pass
    } else {
        ok = false;
        DoctorCheckLevel::Warn
    };
    checks.push(DoctorCheck {
        name: "endpoint_scheme",
        level: endpoint_level,
        detail: format!("endpoint={}", input.endpoint),
    });

    let mode_level = if input.mode == ExecutionMode::Halted {
        ok = false;
        DoctorCheckLevel::Warn
    } else {
        DoctorCheckLevel::Pass
    };
    checks.push(DoctorCheck {
        name: "runtime_mode",
        level: mode_level,
        detail: format!("mode={}", mode_name(input.mode)),
    });

    let provider_level = if input.provider_model.trim().is_empty() {
        ok = false;
        DoctorCheckLevel::Warn
    } else {
        DoctorCheckLevel::Pass
    };
    checks.push(DoctorCheck {
        name: "provider_model",
        level: provider_level,
        detail: format!("provider_model={}", input.provider_model),
    });

    checks.push(DoctorCheck {
        name: "memory_adapter",
        level: DoctorCheckLevel::Info,
        detail: format!("enabled={}", input.memory_enabled),
    });
    checks.push(DoctorCheck {
        name: "tool_adapter",
        level: DoctorCheckLevel::Info,
        detail: format!("enabled={}", input.tool_enabled),
    });

    checks.push(build_daemon_health_check(input.daemon_health, &mut ok));

    DoctorReport {
        ok,
        profile: input.profile.to_owned(),
        endpoint: input.endpoint.to_owned(),
        mode: input.mode,
        revision: input.revision,
        checks,
    }
}

fn has_http_scheme(endpoint: &str) -> bool {
    endpoint.starts_with("http://") || endpoint.starts_with("https://")
}

fn build_daemon_health_check(daemon_health: DaemonHealthInput, ok: &mut bool) -> DoctorCheck {
    match daemon_health {
        DaemonHealthInput::MissingEnv => DoctorCheck {
            name: "daemon_health",
            level: DoctorCheckLevel::Info,
            detail: format!("status=missing env={DAEMON_HEALTH_ENV}"),
        },
        DaemonHealthInput::InvalidEnvValue { reason } => {
            *ok = false;
            DoctorCheck {
                name: "daemon_health",
                level: DoctorCheckLevel::Warn,
                detail: format!("status=invalid_env env={DAEMON_HEALTH_ENV} reason={reason}"),
            }
        }
        DaemonHealthInput::ReadError { path, kind } => {
            *ok = false;
            DoctorCheck {
                name: "daemon_health",
                level: DoctorCheckLevel::Warn,
                detail: format!(
                    "status=read_error env={DAEMON_HEALTH_ENV} path={path} kind={}",
                    kind.as_str()
                ),
            }
        }
        DaemonHealthInput::ParseError { path, error } => {
            *ok = false;
            DoctorCheck {
                name: "daemon_health",
                level: DoctorCheckLevel::Warn,
                detail: format!(
                    "status=parse_error env={DAEMON_HEALTH_ENV} path={path} error={}",
                    error.as_detail()
                ),
            }
        }
        DaemonHealthInput::Snapshot { path, snapshot } => DoctorCheck {
            name: "daemon_health",
            level: DoctorCheckLevel::Pass,
            detail: format!(
                "status=ok env={DAEMON_HEALTH_ENV} path={path} tick={} state={} state_detail={} reason={} in_flight={} in_flight_attempt={} queue_depth={} completed={} failed={}",
                snapshot.tick,
                snapshot.state,
                snapshot.state_detail,
                snapshot.reason,
                snapshot.in_flight,
                snapshot.in_flight_attempt,
                snapshot.queue_depth,
                snapshot.completed,
                snapshot.failed
            ),
        },
    }
}

fn parse_number_field<T>(field: &'static str, value: &str) -> Result<T, DaemonHealthParseError>
where
    T: std::str::FromStr,
{
    value
        .trim()
        .parse::<T>()
        .map_err(|_| DaemonHealthParseError::InvalidNumber {
            field,
            value: value.to_owned(),
        })
}

fn required_field<T>(value: Option<T>, field: &'static str) -> Result<T, DaemonHealthParseError> {
    value.ok_or(DaemonHealthParseError::MissingField(field))
}

#[cfg(test)]
mod tests {
    use super::{
        DaemonHealthInput, DaemonHealthParseError, DaemonHealthSnapshot, DoctorCheckLevel,
        DoctorInput, build_doctor_report, parse_daemon_health,
    };
    use axiom_core::ExecutionMode;

    fn sample_snapshot() -> DaemonHealthSnapshot {
        DaemonHealthSnapshot {
            tick: 4,
            state: String::from("running"),
            state_detail: String::from("item=job-1 attempt=2"),
            reason: String::from("-"),
            in_flight: String::from("job-1"),
            in_flight_attempt: 2,
            queue_depth: 3,
            completed: 5,
            failed: 1,
        }
    }

    #[test]
    fn doctor_report_is_ok_for_default_runtime_shape() {
        let report = build_doctor_report(DoctorInput {
            profile: "prod",
            endpoint: "http://127.0.0.1:8080",
            mode: ExecutionMode::Active,
            revision: 7,
            provider_model: "mock-local",
            memory_enabled: false,
            tool_enabled: false,
            daemon_health: DaemonHealthInput::Snapshot {
                path: String::from("/tmp/daemon.health"),
                snapshot: sample_snapshot(),
            },
        });

        assert!(report.ok);
        assert_eq!(report.checks.len(), 6);
        assert_eq!(report.checks[0].name, "endpoint_scheme");
        assert_eq!(report.checks[0].level, DoctorCheckLevel::Pass);
        assert_eq!(report.checks[1].name, "runtime_mode");
        assert_eq!(report.checks[1].level, DoctorCheckLevel::Pass);
        assert_eq!(report.checks[2].name, "provider_model");
        assert_eq!(report.checks[2].level, DoctorCheckLevel::Pass);
        assert_eq!(report.checks[3].level, DoctorCheckLevel::Info);
        assert_eq!(report.checks[4].level, DoctorCheckLevel::Info);
        assert_eq!(report.checks[5].name, "daemon_health");
        assert_eq!(report.checks[5].level, DoctorCheckLevel::Pass);
    }

    #[test]
    fn doctor_report_warns_for_invalid_endpoint_and_halted_mode() {
        let report = build_doctor_report(DoctorInput {
            profile: "prod",
            endpoint: "127.0.0.1:8080",
            mode: ExecutionMode::Halted,
            revision: 9,
            provider_model: "",
            memory_enabled: true,
            tool_enabled: true,
            daemon_health: DaemonHealthInput::MissingEnv,
        });

        assert!(!report.ok);
        assert_eq!(report.checks[0].level, DoctorCheckLevel::Warn);
        assert_eq!(report.checks[1].level, DoctorCheckLevel::Warn);
        assert_eq!(report.checks[2].level, DoctorCheckLevel::Warn);
        assert_eq!(report.checks[5].level, DoctorCheckLevel::Info);
    }

    #[test]
    fn parse_daemon_health_reads_all_expected_fields() {
        let parsed = parse_daemon_health(
            "tick=7\nstate=running\nstate_detail=item=job-1 attempt=2\nreason=-\nin_flight=job-1\nin_flight_attempt=2\nqueue_depth=4\ncompleted=9\nfailed=1\n",
        )
        .expect("daemon health should parse");

        assert_eq!(parsed.tick, 7);
        assert_eq!(parsed.state, "running");
        assert_eq!(parsed.state_detail, "item=job-1 attempt=2");
        assert_eq!(parsed.reason, "-");
        assert_eq!(parsed.in_flight, "job-1");
        assert_eq!(parsed.in_flight_attempt, 2);
        assert_eq!(parsed.queue_depth, 4);
        assert_eq!(parsed.completed, 9);
        assert_eq!(parsed.failed, 1);
    }

    #[test]
    fn parse_daemon_health_requires_all_fields() {
        let err = parse_daemon_health(
            "tick=1\nstate=idle\nstate_detail=-\nreason=-\nin_flight=-\nin_flight_attempt=0\nqueue_depth=0\ncompleted=0\n",
        )
        .expect_err("missing failed field should error");

        assert_eq!(err, DaemonHealthParseError::MissingField("failed"));
    }

    #[test]
    fn doctor_report_warns_for_daemon_health_parse_error() {
        let report = build_doctor_report(DoctorInput {
            profile: "prod",
            endpoint: "http://127.0.0.1:8080",
            mode: ExecutionMode::Active,
            revision: 3,
            provider_model: "mock-local",
            memory_enabled: true,
            tool_enabled: true,
            daemon_health: DaemonHealthInput::ParseError {
                path: String::from("/tmp/daemon.health"),
                error: DaemonHealthParseError::MissingField("failed"),
            },
        });

        assert!(!report.ok);
        assert_eq!(report.checks[5].name, "daemon_health");
        assert_eq!(report.checks[5].level, DoctorCheckLevel::Warn);
        assert!(report.checks[5].detail.contains("status=parse_error"));
    }
}
