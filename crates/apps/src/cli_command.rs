use axonrunner_core::Intent;

pub const USAGE: &str = "\
usage:
  axonrunner_apps [global-options] <command> [command-args]

global-options:
  --config-file <path>
  --profile=<name>
  --actor=<id>  (default: system)

commands:
  run <intent-spec>
  status
  batch [--reset-state] <intent-spec>...
  health

intent-spec:
  read:<key>
  write:<key>=<value>
  remove:<key>
  freeze
  halt";

#[derive(Debug, Clone, PartialEq)]
pub enum CliCommand {
    Run(IntentTemplate),
    Batch {
        intents: Vec<IntentTemplate>,
        reset_state: bool,
    },
    Status,
    Health,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentTemplate {
    Read { key: String },
    Write { key: String, value: String },
    Remove { key: String },
    Freeze,
    Halt,
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
}

pub fn parse_command(tokens: &[String]) -> Result<CliCommand, String> {
    let command = tokens
        .first()
        .ok_or_else(|| format!("missing command\n{USAGE}"))?;
    let args = &tokens[1..];

    match command.as_str() {
        "run" => parse_run_command(args),
        "read" | "write" | "remove" | "freeze" | "halt" => parse_legacy_run_alias(command, args),
        "status" => {
            no_args(command, args)?;
            Ok(CliCommand::Status)
        }
        "health" => {
            no_args(command, args)?;
            Ok(CliCommand::Health)
        }
        "batch" => parse_batch_command(args),
        _ => Err(format!("unknown command '{}'\n{USAGE}", command)),
    }
}

fn parse_run_command(args: &[String]) -> Result<CliCommand, String> {
    let intent_spec = exactly_one_arg("run", args)?;
    Ok(CliCommand::Run(parse_intent_spec(&intent_spec)?))
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
        intents.push(parse_intent_spec(arg)?);
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

fn parse_intent_spec(raw: &str) -> Result<IntentTemplate, String> {
    if let Some(key) = raw.strip_prefix("read:") {
        return Ok(IntentTemplate::Read {
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
        return Ok(IntentTemplate::Write {
            key,
            value: value.trim().to_owned(),
        });
    }
    if let Some(key) = raw.strip_prefix("remove:") {
        return Ok(IntentTemplate::Remove {
            key: parse_intent_key(key, "remove")?,
        });
    }
    if raw == "freeze" {
        return Ok(IntentTemplate::Freeze);
    }
    if raw == "halt" {
        return Ok(IntentTemplate::Halt);
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
