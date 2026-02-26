use crate::agent_loop::{AgentAction, AgentExecutionContext, AgentResultKind, execute_agent_action};
use crate::display::mode_name;
use crate::channel::{ChannelAction, ChannelResult, execute_channel_action};
use crate::cli_command::{
    AgentActionTemplate, ChannelActionTemplate, CliCommand, CronActionTemplate,
    IntegrationsActionTemplate, IntentTemplate, OnboardActionTemplate, ServeMode,
    ServiceActionTemplate, SkillsActionTemplate,
};
use crate::config_loader::AppConfig;
use crate::cron::{CronAction, CronResult, execute_cron_action};
use crate::doctor::{
    DAEMON_HEALTH_ENV, DaemonHealthInput, DaemonHealthReadErrorKind, DoctorInput,
    build_doctor_report, parse_daemon_health,
};
use crate::integrations::{IntegrationsAction, IntegrationsResult, execute_integrations_action};
use crate::migrate_args::parse_args as parse_migrate_args;
use crate::migrate_runner::run_migration;
use crate::migrate_types::MigrationReport;
use crate::onboard::{OnboardAction, OnboardResult, execute_onboard_action};
use crate::runtime_compose::{RuntimeComposeConfig, RuntimeComposeState};
use axiom_adapters::contracts::ContextAdapter;
use crate::service::{ServiceAction, ServiceResult, execute_service_action};
use crate::skills::{SkillsAction, SkillsResult, execute_skills_action};
use crate::status::{ChannelStatusInput, StatusInput, StatusSnapshot, render_status_lines};
use axiom_apps::{daemon, gateway};
use axiom_core::{
    AgentState, DecisionOutcome, DomainEvent, Intent, IntentKind, PolicyCode,
    build_policy_audit, decide, evaluate_policy, project_from,
};
use std::fs;

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
            execute_onboard(&config.profile, action)?;
        }
        CliCommand::Agent { action } => {
            execute_agent(action, runtime.context())?;
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
            execute_cron(action)?;
        }
        CliCommand::Service { action } => {
            execute_service(action)?;
        }
        CliCommand::Channel { action } => {
            execute_channel(action)?;
        }
        CliCommand::Integrations { action } => {
            execute_integrations(action)?;
        }
        CliCommand::Skills { action } => {
            execute_skills(action)?;
        }
        CliCommand::Migrate { raw_args } => {
            execute_migrate(raw_args)?;
        }
        CliCommand::Serve { mode } => match mode {
            ServeMode::Gateway => gateway::run(&config.profile, &config.endpoint),
            ServeMode::Daemon => daemon::run(&config.profile, &config.endpoint),
        },
    }

    Ok(())
}

fn execute_onboard(profile: &str, action: OnboardActionTemplate) -> Result<(), String> {
    let result = execute_onboard_action(OnboardAction::Configure {
        profile: profile.to_string(),
        interactive: action.interactive,
        channels_only: action.channels_only,
        api_key: action.api_key,
        provider: action.provider,
        memory: action.memory,
    })?;

    match result {
        OnboardResult::Configured {
            state_path,
            workspace_path,
            state,
        } => {
            println!(
                "onboard configured profile={} provider={} memory={} api_key_set={} interactive={} channels_only={} workspace_profile={} workspace={} state_path={}",
                state.profile,
                state.provider,
                state.memory.as_str(),
                state.api_key_set,
                state.interactive,
                state.channels_only,
                state.workspace_profile_path.display(),
                workspace_path.display(),
                state_path.display()
            );
        }
    }

    Ok(())
}

