use std::collections::HashMap;

#[path = "../src/config_loader.rs"]
#[allow(dead_code)]
mod config_loader;
#[path = "../src/env_util.rs"]
#[allow(dead_code)]
mod env_util;
#[path = "../src/parse_util.rs"]
#[allow(dead_code)]
mod parse_util;
mod cli_command {
    pub const USAGE: &str = "usage";
}
#[path = "../src/cli_args.rs"]
#[allow(dead_code)]
mod cli_args;

#[test]
fn config_priority_cli_over_env_over_file_over_default() {
    let file = config_loader::parse_file_config(
        r#"
        profile=file
    "#,
    )
    .expect("file config should parse");

    let env_values = HashMap::from([(String::from("AXIOMRUNNER_PROFILE"), String::from("env"))]);
    let env = config_loader::parse_env_config(|key| env_values.get(key).cloned())
        .expect("env config should parse");

    let cli_args = vec![String::from("--provider=openai")];
    let cli = config_loader::parse_cli_config(&cli_args).expect("CLI config should parse");

    let resolved = config_loader::resolve_config(file, env, cli);

    assert_eq!(resolved.profile, "env");
    assert_eq!(resolved.provider, "openai");
}

#[test]
fn config_uses_defaults_when_no_sources_present() {
    let resolved = config_loader::resolve_config(
        config_loader::PartialConfig::default(),
        config_loader::PartialConfig::default(),
        config_loader::PartialConfig::default(),
    );

    assert_eq!(resolved.profile, "prod");
    assert_eq!(resolved.provider, "mock-local");
}

#[test]
fn config_cli_option_grammar_conformance_matrix() {
    assert_eq!(
        config_loader::parse_cli_config_option("--profile=dev"),
        Some((config_loader::CliConfigOption::Profile, "dev"))
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--provider=openai"),
        Some((config_loader::CliConfigOption::Provider, "openai"))
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--provider-model=gpt-5"),
        Some((config_loader::CliConfigOption::ProviderModel, "gpt-5"))
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--workspace=/tmp/work"),
        Some((config_loader::CliConfigOption::Workspace, "/tmp/work"))
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--state-path=/tmp/state"),
        Some((config_loader::CliConfigOption::StatePath, "/tmp/state"))
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--command-allowlist=git,cargo"),
        Some((
            config_loader::CliConfigOption::CommandAllowlist,
            "git,cargo"
        ))
    );

    assert_eq!(config_loader::parse_cli_config_option("--profile"), None);
    assert_eq!(config_loader::parse_cli_config_option("--provider"), None);
    assert_eq!(
        config_loader::parse_cli_config_option("--provider-model"),
        None
    );
    assert_eq!(config_loader::parse_cli_config_option("--workspace"), None);
    assert_eq!(config_loader::parse_cli_config_option("--state-path"), None);
    assert_eq!(
        config_loader::parse_cli_config_option("--command-allowlist"),
        None
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--profile dev"),
        None
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--provider openai"),
        None
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--PROFILE=dev"),
        None
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--PROVIDER=openai"),
        None
    );
}

#[test]
fn startup_and_config_parsers_share_config_option_grammar() {
    let startup = cli_args::parse_startup_args(vec![
        String::from("--profile=dev"),
        String::from("--provider=openai"),
        String::from("--provider-model=gpt-5"),
        String::from("--workspace=/tmp/work"),
        String::from("--state-path=/tmp/state"),
        String::from("--command-allowlist=git,cargo"),
        String::from("status"),
    ])
    .expect("startup args should parse");

    assert_eq!(
        startup.config_args,
        vec![
            String::from("--profile=dev"),
            String::from("--provider=openai"),
            String::from("--provider-model=gpt-5"),
            String::from("--workspace=/tmp/work"),
            String::from("--state-path=/tmp/state"),
            String::from("--command-allowlist=git,cargo"),
        ]
    );
    assert_eq!(startup.command_tokens, vec![String::from("status")]);

    let parsed =
        config_loader::parse_cli_config(&startup.config_args).expect("CLI config should parse");
    assert_eq!(parsed.profile.as_deref(), Some("dev"));
    assert_eq!(parsed.provider.as_deref(), Some("openai"));
    assert_eq!(parsed.provider_model.as_deref(), Some("gpt-5"));
    assert_eq!(
        parsed.workspace.as_deref(),
        Some(std::path::Path::new("/tmp/work"))
    );
    assert_eq!(
        parsed.state_path.as_deref(),
        Some(std::path::Path::new("/tmp/state"))
    );
    assert_eq!(
        parsed.command_allowlist,
        Some(vec![String::from("git"), String::from("cargo")])
    );
}

