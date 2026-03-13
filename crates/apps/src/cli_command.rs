use crate::goal_file::parse_goal_file;
use axonrunner_core::{Intent, RunGoal};
use std::path::Path;

pub const USAGE: &str = "\
usage:
  axonrunner_apps [global-options] <command> [command-args]

global-options:
  --config-file <path>
  --profile=<name>
  --provider=<id>
  --provider-model=<name>
  --workspace=<path>
  --state-path=<path>
  --command-allowlist=<cmds>
  --actor=<id>  (default: system)

commands:
  run <goal-file>
  status [run-id|latest]
  replay [run-id|latest]
  resume [run-id|latest]
  abort [run-id|latest]
  doctor [--json]
  health
  help

compatibility:
  batch [--reset-state] <intent-spec>...
  read <key>
  write <key> <value>
  remove <key>
  freeze
  halt

legacy intent-spec:
  read:<key>
  write:<key>=<value>
  remove:<key>
  freeze
  halt";

#[derive(Debug, Clone, PartialEq)]
pub enum CliCommand {
    Run(RunTemplate),
    Batch {
        intents: Vec<RunTemplate>,
        reset_state: bool,
    },
    Replay {
        target: String,
    },
    Resume {
        target: String,
    },
    Abort {
        target: String,
    },
    Doctor {
        json: bool,
    },
    Status {
        target: Option<String>,
    },
    Health,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyIntentTemplate {
    Read { key: String },
    Write { key: String, value: String },
    Remove { key: String },
    Freeze,
    Halt,
}

impl LegacyIntentTemplate {
    pub fn to_intent(&self, intent_id: String, actor_id: Option<String>) -> Intent {
        match self {
            LegacyIntentTemplate::Read { key } => Intent::read(intent_id, actor_id, key.clone()),
            LegacyIntentTemplate::Write { key, value } => {
                Intent::write(intent_id, actor_id, key.clone(), value.clone())
            }
            LegacyIntentTemplate::Remove { key } => {
                Intent::remove(intent_id, actor_id, key.clone())
            }
            LegacyIntentTemplate::Freeze => Intent::freeze_writes(intent_id, actor_id),
            LegacyIntentTemplate::Halt => Intent::halt(intent_id, actor_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunTemplate {
    LegacyIntent(LegacyIntentTemplate),
    GoalFile(GoalFileTemplate),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalFileTemplate {
    pub path: String,
    pub goal: RunGoal,
}

impl RunTemplate {
    pub fn goal_file(&self) -> Option<&GoalFileTemplate> {
        match self {
            Self::GoalFile(template) => Some(template),
            Self::LegacyIntent(_) => None,
        }
    }
}

pub fn parse_command(tokens: &[String]) -> Result<CliCommand, String> {
    let command = tokens
        .first()
        .ok_or_else(|| format!("missing command\n{USAGE}"))?;
    let args = &tokens[1..];

    match command.as_str() {
        "run" => parse_run_command(args),
        "doctor" => parse_doctor_command(args),
        "replay" => parse_replay_command(args),
        "resume" => parse_resume_command(args),
        "abort" => parse_abort_command(args),
        "read" | "write" | "remove" | "freeze" | "halt" => parse_legacy_run_alias(command, args),
        "status" => {
            let target = optional_one_arg(command, args)?;
            Ok(CliCommand::Status { target })
        }
        "help" => {
            no_args(command, args)?;
            Ok(CliCommand::Help)
        }
        "health" => {
            no_args(command, args)?;
            Ok(CliCommand::Health)
        }
        "batch" => parse_batch_command(args),
        _ => Err(format!("unknown command '{}'\n{USAGE}", command)),
    }
}

fn parse_doctor_command(args: &[String]) -> Result<CliCommand, String> {
    match args {
        [] => Ok(CliCommand::Doctor { json: false }),
        [arg] if arg == "--json" => Ok(CliCommand::Doctor { json: true }),
        _ => Err(format!(
            "command 'doctor' accepts only optional --json\n{USAGE}"
        )),
    }
}

fn parse_replay_command(args: &[String]) -> Result<CliCommand, String> {
    let target = exactly_one_arg("replay", args)?;
    Ok(CliCommand::Replay { target })
}

fn parse_run_command(args: &[String]) -> Result<CliCommand, String> {
    let intent_spec = exactly_one_arg("run", args)?;
    if Path::new(&intent_spec).is_file() {
        return Ok(CliCommand::Run(RunTemplate::GoalFile(GoalFileTemplate {
            path: intent_spec.clone(),
            goal: parse_goal_file(&intent_spec)?,
        })));
    }
    Ok(CliCommand::Run(RunTemplate::LegacyIntent(
        parse_legacy_intent_spec(&intent_spec)?,
    )))
}

fn parse_resume_command(args: &[String]) -> Result<CliCommand, String> {
    let target = exactly_one_arg("resume", args)?;
    Ok(CliCommand::Resume { target })
}

fn parse_abort_command(args: &[String]) -> Result<CliCommand, String> {
    let target = exactly_one_arg("abort", args)?;
    Ok(CliCommand::Abort { target })
}

fn parse_legacy_run_alias(command: &str, args: &[String]) -> Result<CliCommand, String> {
    match command {
        "read" => parse_run_command(&[format!("read:{}", exactly_one_arg(command, args)?)]),
        "write" => {
            if args.len() < 2 {
                return Err(format!("command 'write' requires <key> <value>\n{USAGE}"));
            }
            let key = args[0].trim();
            if key.is_empty() {
                return Err(String::from("command 'write' requires a non-empty key"));
            }
            let value = args[1..].join(" ");
            if value.trim().is_empty() {
                return Err(String::from("command 'write' requires a non-empty value"));
            }
            parse_run_command(&[format!("write:{key}={}", value.trim())])
        }
        "remove" => parse_run_command(&[format!("remove:{}", exactly_one_arg(command, args)?)]),
        "freeze" => {
            no_args(command, args)?;
            parse_run_command(&[String::from("freeze")])
        }
        "halt" => {
            no_args(command, args)?;
            parse_run_command(&[String::from("halt")])
        }
        _ => Err(format!("unknown command '{}'\n{USAGE}", command)),
    }
}

fn parse_batch_command(args: &[String]) -> Result<CliCommand, String> {
    let mut reset_state = false;
    let mut intents = Vec::new();

    for arg in args {
        if arg == "--reset-state" {
            reset_state = true;
            continue;
        }
        intents.push(RunTemplate::LegacyIntent(parse_legacy_intent_spec(arg)?));
    }

    if intents.is_empty() {
        return Err(format!(
            "command 'batch' requires at least one <intent-spec>\n{USAGE}"
        ));
    }

    Ok(CliCommand::Batch {
        intents,
        reset_state,
    })
}

fn parse_legacy_intent_spec(raw: &str) -> Result<LegacyIntentTemplate, String> {
    if let Some(key) = raw.strip_prefix("read:") {
        return Ok(LegacyIntentTemplate::Read {
            key: parse_intent_key(key, "read")?,
        });
    }
    if let Some(rest) = raw.strip_prefix("write:") {
        let (key, value) = rest.split_once('=').ok_or_else(|| {
            String::from("batch write intent must be in the form write:<key>=<value>")
        })?;
        let key = parse_intent_key(key, "write")?;
        if value.trim().is_empty() {
            return Err(String::from(
                "batch write intent requires a non-empty value",
            ));
        }
        return Ok(LegacyIntentTemplate::Write {
            key,
            value: value.trim().to_owned(),
        });
    }
    if let Some(key) = raw.strip_prefix("remove:") {
        return Ok(LegacyIntentTemplate::Remove {
            key: parse_intent_key(key, "remove")?,
        });
    }
    if raw == "freeze" {
        return Ok(LegacyIntentTemplate::Freeze);
    }
    if raw == "halt" {
        return Ok(LegacyIntentTemplate::Halt);
    }

    Err(format!(
        "unsupported batch intent '{raw}'. expected read/write/remove/freeze/halt"
    ))
}

fn parse_intent_key(raw: &str, label: &str) -> Result<String, String> {
    let key = raw.trim();
    if key.is_empty() {
        return Err(format!("batch {label} intent requires a non-empty key"));
    }
    Ok(key.to_owned())
}

fn exactly_one_arg(command: &str, args: &[String]) -> Result<String, String> {
    if args.len() != 1 {
        return Err(format!("command '{command}' requires exactly one argument"));
    }
    let value = args[0].trim();
    if value.is_empty() {
        return Err(format!("command '{command}' requires a non-empty argument"));
    }
    Ok(value.to_owned())
}

fn no_args(command: &str, args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(format!("command '{command}' does not accept arguments"))
    }
}

fn optional_one_arg(command: &str, args: &[String]) -> Result<Option<String>, String> {
    match args.len() {
        0 => Ok(None),
        1 => {
            let value = args[0].trim();
            if value.is_empty() {
                Err(format!("command '{command}' requires a non-empty argument"))
            } else {
                Ok(Some(value.to_owned()))
            }
        }
        _ => Err(format!("command '{command}' accepts at most one argument")),
    }
}
