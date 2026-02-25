#[path = "../src/config.rs"]
mod config;

use config::{ConfigSource, Sourced, merge_config_sources, merge_optional, merge_sources};

#[test]
fn merge_uses_highest_priority_source() {
    let merged = merge_sources(&[
        Sourced::new("default", ConfigSource::Default),
        Sourced::new("file", ConfigSource::File),
        Sourced::new("env", ConfigSource::Environment),
        Sourced::new("cli", ConfigSource::Cli),
    ])
    .expect("should choose one value");

    assert_eq!(merged.value, "cli");
    assert_eq!(merged.source, ConfigSource::Cli);
}

#[test]
fn merge_same_priority_prefers_last_entry() {
    let merged = merge_config_sources(&[
        Sourced::new("old", ConfigSource::Environment),
        Sourced::new("new", ConfigSource::Environment),
    ])
    .expect("should choose one value");

    assert_eq!(merged.value, "new");
    assert_eq!(merged.source, ConfigSource::Environment);
}

#[test]
fn merge_optional_applies_default_file_env_cli_precedence() {
    let merged = merge_optional(Some("default"), Some("file"), Some("env"), Some("cli"))
        .expect("should choose one value");

    assert_eq!(merged.value, "cli");
    assert_eq!(merged.source, ConfigSource::Cli);
}

#[test]
fn merge_optional_handles_sparse_combinations() {
    let cases = [
        (None, Some("file"), None, None, "file", ConfigSource::File),
        (
            Some("default"),
            None,
            Some("env"),
            None,
            "env",
            ConfigSource::Environment,
        ),
        (
            None,
            None,
            Some("env"),
            Some("cli"),
            "cli",
            ConfigSource::Cli,
        ),
        (
            Some("default"),
            None,
            None,
            None,
            "default",
            ConfigSource::Default,
        ),
    ];

    for (default, file, environment, cli, expected_value, expected_source) in cases {
        let merged =
            merge_optional(default, file, environment, cli).expect("should choose one value");

        assert_eq!(merged.value, expected_value);
        assert_eq!(merged.source, expected_source);
    }
}

#[test]
fn merge_optional_returns_none_when_all_absent() {
    let merged = merge_optional::<&str>(None, None, None, None);
    assert!(merged.is_none());
}