#[test]
fn startup_parser_accepts_spaced_global_config_options() {
    let startup = cli_args::parse_startup_args(vec![
        String::from("--profile"),
        String::from("dev"),
        String::from("--provider"),
        String::from("openai"),
        String::from("--provider-model"),
        String::from("gpt-5"),
        String::from("--workspace"),
        String::from("/tmp/work"),
        String::from("--state-path"),
        String::from("/tmp/state"),
        String::from("--command-allowlist"),
        String::from("git,cargo"),
        String::from("status"),
    ])
    .expect("spaced config options should parse");

    assert_eq!(
        startup.config_args,
        vec![
            String::from("--profile=dev"),
            String::from("--provider=openai"),
            String::from("--provider-model=gpt-5"),
            String::from("--workspace=/tmp/work"),
            String::from("--state-path=/tmp/state"),
            String::from("--command-allowlist=git,cargo"),
        ]
    );
    assert_eq!(startup.command_tokens, vec![String::from("status")]);
}

#[test]
fn startup_parser_preserves_actor_and_config_file_spellings() {
    let spaced = cli_args::parse_startup_args(vec![
        String::from("--actor"),
        String::from("alice"),
        String::from("--config-file"),
        String::from("/tmp/axiomrunner.cfg"),
        String::from("status"),
    ])
    .expect("spaced options should parse");

    assert_eq!(spaced.actor_id, "alice");
    assert_eq!(
        spaced.config_file_path.as_deref(),
        Some("/tmp/axiomrunner.cfg")
    );

    let equals = cli_args::parse_startup_args(vec![
        String::from("--actor=bob"),
        String::from("--config-file=/tmp/axiomrunner-equals.cfg"),
        String::from("status"),
    ])
    .expect("equals options should parse");

    assert_eq!(equals.actor_id, "bob");
    assert_eq!(
        equals.config_file_path.as_deref(),
        Some("/tmp/axiomrunner-equals.cfg")
    );
}

#[test]
fn parser_conformance_rejects_invalid_option_spellings() {
    let startup_profile_err = cli_args::parse_startup_args(vec![
        String::from("--profile"),
        String::from(""),
        String::from("status"),
    ])
    .expect_err("empty spaced profile value should be rejected");
    assert_eq!(startup_profile_err, "--profile must not be empty");

    let startup_provider_err = cli_args::parse_startup_args(vec![
        String::from("--provider"),
        String::from(" "),
        String::from("status"),
    ])
    .expect_err("blank spaced provider value should be rejected");
    assert_eq!(startup_provider_err, "--provider must not be empty");

    let cli_profile_err = config_loader::parse_cli_config(&[String::from("--profile")])
        .expect_err("bare --profile should be rejected");
    assert_eq!(
        cli_profile_err.to_string(),
        "unknown CLI argument '--profile'"
    );

    let cli_provider_err = config_loader::parse_cli_config(&[String::from("--provider")])
        .expect_err("bare --provider should be rejected");
    assert_eq!(
        cli_provider_err.to_string(),
        "unknown CLI argument '--provider'"
    );

    let cli_workspace_err = config_loader::parse_cli_config(&[String::from("--workspace")])
        .expect_err("bare --workspace should be rejected");
    assert_eq!(
        cli_workspace_err.to_string(),
        "unknown CLI argument '--workspace'"
    );
}
