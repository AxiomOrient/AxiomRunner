use crate::goal_file::parse_goal_file_template;
use axonrunner_core::RunGoal;
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
  help";

#[derive(Debug, Clone, PartialEq)]
pub enum CliCommand {
    Run(RunTemplate),
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
pub struct GoalFileTemplate {
    pub path: String,
    pub goal: RunGoal,
    pub workflow_pack: Option<axonrunner_adapters::WorkflowPackContract>,
}

pub type RunTemplate = GoalFileTemplate;

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
    let goal_file = exactly_one_arg("run", args)?;
    if Path::new(&goal_file).is_file() {
        return Ok(CliCommand::Run(parse_goal_file_template(&goal_file)?));
    }
    Err(format!("goal file not found: {goal_file}"))
}

fn parse_resume_command(args: &[String]) -> Result<CliCommand, String> {
    let target = exactly_one_arg("resume", args)?;
    Ok(CliCommand::Resume { target })
}

fn parse_abort_command(args: &[String]) -> Result<CliCommand, String> {
    let target = exactly_one_arg("abort", args)?;
    Ok(CliCommand::Abort { target })
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
