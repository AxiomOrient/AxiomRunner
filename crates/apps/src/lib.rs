#![forbid(unsafe_code)]

extern crate self as axonrunner_apps;

mod cli_args;
mod cli_runtime;
mod config_loader;
mod dev_guard;
mod env_util;
mod runtime_compose;
mod status;

pub mod async_runtime_host;
pub mod cli_command;
pub mod display;
pub mod parse_util;

use crate::cli_args::parse_startup_args;
use crate::cli_command::parse_command;
use crate::cli_runtime::{CliRuntime, execute_command};
use std::process::ExitCode;

pub fn run_cli_entrypoint() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let startup = parse_startup_args(raw_args)?;

    let file_contents = match startup.config_file_path {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(contents) => Some(contents),
            Err(err) => return Err(format!("failed to read config file '{path}': {err}")),
        },
        None => None,
    };

    let config = config_loader::load_config(&startup.config_args, file_contents.as_deref())
        .map_err(|err| format!("config error: {err}"))?;
    dev_guard::enforce_current_build(&config)
        .map_err(|err| format!("release gate blocked startup: {err}"))?;
    let command = parse_command(&startup.command_tokens)?;

    let mut runtime = CliRuntime::new(startup.actor_id, &config)
        .map_err(|err| format!("runtime initialization error: {err}"))?;
    execute_command(&mut runtime, &config, command)
}
