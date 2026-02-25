mod agent_loop;
mod channel;
mod channel_serve;
mod estop;
mod otp_gate;
mod cli_args;
mod env_util;
mod hex_util;
mod parse_util;
use axiom_apps::cli_command;
mod cli_runtime;
mod config_loader;
mod cron;
mod dev_guard;
mod display;
mod doctor;
mod identity_bootstrap;
mod integrations;
mod migrate_args;
mod migrate_io;
mod migrate_memory;
mod migrate_report;
mod migrate_runner;
mod migrate_types;
mod onboard;
mod runtime_compose;
mod service;
mod skills;
mod skills_registry;
mod status;
mod time_util;

use crate::cli_args::parse_startup_args;
use crate::cli_command::parse_command;
use crate::cli_runtime::{CliRuntime, execute_command};
use std::process::ExitCode;

fn main() -> ExitCode {
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

    let mut runtime = CliRuntime::new(startup.actor_id, &config);
    execute_command(&mut runtime, &config, command)
}
