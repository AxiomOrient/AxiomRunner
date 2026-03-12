use axonrunner_core::Intent;

pub const USAGE: &str = "\
usage:
  axonrunner_apps [global-options] <command> [command-args]

global-options:
  --config-file <path>
  --profile=<name>
  --endpoint=<url>
  --actor=<id>  (default: system)

commands:
  onboard [onboard-options]
  agent [agent-options]
  read <key>
  write <key> <value>
  remove <key>
  freeze
  halt
  status
  batch [--reset-state] <intent-spec>...
  health
  doctor
  cron <list|add|remove> [cron-args]
  service <install|start|stop|status|uninstall>
  channel <list|start|doctor|add|remove|serve> [channel-args]
  integrations info <name>
  skills <list|install|remove> [skills-args]
  serve --mode=<gateway|daemon>

intent-spec:
  read:<key>
  write:<key>=<value>
  remove:<key>
  freeze
  halt";

#[derive(Debug, Clone, PartialEq)]
pub enum CliCommand {
    Onboard {
        action: OnboardActionTemplate,
    },
    Agent {
        action: AgentActionTemplate,
    },
    Run(IntentTemplate),
    Batch {
        intents: Vec<IntentTemplate>,
        reset_state: bool,
    },
    Status,
    Health,
    Doctor,
    Cron {
        action: CronActionTemplate,
    },
    Service {
        action: ServiceActionTemplate,
    },
    Channel {
        action: ChannelActionTemplate,
    },
    Integrations {
        action: IntegrationsActionTemplate,
    },
    Skills {
        action: SkillsActionTemplate,
    },
    Serve {
        mode: ServeMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServeMode {
    Gateway,
    Daemon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentTemplate {
    Read { key: String },
    Write { key: String, value: String },
    Remove { key: String },
    Freeze,
    Halt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CronActionTemplate {
    List,
    Add { expression: String, command: String },
    Remove { id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceActionTemplate {
    Install,
    Start,
    Stop,
    Status,
    Uninstall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardActionTemplate {
    pub interactive: bool,
    pub channels_only: bool,
    pub api_key: Option<String>,
    pub provider: Option<String>,
    pub memory: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentActionTemplate {
    pub cwd: Option<String>,
    pub message: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelActionTemplate {
    List,
    Start,
    Doctor,
    Add {
        channel_type: String,
        config: String,
    },
    Remove {
        name: String,
    },
    Serve {
        poll_interval_secs: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationsActionTemplate {
    Info { name: String },
    Install { name: String },
    Remove { name: String },
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillsActionTemplate {
    List,
    Install { source: String },
    Remove { name: String },
}

impl IntentTemplate {
    pub fn to_intent(&self, intent_id: String, actor_id: Option<String>) -> Intent {
        match self {
            IntentTemplate::Read { key } => Intent::read(intent_id, actor_id, key.clone()),
            IntentTemplate::Write { key, value } => {
                Intent::write(intent_id, actor_id, key.clone(), value.clone())
            }
            IntentTemplate::Remove { key } => Intent::remove(intent_id, actor_id, key.clone()),
            IntentTemplate::Freeze => Intent::freeze_writes(intent_id, actor_id),
            IntentTemplate::Halt => Intent::halt(intent_id, actor_id),
        }
    }

    pub fn read_key(&self) -> Option<&str> {
        match self {
            IntentTemplate::Read { key } => Some(key.as_str()),
            _ => None,
        }
    }
}

pub fn parse_command(tokens: &[String]) -> Result<CliCommand, String> {
    let command = tokens
        .first()
        .ok_or_else(|| format!("missing command\n{USAGE}"))?;
    let args = &tokens[1..];

    match command.as_str() {
        "onboard" => parse_onboard_command(args),
        "agent" => parse_agent_command(args),
        "read" => {
            let key = exactly_one_arg(command, args)?;
            Ok(CliCommand::Run(IntentTemplate::Read { key }))
        }
        "write" => {
            if args.len() < 2 {
                return Err(format!("command 'write' requires <key> <value>\n{USAGE}"));
            }
            let key = args[0].trim().to_string();
            if key.is_empty() {
                return Err(String::from("command 'write' requires a non-empty key"));
            }
            let value = args[1..].join(" ");
            if value.trim().is_empty() {
                return Err(String::from("command 'write' requires a non-empty value"));
            }
            Ok(CliCommand::Run(IntentTemplate::Write { key, value }))
        }
        "remove" => {
            let key = exactly_one_arg(command, args)?;
            Ok(CliCommand::Run(IntentTemplate::Remove { key }))
        }
        "freeze" => {
            no_args(command, args)?;
            Ok(CliCommand::Run(IntentTemplate::Freeze))
        }
        "halt" => {
            no_args(command, args)?;
            Ok(CliCommand::Run(IntentTemplate::Halt))
        }
        "status" => {
            no_args(command, args)?;
            Ok(CliCommand::Status)
        }
        "health" => {
            no_args(command, args)?;
            Ok(CliCommand::Health)
        }
        "doctor" => {
            no_args(command, args)?;
            Ok(CliCommand::Doctor)
        }
        "cron" => parse_cron_command(args),
        "service" => parse_service_command(args),
        "channel" => parse_channel_command(args),
        "integrations" => parse_integrations_command(args),
        "skills" => parse_skills_command(args),
        "batch" => parse_batch_command(args),
        "serve" => parse_serve_command(args),
        _ => Err(format!("unknown command '{}'\n{USAGE}", command)),
    }
}

fn parse_onboard_command(args: &[String]) -> Result<CliCommand, String> {
    let mut interactive = false;
    let mut channels_only = false;
    let mut api_key = None;
    let mut provider = None;
    let mut memory = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--interactive" => interactive = true,
            "--channels-only" => channels_only = true,
            "--api-key" => {
                api_key = Some(next_arg(args, &mut index, "onboard", "--api-key")?.to_owned())
            }
            "--provider" => {
                provider = Some(next_arg(args, &mut index, "onboard", "--provider")?.to_owned())
            }
            "--memory" => {
                memory = Some(next_arg(args, &mut index, "onboard", "--memory")?.to_owned())
            }
            _ => {
                if let Some(value) = arg.strip_prefix("--api-key=") {
                    api_key = Some(non_empty_value(value, "onboard --api-key")?);
                } else if let Some(value) = arg.strip_prefix("--provider=") {
                    provider = Some(non_empty_value(value, "onboard --provider")?);
                } else if let Some(value) = arg.strip_prefix("--memory=") {
                    memory = Some(non_empty_value(value, "onboard --memory")?);
                } else {
                    return Err(format!("unknown onboard option '{arg}'\n{USAGE}"));
                }
            }
        }
        index += 1;
    }

    Ok(CliCommand::Onboard {
        action: OnboardActionTemplate {
            interactive,
            channels_only,
            api_key,
            provider,
            memory,
        },
    })
}

fn parse_agent_command(args: &[String]) -> Result<CliCommand, String> {
    let mut cwd = None;
    let mut message = None;
    let mut model = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--cwd" | "-d" => cwd = Some(next_arg(args, &mut index, "agent", arg)?.to_owned()),
            "--message" | "-m" => {
                message = Some(next_arg(args, &mut index, "agent", arg)?.to_owned())
            }
            "--model" => model = Some(next_arg(args, &mut index, "agent", "--model")?.to_owned()),
            _ => {
                if let Some(value) = arg.strip_prefix("--cwd=") {
                    cwd = Some(non_empty_value(value, "agent --cwd")?);
                } else if let Some(value) = arg.strip_prefix("--message=") {
                    message = Some(non_empty_value(value, "agent --message")?);
                } else if let Some(value) = arg.strip_prefix("--model=") {
                    model = Some(non_empty_value(value, "agent --model")?);
                } else {
                    return Err(format!("unknown agent option '{arg}'\n{USAGE}"));
                }
            }
        }
        index += 1;
    }

    Ok(CliCommand::Agent {
        action: AgentActionTemplate {
            cwd,
            message,
            model,
        },
    })
}

fn parse_cron_command(args: &[String]) -> Result<CliCommand, String> {
    let subcommand = args
        .first()
        .ok_or_else(|| format!("command 'cron' requires <list|add|remove>\n{USAGE}"))?;
    let rest = &args[1..];

    match subcommand.as_str() {
        "list" => {
            no_args("cron list", rest)?;
            Ok(CliCommand::Cron {
                action: CronActionTemplate::List,
            })
        }
        "add" => {
            if rest.len() < 2 {
                return Err(format!(
                    "command 'cron add' requires <expression> <command>\n{USAGE}"
                ));
            }

            let expression = rest[0].trim().to_string();
            if expression.is_empty() {
                return Err(String::from(
                    "command 'cron add' requires a non-empty expression",
                ));
            }

            let command = rest[1..].join(" ");
            if command.trim().is_empty() {
                return Err(String::from(
                    "command 'cron add' requires a non-empty command",
                ));
            }

            Ok(CliCommand::Cron {
                action: CronActionTemplate::Add {
                    expression,
                    command,
                },
            })
        }
        "remove" => {
            let id = exactly_one_arg("cron remove", rest)?;
            Ok(CliCommand::Cron {
                action: CronActionTemplate::Remove { id },
            })
        }
        _ => Err(format!("unknown cron subcommand '{}'\n{USAGE}", subcommand)),
    }
}

fn parse_service_command(args: &[String]) -> Result<CliCommand, String> {
    let action = args.first().ok_or_else(|| {
        format!("command 'service' requires <install|start|stop|status|uninstall>\n{USAGE}")
    })?;
    let rest = &args[1..];
    no_args(&format!("service {action}"), rest)?;

    let action = match action.as_str() {
        "install" => ServiceActionTemplate::Install,
        "start" => ServiceActionTemplate::Start,
        "stop" => ServiceActionTemplate::Stop,
        "status" => ServiceActionTemplate::Status,
        "uninstall" => ServiceActionTemplate::Uninstall,
        _ => return Err(format!("unknown service subcommand '{}'\n{USAGE}", action)),
    };

    Ok(CliCommand::Service { action })
}

fn parse_channel_command(args: &[String]) -> Result<CliCommand, String> {
    let action = args.first().ok_or_else(|| {
        format!("command 'channel' requires <list|start|doctor|add|remove|serve>\n{USAGE}")
    })?;
    let rest = &args[1..];

    match action.as_str() {
        "list" => {
            no_args("channel list", rest)?;
            Ok(CliCommand::Channel {
                action: ChannelActionTemplate::List,
            })
        }
        "start" => {
            no_args("channel start", rest)?;
            Ok(CliCommand::Channel {
                action: ChannelActionTemplate::Start,
            })
        }
        "doctor" => {
            no_args("channel doctor", rest)?;
            Ok(CliCommand::Channel {
                action: ChannelActionTemplate::Doctor,
            })
        }
        "add" => {
            if rest.len() < 2 {
                return Err(format!(
                    "command 'channel add' requires <channel-type> <config>\n{USAGE}"
                ));
            }

            let channel_type = rest[0].trim().to_string();
            if channel_type.is_empty() {
                return Err(String::from(
                    "command 'channel add' requires a non-empty channel-type",
                ));
            }

            let config = rest[1..].join(" ");
            if config.trim().is_empty() {
                return Err(String::from(
                    "command 'channel add' requires a non-empty config",
                ));
            }

            Ok(CliCommand::Channel {
                action: ChannelActionTemplate::Add {
                    channel_type,
                    config,
                },
            })
        }
        "remove" => {
            let name = exactly_one_arg("channel remove", rest)?;
            Ok(CliCommand::Channel {
                action: ChannelActionTemplate::Remove { name },
            })
        }
        "serve" => {
            let poll_interval_secs = parse_channel_serve_args(rest)?;
            Ok(CliCommand::Channel {
                action: ChannelActionTemplate::Serve { poll_interval_secs },
            })
        }
        _ => Err(format!("unknown channel subcommand '{}'\n{USAGE}", action)),
    }
}

fn parse_channel_serve_args(args: &[String]) -> Result<u64, String> {
    let mut poll_interval_secs: u64 = 2;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if arg == "--poll-interval" {
            index += 1;
            if index >= args.len() {
                return Err(format!(
                    "channel serve --poll-interval requires a value\n{USAGE}"
                ));
            }
            poll_interval_secs = args[index].trim().parse::<u64>().map_err(|_| {
                format!("channel serve --poll-interval must be a non-negative integer\n{USAGE}")
            })?;
        } else if let Some(value) = arg.strip_prefix("--poll-interval=") {
            poll_interval_secs = value.trim().parse::<u64>().map_err(|_| {
                format!("channel serve --poll-interval must be a non-negative integer\n{USAGE}")
            })?;
        } else {
            return Err(format!("unknown channel serve option '{arg}'\n{USAGE}"));
        }
        index += 1;
    }

    Ok(poll_interval_secs)
}

fn parse_batch_command(args: &[String]) -> Result<CliCommand, String> {
    if args.is_empty() {
        return Err(format!(
            "command 'batch' requires at least one intent-spec\n{USAGE}"
        ));
    }

    let (reset_state, intent_args) = if args[0] == "--reset-state" {
        (true, &args[1..])
    } else {
        (false, args)
    };

    if intent_args.is_empty() {
        return Err(format!(
            "command 'batch' requires at least one intent-spec\n{USAGE}"
        ));
    }

    let intents = parse_intent_specs(intent_args)?;
    Ok(CliCommand::Batch {
        intents,
        reset_state,
    })
}

fn parse_integrations_command(args: &[String]) -> Result<CliCommand, String> {
    let action = args.first().ok_or_else(|| {
        format!("command 'integrations' requires 'info|install|remove|list'\n{USAGE}")
    })?;
    let rest = &args[1..];

    match action.as_str() {
        "info" => {
            let name = exactly_one_arg("integrations info", rest)?;
            Ok(CliCommand::Integrations {
                action: IntegrationsActionTemplate::Info { name },
            })
        }
        "install" => {
            let name = exactly_one_arg("integrations install", rest)?;
            Ok(CliCommand::Integrations {
                action: IntegrationsActionTemplate::Install { name },
            })
        }
        "remove" => {
            let name = exactly_one_arg("integrations remove", rest)?;
            Ok(CliCommand::Integrations {
                action: IntegrationsActionTemplate::Remove { name },
            })
        }
        "list" => {
            if !rest.is_empty() {
                return Err(format!("'integrations list' takes no arguments\n{USAGE}"));
            }
            Ok(CliCommand::Integrations {
                action: IntegrationsActionTemplate::List,
            })
        }
        _ => Err(format!(
            "unknown integrations subcommand '{}'\n{USAGE}",
            action
        )),
    }
}

fn parse_skills_command(args: &[String]) -> Result<CliCommand, String> {
    let action = args
        .first()
        .ok_or_else(|| format!("command 'skills' requires <list|install|remove>\n{USAGE}"))?;
    let rest = &args[1..];

    match action.as_str() {
        "list" => {
            no_args("skills list", rest)?;
            Ok(CliCommand::Skills {
                action: SkillsActionTemplate::List,
            })
        }
        "install" => {
            let source = exactly_one_arg("skills install", rest)?;
            Ok(CliCommand::Skills {
                action: SkillsActionTemplate::Install { source },
            })
        }
        "remove" => {
            let name = exactly_one_arg("skills remove", rest)?;
            Ok(CliCommand::Skills {
                action: SkillsActionTemplate::Remove { name },
            })
        }
        _ => Err(format!("unknown skills subcommand '{}'\n{USAGE}", action)),
    }
}

fn parse_serve_command(args: &[String]) -> Result<CliCommand, String> {
    let mut mode = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if arg == "--mode" {
            let value = next_arg(args, &mut index, "serve", "--mode")?;
            mode = Some(parse_serve_mode(value)?);
        } else if let Some(value) = arg.strip_prefix("--mode=") {
            mode = Some(parse_serve_mode(value)?);
        } else {
            return Err(format!("unknown serve option '{arg}'\n{USAGE}"));
        }
        index += 1;
    }

    let mode =
        mode.ok_or_else(|| format!("command 'serve' requires --mode=<gateway|daemon>\n{USAGE}"))?;
    Ok(CliCommand::Serve { mode })
}

fn parse_serve_mode(value: &str) -> Result<ServeMode, String> {
    let mode = value.trim();
    match mode {
        "gateway" => Ok(ServeMode::Gateway),
        "daemon" => Ok(ServeMode::Daemon),
        _ => Err(format!("unknown serve mode '{mode}'\n{USAGE}")),
    }
}

fn parse_intent_specs(args: &[String]) -> Result<Vec<IntentTemplate>, String> {
    let mut intents = Vec::with_capacity(args.len());
    for spec in args {
        intents.push(parse_intent_spec(spec)?);
    }
    Ok(intents)
}

fn next_arg<'a>(
    args: &'a [String],
    index: &mut usize,
    cmd: &str,
    flag: &str,
) -> Result<&'a str, String> {
    *index += 1;
    if *index >= args.len() {
        return Err(format!(
            "command '{cmd}' requires value for {flag}\n{USAGE}"
        ));
    }
    let value = args[*index].trim();
    if value.is_empty() {
        return Err(format!("command '{cmd}' {flag} requires a non-empty value"));
    }
    Ok(value)
}

fn non_empty_value(raw: &str, label: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(format!("command '{label}' requires a non-empty value"));
    }
    Ok(value.to_string())
}

fn exactly_one_arg(command: &str, args: &[String]) -> Result<String, String> {
    if args.len() != 1 {
        return Err(format!(
            "command '{command}' requires exactly one argument\n{USAGE}"
        ));
    }

    let value = args[0].trim();
    if value.is_empty() {
        return Err(format!("command '{command}' argument cannot be empty"));
    }

    Ok(value.to_string())
}

fn no_args(command: &str, args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "command '{command}' does not accept arguments\n{USAGE}"
        ))
    }
}

