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

    let env_values = HashMap::from([(String::from("AXONRUNNER_PROFILE"), String::from("env"))]);
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

    assert_eq!(config_loader::parse_cli_config_option("--profile"), None);
    assert_eq!(config_loader::parse_cli_config_option("--provider"), None);
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
        String::from("status"),
    ])
    .expect("startup args should parse");

    assert_eq!(
        startup.config_args,
        vec![
            String::from("--profile=dev"),
            String::from("--provider=openai"),
        ]
    );
    assert_eq!(startup.command_tokens, vec![String::from("status")]);

    let parsed =
        config_loader::parse_cli_config(&startup.config_args).expect("CLI config should parse");
    assert_eq!(parsed.profile.as_deref(), Some("dev"));
    assert_eq!(parsed.provider.as_deref(), Some("openai"));
}

#[test]
fn startup_parser_preserves_actor_and_config_file_spellings() {
    let spaced = cli_args::parse_startup_args(vec![
        String::from("--actor"),
        String::from("alice"),
        String::from("--config-file"),
        String::from("/tmp/axonrunner.cfg"),
        String::from("status"),
    ])
    .expect("spaced options should parse");

    assert_eq!(spaced.actor_id, "alice");
    assert_eq!(
        spaced.config_file_path.as_deref(),
        Some("/tmp/axonrunner.cfg")
    );

    let equals = cli_args::parse_startup_args(vec![
        String::from("--actor=bob"),
        String::from("--config-file=/tmp/axonrunner-equals.cfg"),
        String::from("status"),
    ])
    .expect("equals options should parse");

    assert_eq!(equals.actor_id, "bob");
    assert_eq!(
        equals.config_file_path.as_deref(),
        Some("/tmp/axonrunner-equals.cfg")
    );
}

#[test]
fn parser_conformance_rejects_invalid_option_spellings() {
    let startup_profile_err = cli_args::parse_startup_args(vec![
        String::from("--profile"),
        String::from("dev"),
        String::from("status"),
    ])
    .expect_err("space-separated profile option should be rejected");
    assert_eq!(startup_profile_err, "unknown option '--profile'");

    let startup_provider_err = cli_args::parse_startup_args(vec![
        String::from("--provider"),
        String::from("openai"),
        String::from("status"),
    ])
    .expect_err("space-separated provider option should be rejected");
    assert_eq!(startup_provider_err, "unknown option '--provider'");

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
}
