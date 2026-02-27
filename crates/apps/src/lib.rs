#![forbid(unsafe_code)]

extern crate self as axiom_apps;

mod channel;
mod cli;
mod cli_args;
mod cli_runtime;
mod config_loader;
mod cron;
mod dev_guard;
mod doctor;
mod env_util;
mod hex_util;
mod identity_bootstrap;
mod integrations;
mod onboard;
mod otp_gate;
mod runtime_compose;
mod service;
mod skills;
mod skills_registry;
mod status;
mod time_util;

pub mod agent_loop;
pub mod async_runtime_host;
pub mod channel_serve;
pub mod cli_command;
pub mod daemon;
pub mod display;
pub mod estop;
pub mod gateway;
pub mod gateway_signature;
pub mod heartbeat;
pub mod metrics;
pub mod metrics_http;
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

    let mut runtime = CliRuntime::new(startup.actor_id, &config);
    execute_command(&mut runtime, &config, command)
}