fn execute_agent(action: AgentActionTemplate, context: Option<&dyn ContextAdapter>) -> Result<(), String> {
    // OTP gate: if AXIOM_OTP_SECRET is set, require a valid TOTP code via AXIOM_OTP_CODE.
    // If the env var is absent the gate is disabled and execution proceeds unchanged.
    if let Some(gate_result) = crate::otp_gate::OtpGate::load_from_env() {
        let gate = gate_result.map_err(|e| format!("OTP gate config error: {e}"))?;
        let provided = std::env::var("AXIOM_OTP_CODE").unwrap_or_default();
        if !gate.verify(&provided) {
            return Err(
                "OTP verification failed. Set AXIOM_OTP_CODE=<6-digit-code> and retry.".to_string(),
            );
        }
    }

    let agent = axiom_adapters::build_contract_agent("")
        .map_err(|e| format!("agent backend init failed: {e}"))?;

    let result = execute_agent_action(
        AgentAction {
            cwd: action.cwd,
            message: action.message,
            model: action.model,
        },
        AgentExecutionContext {
            agent: agent.as_ref(),
            estop: None,
            context,
        },
    )?;

    match result.kind {
        AgentResultKind::Single { input, output } => {
            let agent_id = &result.base.agent_id;
            let cwd = &result.base.cwd;
            let model = &result.base.model;
            let reason = result.base.reason;
            println!(
                "agent single agent={agent_id} cwd={cwd} model={model} input={input} output={output} reason={}",
                reason.as_str()
            );
        }
        AgentResultKind::Interactive { turns } => {
            let agent_id = &result.base.agent_id;
            let cwd = &result.base.cwd;
            let model = &result.base.model;
            let reason = result.base.reason;
            println!("agent interactive agent={agent_id} cwd={cwd} model={model}");
            let turn_count = turns.len();
            for turn in turns {
                println!(
                    "agent turn index={} input={} output={} stop={}",
                    turn.index, turn.input, turn.output, turn.stop
                );
            }
            println!("agent complete turns={turn_count} reason={}", reason.as_str());
        }
    }

    Ok(())
}

fn execute_intent(runtime: &mut CliRuntime, intent: &IntentTemplate) {
    // Short-circuit ReadFact: reading is a pure query with no side-effects.
    // Running it through the full pipeline would emit 4 DomainEvents and
    // increment `revision` on every read. Answer directly from current state.
    if let IntentTemplate::Read { key } = intent {
        print_read_value(&runtime.state, key);
        return;
    }

    let applied = runtime.apply_template(intent);
    runtime.persist_template_result(intent, &applied);
    print_intent_result(&applied);
}

fn execute_migrate(raw_args: Vec<String>) -> Result<(), String> {
    let mut parse_failed = false;
    let mut report = match parse_migrate_args(raw_args) {
        Ok(args) => run_migration(args),
        Err(message) => {
            parse_failed = true;
            MigrationReport::argument_failure(message)
        }
    };
    let mut json = report.to_json();

    if let Some(report_path) = report.source_paths.report.clone()
        && let Err(error) = fs::write(&report_path, &json)
    {
        report.fatal = true;
        report.errors.push(format!(
            "failed to write report '{}': {error}",
            report_path.display()
        ));
        json = report.to_json();
    }

    println!("{json}");

    if parse_failed {
        return Err(String::from("migrate command argument parsing failed"));
    }

    if report.fatal {
        let detail = report
            .errors
            .first()
            .cloned()
            .unwrap_or_else(|| String::from("unknown migrate error"));
        return Err(format!("migrate command failed: {detail}"));
    }

    Ok(())
}

