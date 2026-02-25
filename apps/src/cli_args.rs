use crate::cli_command::USAGE;
use crate::config_loader;
use crate::parse_util::parse_non_empty;

const DEFAULT_ACTOR_ID: &str = "system";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupArgs {
    pub config_args: Vec<String>,
    pub config_file_path: Option<String>,
    pub actor_id: String,
    pub command_tokens: Vec<String>,
}

pub fn parse_startup_args(args: Vec<String>) -> Result<StartupArgs, String> {
    let mut config_args = Vec::new();
    let mut config_file_path = None;
    let mut actor_id = String::from(DEFAULT_ACTOR_ID);
    let mut command_tokens = Vec::new();
    let mut iter = args.into_iter();
    let mut command_started = false;

    while let Some(arg) = iter.next() {
        if command_started {
            command_tokens.push(arg);
            continue;
        }

        if arg == "--config-file" {
            let value = iter
                .next()
                .ok_or_else(|| String::from("--config-file requires a path value"))?;
            config_file_path = Some(value);
            continue;
        }

        if arg == "--actor" {
            let value = iter
                .next()
                .ok_or_else(|| String::from("--actor requires an id value"))?;
            actor_id = parse_non_empty(&value, "--actor")?;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--config-file=") {
            if value.trim().is_empty() {
                return Err(String::from("--config-file requires a path value"));
            }
            config_file_path = Some(value.to_string());
            continue;
        }

        if let Some(value) = arg.strip_prefix("--actor=") {
            actor_id = parse_non_empty(value, "--actor")?;
            continue;
        }

        if is_config_option(&arg) {
            config_args.push(arg);
            continue;
        }

        if arg.starts_with("--") {
            return Err(format!("unknown option '{arg}'"));
        }

        command_started = true;
        command_tokens.push(arg);
    }

    if command_tokens.is_empty() {
        return Err(format!("missing command\n{USAGE}"));
    }

    Ok(StartupArgs {
        config_args,
        config_file_path,
        actor_id,
        command_tokens,
    })
}

fn is_config_option(arg: &str) -> bool {
    config_loader::parse_cli_config_option(arg).is_some()
}

