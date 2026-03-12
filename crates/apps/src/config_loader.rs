use std::fmt::{Display, Formatter};

use crate::env_util::read_env_trimmed;

const ENV_PROFILE: &str = "AXONRUNNER_PROFILE";
const ENV_PROVIDER: &str = "AXONRUNNER_RUNTIME_PROVIDER";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub profile: String,
    pub provider: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            profile: String::from("prod"),
            provider: String::from("mock-local"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartialConfig {
    pub profile: Option<String>,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Selected<T> {
    value: T,
}

fn merge_optional<T>(
    default: Option<T>,
    file: Option<T>,
    environment: Option<T>,
    cli: Option<T>,
) -> Option<Selected<T>> {
    cli.or(environment)
        .or(file)
        .or(default)
        .map(|value| Selected { value })
}

pub fn load_config(args: &[String], file_contents: Option<&str>) -> Result<AppConfig, ConfigError> {
    let file = match file_contents {
        Some(contents) => parse_file_config(contents)?,
        None => PartialConfig::default(),
    };
    let env = parse_env_config(|key| read_env_trimmed(key).ok().flatten())?;
    let cli = parse_cli_config(args)?;

    Ok(resolve_config(file, env, cli))
}

pub fn resolve_config(file: PartialConfig, env: PartialConfig, cli: PartialConfig) -> AppConfig {
    let defaults = AppConfig::default();
    let default_profile = defaults.profile;
    let default_provider = defaults.provider;

    AppConfig {
        profile: merge_optional(
            Some(default_profile),
            file.profile,
            env.profile,
            cli.profile,
        )
        .expect("profile default must always exist")
        .value,
        provider: merge_optional(
            Some(default_provider),
            file.provider,
            env.provider,
            cli.provider,
        )
        .expect("provider default must always exist")
        .value,
    }
}

pub fn parse_file_config(contents: &str) -> Result<PartialConfig, ConfigError> {
    let mut partial = PartialConfig::default();

    for (index, raw_line) in contents.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (key, value) = line.split_once('=').ok_or_else(|| {
            ConfigError::new(format!(
                "invalid line {} in config file: '{}'",
                index + 1,
                line
            ))
        })?;
        let key = key.trim();
        let value = value.trim();

        match key {
            "profile" => partial.profile = Some(value.to_string()),
            "provider" => partial.provider = Some(value.to_string()),
            _ => {
                return Err(ConfigError::new(format!(
                    "unknown config key '{}' on line {}",
                    key,
                    index + 1
                )));
            }
        }
    }

    Ok(partial)
}

pub fn parse_env_config(
    mut read_env: impl FnMut(&str) -> Option<String>,
) -> Result<PartialConfig, ConfigError> {
    let mut partial = PartialConfig::default();

    if let Some(value) = read_env(ENV_PROFILE) {
        partial.profile = Some(value);
    }
    if let Some(value) = read_env(ENV_PROVIDER) {
        partial.provider = Some(value);
    }

    Ok(partial)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliConfigOption {
    Profile,
    Provider,
}

pub(crate) fn parse_cli_config_option(arg: &str) -> Option<(CliConfigOption, &str)> {
    if let Some(value) = arg.strip_prefix("--profile=") {
        return Some((CliConfigOption::Profile, value));
    }
    if let Some(value) = arg.strip_prefix("--provider=") {
        return Some((CliConfigOption::Provider, value));
    }
    None
}

pub fn parse_cli_config(args: &[String]) -> Result<PartialConfig, ConfigError> {
    let mut partial = PartialConfig::default();

    for arg in args {
        match parse_cli_config_option(arg) {
            Some((CliConfigOption::Profile, value)) => {
                partial.profile = Some(value.to_string());
            }
            Some((CliConfigOption::Provider, value)) => {
                partial.provider = Some(value.to_string());
            }
            None => {
                return Err(ConfigError::new(format!("unknown CLI argument '{arg}'")));
            }
        }
    }

    Ok(partial)
}

#[cfg(test)]
mod tests {
    use super::{
        AppConfig, CliConfigOption, PartialConfig, parse_cli_config, parse_cli_config_option,
        parse_env_config, parse_file_config, resolve_config,
    };

    #[test]
    fn resolve_config_uses_provider_precedence() {
        let file = PartialConfig {
            profile: Some(String::from("dev")),
            provider: Some(String::from("openai")),
        };
        let env = PartialConfig {
            profile: None,
            provider: Some(String::from("openrouter")),
        };
        let cli = PartialConfig {
            profile: None,
            provider: Some(String::from("ollama")),
        };

        let resolved = resolve_config(file, env, cli);

        assert_eq!(resolved.provider, "ollama");
    }

    #[test]
    fn parse_file_config_accepts_provider_key() {
        let parsed =
            parse_file_config("profile=prod\nprovider=gemini\n").expect("file config should parse");

        assert_eq!(parsed.provider.as_deref(), Some("gemini"));
    }

    #[test]
    fn parse_env_config_reads_provider() {
        let parsed = parse_env_config(|key| match key {
            "AXONRUNNER_RUNTIME_PROVIDER" => Some(String::from("openrouter")),
            _ => None,
        })
        .expect("env parse should succeed");

        assert_eq!(parsed.provider.as_deref(), Some("openrouter"));
    }

    #[test]
    fn parse_cli_config_parses_provider_option() {
        let args = vec![String::from("--provider=openai")];
        let parsed = parse_cli_config(&args).expect("cli config should parse");
        assert_eq!(parsed.provider.as_deref(), Some("openai"));
    }

    #[test]
    fn parse_cli_config_option_marks_provider_as_config_option() {
        let option = parse_cli_config_option("--provider=anthropic")
            .expect("provider option should be recognized");
        assert_eq!(option.0, CliConfigOption::Provider);
        assert_eq!(option.1, "anthropic");
    }

    #[test]
    fn default_config_keeps_mock_local_provider() {
        let config = AppConfig::default();
        assert_eq!(config.provider, "mock-local");
    }

    #[test]
    fn parse_file_config_rejects_unknown_key() {
        let err = parse_file_config("profile=prod\nunknown_key=value\n")
            .expect_err("unknown key should be rejected");
        assert!(
            err.to_string().contains("unknown config key"),
            "error should mention unknown key, got: {err}"
        );
        assert!(
            err.to_string().contains("unknown_key"),
            "error should name the offending key, got: {err}"
        );
    }

    #[test]
    fn parse_file_config_rejects_line_without_equals() {
        let err = parse_file_config("profile=prod\nno_equals_here\n")
            .expect_err("line without '=' should be rejected");
        assert!(
            err.to_string().contains("invalid line"),
            "error should describe the malformed line, got: {err}"
        );
    }

    #[test]
    fn parse_file_config_trims_whitespace_around_key_and_value() {
        let parsed = parse_file_config("  profile  =  dev  \n")
            .expect("whitespace-padded line should parse");
        assert_eq!(parsed.profile.as_deref(), Some("dev"));
    }

    #[test]
    fn parse_file_config_skips_comments_and_blank_lines() {
        let parsed = parse_file_config("# this is a comment\n\nprofile=test\n")
            .expect("comments and blank lines should be skipped");
        assert_eq!(parsed.profile.as_deref(), Some("test"));
    }
}
