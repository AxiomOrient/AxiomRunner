use crate::channel::{ChannelAction, ChannelResult, execute_channel_action};
use crate::cli_command::{CliCommand, IntentTemplate, ServeMode};
use crate::config_loader::AppConfig;
use crate::display::mode_name;
use crate::doctor::{
    DAEMON_HEALTH_ENV, DaemonHealthInput, DaemonHealthReadErrorKind, DoctorContextInput,
    DoctorInput, DoctorRuntimeInput, build_doctor_report, parse_daemon_health,
};
use crate::runtime_compose::{RuntimeComposeConfig, RuntimeComposeState};
use crate::status::{
    ChannelStatusInput, RuntimeStatusInput, StateStatusInput, StatusInput, StatusSnapshot,
    render_status_lines,
};
use axiom_adapters::contracts::ContextAdapter;
use axiom_apps::{daemon, gateway};
use axiom_core::{
    AgentState, DecisionOutcome, DomainEvent, Intent, IntentKind, PolicyCode, build_policy_audit,
    decide, evaluate_policy, project_from,
};
use std::fs;

mod actions;

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedIntent {
    intent_id: String,
    kind: &'static str,
    outcome: DecisionOutcome,
    policy_code: PolicyCode,
    effect_count: usize,
}

pub struct CliRuntime {
    state: AgentState,
    actor_id: String,
    next_intent_seq: u64,
    compose_state: RuntimeComposeState,
}

impl CliRuntime {
    pub fn new(actor_id: String, config: &AppConfig) -> Self {
        Self::new_with_compose(
            actor_id,
            RuntimeComposeConfig::from_env(config.provider.as_str()),
        )
    }

    fn new_with_compose(actor_id: String, compose_config: RuntimeComposeConfig) -> Self {
        Self {
            state: AgentState::default(),
            actor_id,
            next_intent_seq: 0,
            compose_state: RuntimeComposeState::new(compose_config),
        }
    }

    pub fn context(&self) -> Option<&dyn ContextAdapter> {
        self.compose_state.context()
    }

    fn reset(&mut self) {
        self.state = AgentState::default();
        self.next_intent_seq = 0;
        if let Err(error) = self.compose_state.clear() {
            eprintln!("runtime_compose clear failed error={error}");
        }
    }

    fn apply_template(&mut self, template: &IntentTemplate) -> AppliedIntent {
        let intent = template.to_intent(self.next_intent_id(), Some(self.actor_id.clone()));
        self.apply_intent(intent)
    }

    fn apply_intent(&mut self, intent: Intent) -> AppliedIntent {
        let verdict = evaluate_policy(&self.state, &intent);
        let decision = decide(&intent, &verdict);
        let audit = build_policy_audit(&self.state, &intent, &verdict);
        let effects = decision.effects.clone();

        let events = vec![
            DomainEvent::IntentAccepted {
                intent: intent.clone(),
            },
            DomainEvent::PolicyEvaluated { audit },
            DomainEvent::DecisionCalculated { decision },
            DomainEvent::EffectsApplied { effects },
        ];

        self.state = project_from(&self.state, &events);

        let decision = match &events[2] {
            DomainEvent::DecisionCalculated { decision } => decision,
            _ => unreachable!("event order is fixed"),
        };

        let audit = match &events[1] {
            DomainEvent::PolicyEvaluated { audit } => audit,
            _ => unreachable!("event order is fixed"),
        };

        let effects = match &events[3] {
            DomainEvent::EffectsApplied { effects } => effects,
            _ => unreachable!("event order is fixed"),
        };

        AppliedIntent {
            intent_id: intent.intent_id,
            kind: intent_kind_name(&intent.kind),
            outcome: decision.outcome,
            policy_code: audit.code,
            effect_count: effects.len(),
        }
    }

