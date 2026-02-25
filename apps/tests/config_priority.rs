use std::collections::HashMap;

#[path = "../src/config_loader.rs"]
#[allow(dead_code)]
mod config_loader;
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
        endpoint=http://file.local
    "#,
    )
    .expect("file config should parse");

    let env_values = HashMap::from([
        (String::from("AXIOM_PROFILE"), String::from("env")),
        (
            String::from("AXIOM_ENDPOINT"),
            String::from("http://env.local"),
        ),
    ]);
    let env = config_loader::parse_env_config(|key| env_values.get(key).cloned())
        .expect("env config should parse");

    let cli_args = vec![String::from("--endpoint=http://cli.local")];
    let cli = config_loader::parse_cli_config(&cli_args).expect("CLI config should parse");

    let resolved = config_loader::resolve_config(file, env, cli);

    assert_eq!(resolved.profile, "env");
    assert_eq!(resolved.endpoint, "http://cli.local");
}

#[test]
fn config_uses_defaults_when_no_sources_present() {
    let resolved = config_loader::resolve_config(
        config_loader::PartialConfig::default(),
        config_loader::PartialConfig::default(),
        config_loader::PartialConfig::default(),
    );

    assert_eq!(resolved.profile, "prod");
    assert_eq!(resolved.endpoint, "http://127.0.0.1:8080");
}

#[test]
fn config_cli_option_grammar_conformance_matrix() {
    assert_eq!(
        config_loader::parse_cli_config_option("--profile=dev"),
        Some((config_loader::CliConfigOption::Profile, "dev"))
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--endpoint=http://cli.local"),
        Some((config_loader::CliConfigOption::Endpoint, "http://cli.local"))
    );

    assert_eq!(config_loader::parse_cli_config_option("--profile"), None);
    assert_eq!(config_loader::parse_cli_config_option("--endpoint"), None);
    assert_eq!(
        config_loader::parse_cli_config_option("--profile dev"),
        None
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--endpoint cli"),
        None
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--PROFILE=dev"),
        None
    );
    assert_eq!(
        config_loader::parse_cli_config_option("--ENDPOINT=http://cli.local"),
        None
    );
}

#[test]
fn startup_and_config_parsers_share_config_option_grammar() {
    let startup = cli_args::parse_startup_args(vec![
        String::from("--profile=dev"),
        String::from("--endpoint=http://cli.local"),
        String::from("status"),
    ])
    .expect("startup args should parse");

    assert_eq!(
        startup.config_args,
        vec![
            String::from("--profile=dev"),
            String::from("--endpoint=http://cli.local"),
        ]
    );
    assert_eq!(startup.command_tokens, vec![String::from("status")]);

    let parsed =
        config_loader::parse_cli_config(&startup.config_args).expect("CLI config should parse");
    assert_eq!(parsed.profile.as_deref(), Some("dev"));
    assert_eq!(parsed.endpoint.as_deref(), Some("http://cli.local"));
}

#[test]
fn startup_parser_preserves_actor_and_config_file_spellings() {
    let spaced = cli_args::parse_startup_args(vec![
        String::from("--actor"),
        String::from("alice"),
        String::from("--config-file"),
        String::from("/tmp/axiom.cfg"),
        String::from("status"),
    ])
    .expect("spaced options should parse");

    assert_eq!(spaced.actor_id, "alice");
    assert_eq!(spaced.config_file_path.as_deref(), Some("/tmp/axiom.cfg"));

    let equals = cli_args::parse_startup_args(vec![
        String::from("--actor=bob"),
        String::from("--config-file=/tmp/axiom-equals.cfg"),
        String::from("status"),
    ])
    .expect("equals options should parse");

    assert_eq!(equals.actor_id, "bob");
    assert_eq!(
        equals.config_file_path.as_deref(),
        Some("/tmp/axiom-equals.cfg")
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

    let startup_endpoint_err = cli_args::parse_startup_args(vec![
        String::from("--endpoint"),
        String::from("http://cli.local"),
        String::from("status"),
    ])
    .expect_err("space-separated endpoint option should be rejected");
    assert_eq!(startup_endpoint_err, "unknown option '--endpoint'");

    let cli_profile_err = config_loader::parse_cli_config(&[String::from("--profile")])
        .expect_err("bare --profile should be rejected");
    assert_eq!(
        cli_profile_err.to_string(),
        "unknown CLI argument '--profile'"
    );

    let cli_endpoint_err = config_loader::parse_cli_config(&[String::from("--endpoint")])
        .expect_err("bare --endpoint should be rejected");
    assert_eq!(
        cli_endpoint_err.to_string(),
        "unknown CLI argument '--endpoint'"
    );
}
