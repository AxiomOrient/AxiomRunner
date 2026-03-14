use crate::state_store::PendingRunSnapshot;
use crate::async_runtime_host::global_async_runtime_host_status;
use crate::config_loader::AppConfig;
use crate::display::mode_name;
use crate::runtime_compose::{RuntimeComposeConfig, RuntimeComposeHealth};
use crate::trace_store::TraceStore;
use axonrunner_adapters::provider_registry;
use axonrunner_core::AgentState;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub profile: String,
    pub provider_id: String,
    pub provider_model: String,
    pub provider_experimental: bool,
    pub state: DoctorState,
    pub runtime: DoctorRuntime,
    pub paths: DoctorPaths,
    pub pending_run: Option<DoctorPendingRun>,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorState {
    pub revision: u64,
    pub mode: String,
    pub facts: usize,
    pub denied: u64,
    pub audit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorRuntime {
    pub provider_state: String,
    pub provider_detail: String,
    pub memory_state: String,
    pub memory_detail: String,
    pub tool_state: String,
    pub tool_detail: String,
    pub async_host_detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorPaths {
    pub workspace: String,
    pub state_path: String,
    pub trace_events_path: String,
    pub tool_log_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorPendingRun {
    pub run_id: String,
    pub intent_id: String,
    pub goal_file_path: String,
    pub phase: String,
    pub reason: String,
    pub approval_state: String,
    pub verifier_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorCheck {
    pub name: String,
    pub state: String,
    pub detail: String,
}

pub fn build_doctor_report(
    config: &AppConfig,
    state: &AgentState,
    compose: &RuntimeComposeHealth,
    trace_store: &TraceStore,
    state_path: &Path,
    pending_run: Option<&PendingRunSnapshot>,
) -> DoctorReport {
    let compose_config = RuntimeComposeConfig::from_app_config(config);
    let async_host = global_async_runtime_host_status();
    let workspace = compose_config.tool_workspace.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf())
    });
    let workspace_display = workspace.display().to_string();
    let state_path_display = state_path.display().to_string();
    let trace_path = trace_store.events_path().display().to_string();

    let checks = vec![
        directory_check("workspace_dir", &workspace),
        parent_directory_check("state_parent_dir", state_path),
        parent_directory_check("trace_parent_dir", trace_store.events_path()),
        file_presence_check("state_snapshot", state_path),
        trace_log_check(trace_store),
        provider_check(compose),
    ];

    let ok = checks.iter().all(|check| check.state == "ok");

    DoctorReport {
        ok,
        profile: config.profile.clone(),
        provider_id: compose.provider_id.clone(),
        provider_model: compose.provider_model.clone(),
        provider_experimental: provider_registry()
            .iter()
            .find(|entry| entry.id == compose.provider_id)
            .map(|entry| entry.experimental)
            .unwrap_or(false),
        state: DoctorState {
            revision: state.revision,
            mode: mode_name(state.mode).to_owned(),
            facts: state.facts.len(),
            denied: state.denied_count,
            audit: state.audit_count,
        },
        runtime: DoctorRuntime {
            provider_state: compose.provider.state.to_owned(),
            provider_detail: compose.provider.detail.clone(),
            memory_state: compose.memory.state.to_owned(),
            memory_detail: compose.memory.detail.clone(),
            tool_state: compose.tool.state.to_owned(),
            tool_detail: compose.tool.detail.clone(),
            async_host_detail: format_async_host_detail(&async_host),
        },
        paths: DoctorPaths {
            workspace: workspace_display,
            state_path: state_path_display,
            trace_events_path: trace_path,
            tool_log_path: compose_config.tool_log_path,
        },
        pending_run: pending_run.map(|pending| DoctorPendingRun {
            run_id: pending.run_id.clone(),
            intent_id: pending.intent_id.clone(),
            goal_file_path: pending.goal_file_path.clone(),
            phase: pending.phase.clone(),
            reason: pending.reason.clone(),
            approval_state: pending.approval_state.clone(),
            verifier_state: pending.verifier_state.clone(),
        }),
        checks,
    }
}

pub fn render_doctor_lines(report: &DoctorReport) -> Vec<String> {
    let mut lines = vec![
        format!(
            "doctor ok={} profile={} provider_id={} provider_model={} provider_experimental={}",
            report.ok,
            report.profile,
            report.provider_id,
            report.provider_model,
            report.provider_experimental
        ),
        format!(
            "doctor state revision={} mode={} facts={} denied={} audit={}",
            report.state.revision,
            report.state.mode,
            report.state.facts,
            report.state.denied,
            report.state.audit
        ),
        format!(
            "doctor runtime provider_state={} memory_state={} tool_state={}",
            report.runtime.provider_state, report.runtime.memory_state, report.runtime.tool_state
        ),
        format!(
            "doctor detail provider_detail={} memory_detail={} tool_detail={}",
            report.runtime.provider_detail,
            report.runtime.memory_detail,
            report.runtime.tool_detail
        ),
        format!(
            "doctor async_host detail={}",
            report.runtime.async_host_detail
        ),
        format!(
            "doctor paths workspace={} state_path={} trace_events_path={} tool_log_path={}",
            report.paths.workspace,
            report.paths.state_path,
            report.paths.trace_events_path,
            report.paths.tool_log_path
        ),
    ];

    if let Some(pending) = &report.pending_run {
        lines.push(format!(
            "doctor pending_run run_id={} intent_id={} goal_file_path={} phase={} reason={} approval_state={} verifier_state={}",
            pending.run_id,
            pending.intent_id,
            pending.goal_file_path,
            pending.phase,
            pending.reason,
            pending.approval_state,
            pending.verifier_state
        ));
    }

    for check in &report.checks {
        lines.push(format!(
            "doctor check name={} state={} detail={}",
            check.name, check.state, check.detail
        ));
    }

    lines
}

fn format_async_host_detail(
    async_host: &crate::async_runtime_host::AsyncRuntimeHostStatus,
) -> String {
    if let Some(error) = async_host.init_error.as_deref() {
        format!("init_mode={},error={error}", async_host.init_mode)
    } else {
        format!(
            "init_mode={},worker_threads={},max_in_flight={},timeout_ms={}",
            async_host.init_mode,
            async_host.worker_threads,
            async_host.max_in_flight,
            async_host
                .timeout_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| String::from("none"))
        )
    }
}