    fn persist_template_result(&mut self, template: &IntentTemplate, applied: &AppliedIntent) {
        let execution =
            self.compose_state
                .apply_template(template, &applied.intent_id, applied.outcome);
        if let Some((stage, message)) = execution.first_failure() {
            eprintln!(
                "runtime_compose failed intent_id={} stage={} error={}",
                applied.intent_id, stage, message
            );
        }
    }

    fn next_intent_id(&mut self) -> String {
        self.next_intent_seq = self.next_intent_seq.saturating_add(1);
        format!("cli-{}", self.next_intent_seq)
    }
}

pub fn execute_command(
    runtime: &mut CliRuntime,
    config: &AppConfig,
    command: CliCommand,
) -> Result<(), String> {
    match command {
        CliCommand::Onboard { action } => {
            actions::execute_onboard(&config.profile, action)?;
        }
        CliCommand::Agent { action } => {
            actions::execute_agent(action, runtime.context())?;
        }
        CliCommand::Run(intent) => {
            execute_intent(runtime, &intent);
        }
        CliCommand::Batch {
            intents,
            reset_state,
        } => {
            if reset_state {
                runtime.reset();
            }
            for intent in &intents {
                execute_intent(runtime, intent);
            }
            print_summary("batch", intents.len(), &runtime.state);
        }
        CliCommand::Status => {
            print_status(runtime, config);
        }
        CliCommand::Health => {
            print_health(&runtime.state, config);
        }
        CliCommand::Doctor => {
            execute_doctor(runtime, config);
        }
        CliCommand::Cron { action } => {
            actions::execute_cron(action)?;
        }
        CliCommand::Service { action } => {
            actions::execute_service(action)?;
        }
        CliCommand::Channel { action } => {
            actions::execute_channel(action)?;
        }
        CliCommand::Integrations { action } => {
            actions::execute_integrations(action)?;
        }
        CliCommand::Skills { action } => {
            actions::execute_skills(action)?;
        }

        CliCommand::Serve { mode } => match mode {
            ServeMode::Gateway => gateway::run(&config.profile, &config.endpoint),
            ServeMode::Daemon => daemon::run(&config.profile, &config.endpoint),
        },
    }

    Ok(())
}

fn execute_intent(runtime: &mut CliRuntime, intent: &IntentTemplate) {
    // Short-circuit ReadFact: reading is a pure query with no side-effects.
    // Running it through the full pipeline would emit 4 DomainEvents and
    // increment `revision` on every read. Answer directly from current state.
    //
    // Safety invariant: persist_template_result() is a no-op for ReadFact —
    // it only writes to the memory adapter for WriteFact/RemoveFact/Halt.
    // Skipping apply_template() + persist_template_result() is safe and does
    // not change observable storage state.
    if let IntentTemplate::Read { key } = intent {
        print_read_value(&runtime.state, key);
        return;
    }

    let applied = runtime.apply_template(intent);
    runtime.persist_template_result(intent, &applied);
    print_intent_result(&applied);
}

fn execute_doctor(runtime: &CliRuntime, config: &AppConfig) {
    let compose = runtime.compose_state.health();
    let report = build_doctor_report(DoctorInput {
        context: DoctorContextInput {
            profile: &config.profile,
            endpoint: &config.endpoint,
        },
        runtime: DoctorRuntimeInput {
            mode: runtime.state.mode,
            revision: runtime.state.revision,
            provider_model: &compose.provider_model,
            memory_enabled: compose.memory.enabled,
            tool_enabled: compose.tool.enabled,
        },
        daemon_health: daemon_health_from_config(config),
    });

    println!(
        "doctor ok={} profile={} endpoint={} mode={} revision={} checks={}",
        report.ok,
        report.profile,
        report.endpoint,
        mode_name(report.mode),
        report.revision,
        report.checks.len()
    );

    for check in report.checks {
        println!(
            "doctor check={} level={} detail={}",
            check.name,
            check.level.as_str(),
            check.detail
        );
    }
}

