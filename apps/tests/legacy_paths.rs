#[path = "../../schema/src/legacy.rs"]
#[allow(dead_code)]
mod legacy;

use legacy::{
    LegacyPathScope, LegacyPathSpec, ParseLegacyPathError, is_legacy_path, legacy_path_rules,
    normalize_legacy_path, parse_legacy_path,
};

#[test]
fn legacy_paths_scope_and_spec_matrix() {
    let cases = [
        (
            "~/.zeroclaw/config.toml",
            LegacyPathScope::UserHome,
            LegacyPathSpec::ConfigToml,
        ),
        (
            "~/.zeroclaw/workspace",
            LegacyPathScope::UserHome,
            LegacyPathSpec::WorkspaceHint,
        ),
        (
            "memory/brain.db",
            LegacyPathScope::Workspace,
            LegacyPathSpec::MemorySqlite,
        ),
        (
            "MEMORY.md",
            LegacyPathScope::Workspace,
            LegacyPathSpec::MemoryRootMarkdown,
        ),
        (
            "memory/*.md",
            LegacyPathScope::Workspace,
            LegacyPathSpec::MemoryDailyMarkdown,
        ),
        (
            "memory/2026-02-17.md",
            LegacyPathScope::Workspace,
            LegacyPathSpec::MemoryDailyMarkdown,
        ),
    ];

    for (raw, expected_scope, expected_spec) in cases {
        let parsed = parse_legacy_path(raw).expect("legacy path should parse");
        assert_eq!(parsed.scope, expected_scope, "scope mismatch for '{raw}'");
        assert_eq!(parsed.spec, expected_spec, "spec mismatch for '{raw}'");
        assert!(is_legacy_path(raw), "expected '{raw}' to be valid");
    }
}

#[test]
fn legacy_path_normalization_is_pure_and_deterministic() {
    let normalized = normalize_legacy_path(r"  memory\\2026-02-17.md  ")
        .expect("path should normalize with slash conversion");
    assert_eq!(normalized, "memory/2026-02-17.md");

    let parsed = parse_legacy_path("memory//2026-02-17.md")
        .expect("collapsed separators should still parse");
    assert_eq!(parsed.normalized, "memory/2026-02-17.md");
}

#[test]
fn legacy_paths_reject_unknown_scope_or_spec() {
    assert_eq!(
        parse_legacy_path("").unwrap_err(),
        ParseLegacyPathError::Empty
    );
    assert_eq!(
        parse_legacy_path("memory/../brain.db").unwrap_err(),
        ParseLegacyPathError::InvalidSegment
    );
    assert_eq!(
        parse_legacy_path("tmp/MEMORY.md").unwrap_err(),
        ParseLegacyPathError::UnknownScope
    );
    assert_eq!(
        parse_legacy_path("~/.zeroclaw/credentials.toml").unwrap_err(),
        ParseLegacyPathError::UnknownSpec
    );
    assert_eq!(
        parse_legacy_path("memory/.md").unwrap_err(),
        ParseLegacyPathError::UnknownSpec
    );

    assert!(!is_legacy_path("memory/brain.sqlite"));
}

#[test]
fn legacy_path_rule_table_matches_contract() {
    let rules = legacy_path_rules();
    assert_eq!(rules.len(), 5);

    let patterns: Vec<&str> = rules.iter().map(|rule| rule.pattern).collect();
    assert_eq!(
        patterns,
        vec![
            "~/.zeroclaw/config.toml",
            "~/.zeroclaw/workspace",
            "memory/brain.db",
            "MEMORY.md",
            "memory/*.md",
        ]
    );
}
