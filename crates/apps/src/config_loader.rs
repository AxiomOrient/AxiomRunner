use std::fmt::{Display, Formatter};

use crate::parse_util::parse_tools_list;

const ENV_PROFILE: &str = "AXIOM_PROFILE";
const ENV_ENDPOINT: &str = "AXIOM_ENDPOINT";
const ENV_PROVIDER: &str = "AXIOM_PROVIDER";
const ENV_CHANNEL: &str = "AXIOM_RUNTIME_CHANNEL";
const ENV_TOOLS: &str = "AXIOM_RUNTIME_TOOLS";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub profile: String,
    pub endpoint: String,
    pub provider: String,
    pub channel: Option<String>,
    pub tools: Option<Vec<String>>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            profile: String::from("prod"),
            endpoint: String::from("http://127.0.0.1:8080"),
            provider: String::from("mock-local"),
            channel: None,
            tools: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartialConfig {
    pub profile: Option<String>,
    pub endpoint: Option<String>,
    pub provider: Option<String>,
    pub channel: Option<String>,
    pub tools: Option<Vec<String>>,
}

impl PartialConfig {
    pub fn merge(self, higher_priority: PartialConfig) -> PartialConfig {
        PartialConfig {
            profile: higher_priority.profile.or(self.profile),
            endpoint: higher_priority.endpoint.or(self.endpoint),
            provider: higher_priority.provider.or(self.provider),
            channel: higher_priority.channel.or(self.channel),
            tools: higher_priority.tools.or(self.tools),
        }
    }
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

pub fn load_config(args: &[String], file_contents: Option<&str>) -> Result<AppConfig, ConfigError> {
    let file = match file_contents {
        Some(contents) => parse_file_config(contents)?,
        None => PartialConfig::default(),
    };
    let env = parse_env_config(|key| std::env::var(key).ok())?;
    let cli = parse_cli_config(args)?;

    Ok(resolve_config(file, env, cli))
}

pub fn resolve_config(file: PartialConfig, env: PartialConfig, cli: PartialConfig) -> AppConfig {
    let merged = PartialConfig::default().merge(file).merge(env).merge(cli);
    let defaults = AppConfig::default();

    AppConfig {
        profile: merged.profile.unwrap_or(defaults.profile),
        endpoint: merged.endpoint.unwrap_or(defaults.endpoint),
        provider: merged.provider.unwrap_or(defaults.provider),
        channel: merged.channel.or(defaults.channel),
        tools: merged.tools.or(defaults.tools),
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
            "endpoint" => partial.endpoint = Some(value.to_string()),
            "provider" => partial.provider = Some(value.to_string()),
            "channel" => partial.channel = Some(value.to_string()),
            "tools" => {
                let tools = parse_tools_list(value);
                if !tools.is_empty() {
                    partial.tools = Some(tools);
                }
            }
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
    if let Some(value) = read_env(ENV_ENDPOINT) {
        partial.endpoint = Some(value);
    }
    if let Some(value) = read_env(ENV_PROVIDER) {
        partial.provider = Some(value);
    }
    if let Some(value) = read_env(ENV_CHANNEL) {
        partial.channel = Some(value);
    }
    if let Some(value) = read_env(ENV_TOOLS) {
        let tools = parse_tools_list(&value);
        if !tools.is_empty() {
            partial.tools = Some(tools);
        }
    }

    Ok(partial)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliConfigOption {
    Profile,
    Endpoint,
    Provider,
    Channel,
    Tools,
}

pub(crate) fn parse_cli_config_option(arg: &str) -> Option<(CliConfigOption, &str)> {
    if let Some(value) = arg.strip_prefix("--profile=") {
        return Some((CliConfigOption::Profile, value));
    }
    if let Some(value) = arg.strip_prefix("--endpoint=") {
        return Some((CliConfigOption::Endpoint, value));
    }
    if let Some(value) = arg.strip_prefix("--provider=") {
        return Some((CliConfigOption::Provider, value));
    }
    if let Some(value) = arg.strip_prefix("--channel=") {
        return Some((CliConfigOption::Channel, value));
    }
    if let Some(value) = arg.strip_prefix("--tool=") {
        return Some((CliConfigOption::Tools, value));
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
            Some((CliConfigOption::Endpoint, value)) => {
                partial.endpoint = Some(value.to_string());
            }
            Some((CliConfigOption::Provider, value)) => {
                partial.provider = Some(value.to_string());
            }
            Some((CliConfigOption::Channel, value)) => {
                partial.channel = Some(value.to_string());
            }
            Some((CliConfigOption::Tools, value)) => {
                let tools = parse_tools_list(value);
                if !tools.is_empty() {
                    partial.tools = Some(tools);
                }
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
            endpoint: None,
            provider: Some(String::from("openai")),
            channel: None,
            tools: None,
        };
        let env = PartialConfig {
            profile: None,
            endpoint: None,
            provider: Some(String::from("openrouter")),
            channel: None,
            tools: None,
        };
        let cli = PartialConfig {
            profile: None,
            endpoint: None,
            provider: Some(String::from("ollama")),
            channel: None,
            tools: None,
        };

        let resolved = resolve_config(file, env, cli);

        assert_eq!(resolved.provider, "ollama");
    }

    #[test]
    fn parse_file_config_accepts_provider_key() {
        let parsed =
            parse_file_config("profile=prod\nendpoint=http://127.0.0.1:8080\nprovider=gemini\n")
                .expect("file config should parse");

        assert_eq!(parsed.provider.as_deref(), Some("gemini"));
    }

    #[test]
    fn parse_env_config_reads_provider() {
        let parsed = parse_env_config(|key| match key {
            "AXIOM_PROVIDER" => Some(String::from("openrouter")),
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