fn daemon_health_from_config(config: &AppConfig) -> DaemonHealthInput {
    let path = match std::env::var(DAEMON_HEALTH_ENV) {
        Ok(path) => {
            if path.trim().is_empty() {
                return DaemonHealthInput::InvalidEnvValue {
                    reason: "empty_path",
                };
            }
            std::path::PathBuf::from(path)
        }
        Err(std::env::VarError::NotPresent) => {
            match daemon::resolve_health_path_from_state(&config.profile, &config.endpoint) {
                Some(path) => path,
                None => return DaemonHealthInput::MissingEnv,
            }
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            return DaemonHealthInput::InvalidEnvValue {
                reason: "not_unicode",
            };
        }
    };
    let path_display = path.display().to_string();

    match fs::read_to_string(&path) {
        Ok(contents) => match parse_daemon_health(&contents) {
            Ok(snapshot) => DaemonHealthInput::Snapshot {
                path: path_display,
                snapshot,
            },
            Err(error) => DaemonHealthInput::ParseError {
                path: path_display,
                error,
            },
        },
        Err(error) => DaemonHealthInput::ReadError {
            path: path_display,
            kind: map_read_error_kind(&error),
        },
    }
}

fn map_read_error_kind(error: &std::io::Error) -> DaemonHealthReadErrorKind {
    match error.kind() {
        std::io::ErrorKind::NotFound => DaemonHealthReadErrorKind::NotFound,
        std::io::ErrorKind::PermissionDenied => DaemonHealthReadErrorKind::PermissionDenied,
        std::io::ErrorKind::InvalidData => DaemonHealthReadErrorKind::InvalidData,
        _ => DaemonHealthReadErrorKind::Other,
    }
}

fn print_intent_result(applied: &AppliedIntent) {
    println!(
        "intent id={} kind={} outcome={} policy={} effects={}",
        applied.intent_id,
        applied.kind,
        outcome_name(applied.outcome),
        applied.policy_code.as_str(),
        applied.effect_count
    );
}

fn print_read_value(state: &AgentState, key: &str) {
    let value = state.facts.get(key).map(String::as_str).unwrap_or("<none>");
    println!("read key={} value={}", key, value);
}

fn print_summary(label: &str, intent_count: usize, state: &AgentState) {
    println!(
        "{} completed count={} revision={} mode={} facts={} denied={} audit={}",
        label,
        intent_count,
        state.revision,
        mode_name(state.mode),
        state.facts.len(),
        state.denied_count,
        state.audit_count
    );
}

fn print_status(runtime: &CliRuntime, config: &AppConfig) {
    let compose = runtime.compose_state.health();
    let snapshot = StatusSnapshot::from(StatusInput {
        state: StateStatusInput {
            revision: runtime.state.revision,
            mode: runtime.state.mode,
            facts: runtime.state.facts.len(),
            denied: runtime.state.denied_count,
            audit: runtime.state.audit_count,
        },
        runtime: RuntimeStatusInput {
            provider_id: compose.provider_id,
            provider_model: compose.provider_model,
            memory_enabled: compose.memory.enabled,
            memory_state: compose.memory.state.to_string(),
            tool_enabled: compose.tool.enabled,
            tool_state: compose.tool.state.to_string(),
            bootstrap_state: compose.bootstrap.state.to_string(),
        },
        daemon_health: daemon_health_from_config(config),
        channels: channel_status_from_store(),
    });

    for line in render_status_lines(&snapshot) {
        println!("{line}");
    }
}

fn print_health(state: &AgentState, config: &AppConfig) {
    println!(
        "health ok=true profile={} endpoint={} mode={} revision={}",
        config.profile,
        config.endpoint,
        mode_name(state.mode),
        state.revision
    );
}

fn channel_status_from_store() -> ChannelStatusInput {
    match execute_channel_action(ChannelAction::List) {
        Ok(ChannelResult::Listed {
            channels, running, ..
        }) => ChannelStatusInput::Listed {
            total: channels.len(),
            running,
        },
        Ok(_) => ChannelStatusInput::Error {
            detail: String::from("unexpected_channel_result"),
        },
        Err(error) => ChannelStatusInput::Error { detail: error },
    }
}