/// Intermediate parsed representation shared between CLI and gateway intent parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IntentSpecVariant {
    Read { key: String },
    Write { key: String, value: String },
    Remove { key: String },
    Freeze,
    Halt,
}

/// Parse an intent spec string into a typed variant.
/// Format: "read:<key>", "write:<key>=<value>", "remove:<key>", "freeze", "halt"
pub(crate) fn parse_intent_spec_raw(spec: &str) -> Result<IntentSpecVariant, String> {
    let trimmed = spec.trim();
    if trimmed.eq_ignore_ascii_case("freeze") {
        return Ok(IntentSpecVariant::Freeze);
    }
    if trimmed.eq_ignore_ascii_case("halt") {
        return Ok(IntentSpecVariant::Halt);
    }
    if let Some(key) = trimmed.strip_prefix("read:") {
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("invalid intent-spec '{spec}': read key is empty"));
        }
        return Ok(IntentSpecVariant::Read {
            key: key.to_owned(),
        });
    }
    if let Some(key) = trimmed.strip_prefix("remove:") {
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("invalid intent-spec '{spec}': remove key is empty"));
        }
        return Ok(IntentSpecVariant::Remove {
            key: key.to_owned(),
        });
    }
    if let Some(payload) = trimmed.strip_prefix("write:") {
        let (key, value) = payload
            .split_once('=')
            .ok_or_else(|| format!("invalid intent-spec '{spec}': expected write:<key>=<value>"))?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            return Err(format!("invalid intent-spec '{spec}': write key is empty"));
        }
        if value.is_empty() {
            return Err(format!(
                "invalid intent-spec '{spec}': write value is empty"
            ));
        }
        return Ok(IntentSpecVariant::Write {
            key: key.to_owned(),
            value: value.to_owned(),
        });
    }
    Err(format!("invalid intent-spec '{spec}'"))
}

fn parse_intent_spec(spec: &str) -> Result<IntentTemplate, String> {
    match parse_intent_spec_raw(spec)? {
        IntentSpecVariant::Read { key } => Ok(IntentTemplate::Read { key }),
        IntentSpecVariant::Write { key, value } => Ok(IntentTemplate::Write { key, value }),
        IntentSpecVariant::Remove { key } => Ok(IntentTemplate::Remove { key }),
        IntentSpecVariant::Freeze => Ok(IntentTemplate::Freeze),
        IntentSpecVariant::Halt => Ok(IntentTemplate::Halt),
    }
}