fn execute_doctor(runtime: &CliRuntime, config: &AppConfig) {
    let compose = runtime.compose_state.health();
    let report = build_doctor_report(DoctorInput {
        profile: &config.profile,
        endpoint: &config.endpoint,
        mode: runtime.state.mode,
        revision: runtime.state.revision,
        provider_model: &compose.provider_model,
        memory_enabled: compose.memory_enabled,
        tool_enabled: compose.tool_enabled,
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

fn execute_cron(action: CronActionTemplate) -> Result<(), String> {
    let action = match action {
        CronActionTemplate::List => CronAction::List,
        CronActionTemplate::Add {
            expression,
            command,
        } => CronAction::Add {
            expression,
            command,
        },
        CronActionTemplate::Remove { id } => CronAction::Remove { id },
    };

    let result = execute_cron_action(action)?;
    match result {
        CronResult::Listed {
            path,
            jobs,
            due_count,
        } => {
            println!(
                "cron list count={} due={} path={}",
                jobs.len(),
                due_count,
                path.display()
            );
            for job in jobs {
                println!(
                    "cron job id={} expr={} next_run={} cmd={}",
                    job.id, job.expression, job.next_run_at, job.command
                );
            }
        }
        CronResult::Added { path, job } => {
            println!(
                "cron added id={} expr={} next_run={} cmd={} path={}",
                job.id,
                job.expression,
                job.next_run_at,
                job.command,
                path.display()
            );
        }
        CronResult::Removed {
            path,
            id,
            remaining,
        } => {
            println!(
                "cron removed id={} remaining={} path={}",
                id,
                remaining,
                path.display()
            );
        }
    }

    Ok(())
}

fn execute_service(action: ServiceActionTemplate) -> Result<(), String> {
    let action = match action {
        ServiceActionTemplate::Install => ServiceAction::Install,
        ServiceActionTemplate::Start => ServiceAction::Start,
        ServiceActionTemplate::Stop => ServiceAction::Stop,
        ServiceActionTemplate::Status => ServiceAction::Status,
        ServiceActionTemplate::Uninstall => ServiceAction::Uninstall,
    };

    let result = execute_service_action(action)?;
    match result {
        ServiceResult::Installed { path, state } => {
            println!(
                "service installed=true running={} installs={} starts={} stops={} path={}",
                state.running,
                state.install_count,
                state.start_count,
                state.stop_count,
                path.display()
            );
        }
        ServiceResult::Started { path, state } => {
            println!(
                "service started=true running={} installs={} starts={} stops={} path={}",
                state.running,
                state.install_count,
                state.start_count,
                state.stop_count,
                path.display()
            );
        }
        ServiceResult::Stopped { path, state } => {
            println!(
                "service stopped=true running={} installs={} starts={} stops={} path={}",
                state.running,
                state.install_count,
                state.start_count,
                state.stop_count,
                path.display()
            );
        }
        ServiceResult::Status { path, state } => {
            println!(
                "service status installed={} running={} installs={} starts={} stops={} path={}",
                state.installed,
                state.running,
                state.install_count,
                state.start_count,
                state.stop_count,
                path.display()
            );
        }
        ServiceResult::Uninstalled { path, removed } => {
            println!(
                "service uninstalled=true removed={} path={}",
                removed,
                path.display()
            );
        }
    }

    Ok(())
}

fn execute_integrations(action: IntegrationsActionTemplate) -> Result<(), String> {
    let action = match action {
        IntegrationsActionTemplate::Info { name } => IntegrationsAction::Info { name },
        IntegrationsActionTemplate::Install { name } => IntegrationsAction::Install { name },
        IntegrationsActionTemplate::Remove { name } => IntegrationsAction::Remove { name },
        IntegrationsActionTemplate::List => IntegrationsAction::List,
    };

    let result = execute_integrations_action(action)?;
    match result {
        IntegrationsResult::Info { entry } => {
            println!(
                "integrations info name={} category={} status={} transport={} summary={}",
                entry.name,
                entry.category.as_str(),
                entry.status.as_str(),
                entry.transport,
                entry.summary
            );
        }
        IntegrationsResult::Installed { name, instructions } => {
            println!("integrations installed name={name}");
            for step in &instructions {
                println!("  step: {step}");
            }
        }
        IntegrationsResult::Removed { name } => {
            println!("integrations removed name={name}");
        }
        IntegrationsResult::Listed { entries } => {
            for entry in &entries {
                println!(
                    "integrations list name={} category={} status={}",
                    entry.name,
                    entry.category.as_str(),
                    entry.status.as_str(),
                );
            }
        }
    }

    Ok(())
}

fn execute_channel(action: ChannelActionTemplate) -> Result<(), String> {
    let action = match action {
        ChannelActionTemplate::List => ChannelAction::List,
        ChannelActionTemplate::Start => ChannelAction::Start,
        ChannelActionTemplate::Doctor => ChannelAction::Doctor,
        ChannelActionTemplate::Add {
            channel_type,
            config,
        } => ChannelAction::Add {
            channel_type,
            config,
        },
        ChannelActionTemplate::Remove { name } => ChannelAction::Remove { name },
        ChannelActionTemplate::Serve { poll_interval_secs } => {
            ChannelAction::Serve { poll_interval_secs }
        }
    };

    let result = execute_channel_action(action)?;
    match result {
        ChannelResult::Listed {
            path,
            channels,
            running,
        } => {
            println!(
                "channel list count={} running={} path={}",
                channels.len(),
                running,
                path.display()
            );
            for channel in channels {
                println!(
                    "channel entry name={} type={} running={} health={} updated_at={} config={}",
                    channel.name,
                    channel.channel_type.as_str(),
                    channel.running,
                    channel
                        .last_health
                        .map(|status| status.as_str())
                        .unwrap_or("unknown"),
                    channel.updated_at,
                    channel.config
                );
            }
        }
        ChannelResult::Started {
            path,
            started,
            total_running,
        } => {
            println!(
                "channel start started={} running={} path={}",
                started,
                total_running,
                path.display()
            );
        }
        ChannelResult::Doctored {
            path,
            checks,
            healthy,
            unhealthy,
        } => {
            println!(
                "channel doctor count={} healthy={} unhealthy={} path={}",
                checks.len(),
                healthy,
                unhealthy,
                path.display()
            );
            for check in checks {
                println!(
                    "channel check name={} type={} status={} detail={} checked_at={}",
                    check.name,
                    check.channel_type.as_str(),
                    check.status.as_str(),
                    check.detail,
                    check.checked_at
                );
            }
        }
        ChannelResult::Added { path, channel } => {
            println!(
                "channel added name={} type={} path={}",
                channel.name,
                channel.channel_type.as_str(),
                path.display()
            );
        }
        ChannelResult::Removed {
            path,
            name,
            remaining,
        } => {
            println!(
                "channel removed name={} remaining={} path={}",
                name,
                remaining,
                path.display()
            );
        }
        ChannelResult::Served {
            channel_name,
            processed,
        } => {
            println!(
                "channel serve channel={} processed={}",
                channel_name, processed
            );
        }
    }

    Ok(())
}

fn execute_skills(action: SkillsActionTemplate) -> Result<(), String> {
    let action = match action {
        SkillsActionTemplate::List => SkillsAction::List,
        SkillsActionTemplate::Install { source } => SkillsAction::Install { source },
        SkillsActionTemplate::Remove { name } => SkillsAction::Remove { name },
    };

    let result = execute_skills_action(action)?;
    match result {
        SkillsResult::Listed { path, skills } => {
            println!("skills list count={} path={}", skills.len(), path.display());
            for skill in skills {
                println!(
                    "skills entry name={} description={} source={}",
                    skill.name, skill.description, skill.source
                );
            }
        }
        SkillsResult::Installed {
            path,
            name,
            source,
            mode,
        } => {
            println!(
                "skills installed name={} source={} mode={} path={}",
                name,
                source,
                mode.as_str(),
                path.display()
            );
        }
        SkillsResult::Removed {
            path,
            name,
            removed,
        } => {
            println!(
                "skills removed name={} removed={} path={}",
                name,
                removed,
                path.display()
            );
        }
    }

    Ok(())
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
        revision: runtime.state.revision,
        mode: runtime.state.mode,
        facts: runtime.state.facts.len(),
        denied: runtime.state.denied_count,
        audit: runtime.state.audit_count,
        provider_id: compose.provider_id,
        provider_model: compose.provider_model,
        memory_enabled: compose.memory_enabled,
        memory_state: compose.memory_state.to_string(),
        tool_enabled: compose.tool_enabled,
        tool_state: compose.tool_state.to_string(),
        bootstrap_state: compose.bootstrap_state.to_string(),
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
        let mut runtime = CliRuntime::new_with_compose(
            String::from("system"),
            compose_config(None, None),
        );
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
        let mut runtime = CliRuntime::new_with_compose(
            String::from("system"),
            compose_config(None, None),
        );
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
