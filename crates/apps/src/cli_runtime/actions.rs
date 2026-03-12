use crate::agent_loop::{
    AgentAction, AgentExecutionContext, AgentResultKind, execute_agent_action,
};
use crate::channel::{ChannelAction, ChannelResult, execute_channel_action};
use crate::cli_command::{
    AgentActionTemplate, ChannelActionTemplate, CronActionTemplate, IntegrationsActionTemplate,
    OnboardActionTemplate, ServiceActionTemplate, SkillsActionTemplate,
};
use crate::cron::{CronAction, CronResult, execute_cron_action};
use crate::env_util::read_env_trimmed;
use crate::integrations::{IntegrationsAction, IntegrationsResult, execute_integrations_action};
use crate::onboard::{OnboardAction, OnboardResult, execute_onboard_action};
use crate::service::{ServiceAction, ServiceResult, execute_service_action};
use crate::skills::{SkillsAction, SkillsResult, execute_skills_action};
use axonrunner_adapters::contracts::ContextAdapter;

pub(super) fn execute_onboard(profile: &str, action: OnboardActionTemplate) -> Result<(), String> {
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

pub(super) fn execute_agent(
    action: AgentActionTemplate,
    context: Option<&dyn ContextAdapter>,
) -> Result<(), String> {
    // OTP gate: if AXONRUNNER_OTP_SECRET is set, require a valid TOTP code via AXONRUNNER_OTP_CODE.
    // If the env var is absent the gate is disabled and execution proceeds unchanged.
    if let Some(gate_result) = crate::otp_gate::OtpGate::load_from_env() {
        let gate = gate_result.map_err(|e| format!("OTP gate config error: {e}"))?;
        let provided = read_env_trimmed("AXONRUNNER_OTP_CODE")
            .map_err(|e| format!("OTP gate config error: {e}"))?
            .unwrap_or_default();
        if !gate.verify(&provided) {
            return Err(
                "OTP verification failed. Set AXONRUNNER_OTP_CODE=<6-digit-code> and retry."
                    .to_string(),
            );
        }
    }

    let agent = axonrunner_adapters::build_contract_agent("")
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
            println!(
                "agent complete turns={turn_count} reason={}",
                reason.as_str()
            );
        }
    }

    Ok(())
}

pub(super) fn execute_cron(action: CronActionTemplate) -> Result<(), String> {
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

pub(super) fn execute_service(action: ServiceActionTemplate) -> Result<(), String> {
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

pub(super) fn execute_integrations(action: IntegrationsActionTemplate) -> Result<(), String> {
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
        IntegrationsResult::RemovalPlanned { name, instructions } => {
            println!("integrations remove-plan name={name}");
            for step in &instructions {
                println!("  step: {step}");
            }
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

pub(super) fn execute_channel(action: ChannelActionTemplate) -> Result<(), String> {
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
                    channel.channel_type,
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
            failed,
            failures,
        } => {
            println!(
                "channel start started={} running={} failed={} path={}",
                started,
                total_running,
                failed,
                path.display()
            );
            for failure in &failures {
                println!("channel start failure {failure}");
            }
            if failed > 0 {
                return Err(format!("channel start failed unhealthy={failed}"));
            }
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
                    check.channel_type,
                    check.status.as_str(),
                    check.detail,
                    check.checked_at
                );
            }
            if unhealthy > 0 {
                return Err(format!("channel doctor failed unhealthy={unhealthy}"));
            }
        }
        ChannelResult::Added { path, channel } => {
            println!(
                "channel added name={} type={} path={}",
                channel.name,
                channel.channel_type,
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

pub(super) fn execute_skills(action: SkillsActionTemplate) -> Result<(), String> {
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
