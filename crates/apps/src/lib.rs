#![forbid(unsafe_code)]

extern crate self as axiomrunner_apps;

mod cli_args;
mod cli_runtime;
mod command_contract;
mod config_loader;
mod dev_guard;
mod doctor;
mod env_util;
mod goal_file;
mod operator_render;
mod replay;
mod run_commit;
mod runtime_compose;
mod status;
mod storage;
mod workspace_lock;

pub mod async_runtime_host;
pub mod cli_command;
pub mod display;
pub mod parse_util;

use crate::cli_args::parse_startup_args;
use crate::cli_command::parse_command;
use crate::cli_runtime::{CliRuntime, execute_command};
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliExit {
    Parse = 2,
    Config = 3,
    ReleaseGate = 4,
    RuntimeInit = 5,
    RuntimeExec = 6,
    RuntimeShutdown = 7,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliError {
    exit: CliExit,
    message: String,
}

impl CliError {
    fn new(exit: CliExit, prefix: &'static str, message: impl Into<String>) -> Self {
        Self {
            exit,
            message: format!("{prefix}: {}", message.into()),
        }
    }
}

pub fn run_cli_entrypoint() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", err.message);
            ExitCode::from(err.exit as u8)
        }
    }
}

fn run() -> Result<(), CliError> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let startup = parse_startup_args(raw_args)
        .map_err(|err| CliError::new(CliExit::Parse, "parse error", err))?;

    let file_contents = match startup.config_file_path {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(contents) => Some(contents),
            Err(err) => {
                return Err(CliError::new(
                    CliExit::Config,
                    "config error",
                    format!("failed to read config file '{path}': {err}"),
                ));
            }
        },
        None => None,
    };

    let command = parse_command(&startup.command_tokens)
        .map_err(|err| CliError::new(CliExit::Parse, "parse error", err))?;
    if matches!(command, crate::cli_command::CliCommand::Help) {
        println!("{}", crate::cli_command::USAGE);
        return Ok(());
    }

    let config = config_loader::load_config(&startup.config_args, file_contents.as_deref())
        .map_err(|err| CliError::new(CliExit::Config, "config error", err.to_string()))?;
    dev_guard::enforce_current_build(&config).map_err(|err| {
        CliError::new(CliExit::ReleaseGate, "release gate error", err.to_string())
    })?;
    if let crate::cli_command::CliCommand::Replay { target } = &command {
        return replay::execute_replay(&config, target)
            .map_err(|err| CliError::new(CliExit::RuntimeExec, "runtime execution error", err));
    }

    let mut runtime = CliRuntime::new(startup.actor_id, &config)
        .map_err(|err| CliError::new(CliExit::RuntimeInit, "runtime init error", err))?;
    let command_result = execute_command(&mut runtime, &config, command);
    let shutdown_result = runtime
        .shutdown()
        .map_err(|err| CliError::new(CliExit::RuntimeShutdown, "runtime shutdown error", err));

    match (command_result, shutdown_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(command_error), Ok(())) => Err(CliError::new(
            CliExit::RuntimeExec,
            "runtime execution error",
            command_error,
        )),
        (Ok(()), Err(shutdown_error)) => Err(shutdown_error),
        (Err(command_error), Err(shutdown_error)) => {
            let combined = format!("{command_error}; {}", shutdown_error.message);
            Err(CliError::new(
                CliExit::RuntimeExec,
                "runtime execution error",
                combined,
            ))
        }
    }
}