fn intent_kind_name(kind: &IntentKind) -> &'static str {
    match kind {
        IntentKind::ReadFact { .. } => "read",
        IntentKind::WriteFact { .. } => "write",
        IntentKind::RemoveFact { .. } => "remove",
        IntentKind::FreezeWrites => "freeze",
        IntentKind::Halt => "halt",
    }
}

fn outcome_name(outcome: DecisionOutcome) -> &'static str {
    match outcome {
        DecisionOutcome::Accepted => "accepted",
        DecisionOutcome::Rejected => "rejected",
    }
}

#[cfg(test)]
mod tests {
    use super::{CliRuntime, RuntimeComposeConfig, execute_command};
    use crate::cli_command::{CliCommand, IntentTemplate};
    use crate::config_loader::AppConfig;
    use axiom_adapters::{MemoryAdapter, memory::MarkdownMemoryAdapter};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_path(label: &str, extension: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiom-cli-runtime-{label}-{}-{tick}.{extension}",
            std::process::id()
        ))
    }

    fn compose_config(
        memory_path: Option<PathBuf>,
        tool_workspace: Option<PathBuf>,
    ) -> RuntimeComposeConfig {
        RuntimeComposeConfig {
            memory_path,
            tool_workspace,
            tool_log_path: String::from("runtime.log"),
            provider_id: String::from("mock-local"),
            provider_model: String::from("mock-local"),
            max_tokens: 4096,
            bootstrap_root: None,
            channel_id: None,
            tool_ids: vec![axiom_adapters::DEFAULT_TOOL_ID.to_owned()],
            context_root: None,
        }
    }

    #[test]
    fn cli_runtime_persists_write_when_memory_enabled() {
        let path = unique_path("persist-write", "md");
        let mut runtime = CliRuntime::new_with_compose(
            String::from("system"),
            compose_config(Some(path.clone()), None),
        );

        let config = AppConfig::default();
        execute_command(
            &mut runtime,
            &config,
            CliCommand::Run(IntentTemplate::Write {
                key: String::from("alpha"),
                value: String::from("42"),
            }),
        )
        .expect("command should execute");

        let reader = MarkdownMemoryAdapter::new(path.clone()).expect("memory file should load");
        let record = reader
            .get("alpha")
            .expect("memory read should succeed")
            .expect("alpha should be persisted");
        assert_eq!(record.value, "42");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn cli_runtime_reset_clears_persisted_memory() {
        let path = unique_path("persist-reset", "md");
        let mut runtime = CliRuntime::new_with_compose(
            String::from("system"),
            compose_config(Some(path.clone()), None),
        );

        let config = AppConfig::default();
        execute_command(
            &mut runtime,
            &config,
            CliCommand::Run(IntentTemplate::Write {
                key: String::from("alpha"),
                value: String::from("42"),
            }),
        )
        .expect("write command should execute");

        execute_command(
            &mut runtime,
            &config,
            CliCommand::Batch {
                intents: vec![IntentTemplate::Read {
                    key: String::from("alpha"),
                }],
                reset_state: true,
            },
        )
        .expect("reset batch should execute");

        let reader = MarkdownMemoryAdapter::new(path.clone()).expect("memory file should load");
        let record = reader.get("alpha").expect("memory read should succeed");
        assert!(record.is_none());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn cli_runtime_compose_executes_provider_memory_and_tool() {
        let memory_path = unique_path("compose-memory", "md");
        let workspace = unique_path("compose-workspace", "dir");
        let mut runtime = CliRuntime::new_with_compose(
            String::from("system"),
            compose_config(Some(memory_path.clone()), Some(workspace.clone())),
        );

        let config = AppConfig::default();
        execute_command(
            &mut runtime,
            &config,
            CliCommand::Run(IntentTemplate::Write {
                key: String::from("alpha"),
                value: String::from("42"),
            }),
        )
        .expect("write command should execute");

        let memory_reader =
            MarkdownMemoryAdapter::new(memory_path.clone()).expect("memory file should load");
        let memory_record = memory_reader
            .get("alpha")
            .expect("memory read should succeed")
            .expect("alpha should be persisted");
        assert_eq!(memory_record.value, "42");

        let log_path = workspace.join("runtime.log");
        let log = fs::read_to_string(&log_path).expect("tool log should exist");
        assert!(log.contains("intent=cli-1 kind=write key=alpha"));
        assert!(log.contains("provider=intent=cli-1 kind=write key=alpha value=42"));

        let _ = fs::remove_file(memory_path);
        let _ = fs::remove_file(log_path);
        let _ = fs::remove_dir_all(workspace);
    }

    // --- OTP gate integration tests ---
    //
    // std::env::set_var / remove_var are unsafe in Rust 2024 and cannot be used
    // under #![forbid(unsafe_code)].  These tests verify the OtpGate contract
    // directly via OtpGate::new(), which exercises the same code paths that
    // execute_agent() calls when the gate is active.

    #[test]
    fn otp_gate_new_valid_secret_constructs_ok() {
        // Mirrors the Some(Ok(gate)) branch in execute_agent: a well-formed
        // secret must produce a usable gate.
        const SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
        let result = crate::otp_gate::OtpGate::new(SECRET);
        assert!(result.is_ok(), "valid base32 secret must build a gate");
    }

    #[test]
    fn otp_gate_wrong_code_is_rejected() {
        // Mirrors the gate.verify(&provided) == false branch in execute_agent,
        // which causes an OTP verification failure error to be returned.
        const SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
        let gate = crate::otp_gate::OtpGate::new(SECRET).expect("valid secret");
        // "000000" is statistically never a valid TOTP window token.
        assert!(!gate.verify("000000"), "wrong code must be rejected");
    }

    #[test]
    fn otp_gate_empty_code_is_rejected() {
        // Mirrors the case where AXIOM_OTP_CODE is absent:
        // unwrap_or_default() yields "" and verify must reject it.
        const SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
        let gate = crate::otp_gate::OtpGate::new(SECRET).expect("valid secret");
        assert!(!gate.verify(""), "empty code must be rejected");
    }

    #[test]
    fn otp_gate_invalid_secret_returns_err() {
        // Mirrors the Some(Err(e)) branch in execute_agent, which surfaces as
        // "OTP gate config error: …" when the env var contains a bad value.
        let result = crate::otp_gate::OtpGate::new("!!!invalid!!!");
        assert!(result.is_err(), "malformed secret must return Err");
    }

    #[test]
    fn read_intent_does_not_increment_revision() {
        let mut runtime =
            CliRuntime::new_with_compose(String::from("system"), compose_config(None, None));
        let config = AppConfig::default();
        execute_command(
            &mut runtime,
            &config,
            CliCommand::Run(IntentTemplate::Write {
                key: String::from("x"),
                value: String::from("1"),
            }),
        )
        .expect("write should succeed");

        let revision_after_write = runtime.state.revision;

        execute_command(
            &mut runtime,
            &config,
            CliCommand::Run(IntentTemplate::Read {
                key: String::from("x"),
            }),
        )
        .expect("read should succeed");

        assert_eq!(
            runtime.state.revision, revision_after_write,
            "ReadFact must not increment revision"
        );
    }

    #[test]
    fn read_intent_does_not_increment_revision_on_missing_key() {
        let mut runtime =
            CliRuntime::new_with_compose(String::from("system"), compose_config(None, None));
        let config = AppConfig::default();
        let baseline = runtime.state.revision;

        execute_command(
            &mut runtime,
            &config,
            CliCommand::Run(IntentTemplate::Read {
                key: String::from("no_such_key"),
            }),
        )
        .expect("read of missing key should succeed");

        assert_eq!(
            runtime.state.revision, baseline,
            "ReadFact on missing key must not increment revision"
        );
    }
}