fn directory_check(name: &str, path: &Path) -> DoctorCheck {
    if path.is_dir() {
        return DoctorCheck {
            name: name.to_owned(),
            state: String::from("ok"),
            detail: format!("exists,path={}", path.display()),
        };
    }

    DoctorCheck {
        name: name.to_owned(),
        state: String::from("fail"),
        detail: format!("missing,path={}", path.display()),
    }
}

fn parent_directory_check(name: &str, path: &Path) -> DoctorCheck {
    let Some(parent) = path.parent() else {
        return DoctorCheck {
            name: name.to_owned(),
            state: String::from("warn"),
            detail: String::from("parent=none"),
        };
    };

    if parent.is_dir() {
        return DoctorCheck {
            name: name.to_owned(),
            state: String::from("ok"),
            detail: format!("exists,path={}", parent.display()),
        };
    }

    DoctorCheck {
        name: name.to_owned(),
        state: String::from("warn"),
        detail: format!("missing,path={}", parent.display()),
    }
}

fn file_presence_check(name: &str, path: &Path) -> DoctorCheck {
    let state = if path.is_file() { "ok" } else { "warn" };
    let detail = if path.is_file() {
        format!("present,path={}", path.display())
    } else {
        format!("missing,path={}", path.display())
    };
    DoctorCheck {
        name: name.to_owned(),
        state: state.to_owned(),
        detail,
    }
}

fn trace_log_check(trace_store: &TraceStore) -> DoctorCheck {
    match trace_store.load_events() {
        Ok(events) => DoctorCheck {
            name: String::from("trace_log"),
            state: String::from("ok"),
            detail: format!(
                "events={},path={}",
                events.len(),
                trace_store.events_path().display()
            ),
        },
        Err(error) => DoctorCheck {
            name: String::from("trace_log"),
            state: String::from("fail"),
            detail: error,
        },
    }
}

fn provider_check(compose: &RuntimeComposeHealth) -> DoctorCheck {
    let state = match compose.provider.state {
        "ready" => "ok",
        "degraded" => "warn",
        _ => "fail",
    };

    DoctorCheck {
        name: String::from("provider_probe"),
        state: state.to_owned(),
        detail: compose.provider.detail.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::format_async_host_detail;
    use crate::async_runtime_host::AsyncRuntimeHostStatus;

    #[test]
    fn doctor_formats_configured_async_host_detail() {
        let detail = format_async_host_detail(&AsyncRuntimeHostStatus {
            init_mode: "configured",
            worker_threads: 2,
            max_in_flight: 8,
            timeout_ms: None,
            init_error: None,
        });

        assert_eq!(
            detail,
            "init_mode=configured,worker_threads=2,max_in_flight=8,timeout_ms=none"
        );
    }

    #[test]
    fn doctor_formats_failed_async_host_detail() {
        let detail = format_async_host_detail(&AsyncRuntimeHostStatus {
            init_mode: "failed",
            worker_threads: 0,
            max_in_flight: 0,
            timeout_ms: None,
            init_error: Some(String::from("async runtime host init failed: boom")),
        });

        assert_eq!(
            detail,
            "init_mode=failed,error=async runtime host init failed: boom"
        );
    }
}
