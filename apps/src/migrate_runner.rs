use std::fs;
use std::path::Path;

use axiom_schema::compat::{CompatLevel, check_compatibility_from_str};

use crate::migrate_io::{load_legacy_config, write_outputs};
use crate::migrate_memory::{
    discover_markdown_paths, load_markdown_records, load_sqlite_records, merge_records,
};
use crate::migrate_types::{CliArgs, MigrationReport, SchemaCompatibility, SourcePaths};

pub fn run_migration(args: CliArgs) -> MigrationReport {
    let config_path = args.legacy_root.join("config.toml");
    let workspace_path = args.legacy_root.join("workspace");
    let sqlite_path = args.legacy_root.join("memory").join("brain.db");

    let mut report = MigrationReport::new(
        SourcePaths {
            legacy_root: args.legacy_root.clone(),
            target_root: args.target_root.clone(),
            config: config_path.clone(),
            workspace: workspace_path.clone(),
            markdown: Vec::new(),
            sqlite: sqlite_path.clone(),
            report: args.report_path,
        },
        args.expected_schema.clone(),
        args.dry_run,
    );

    let (legacy_spec, legacy_version) = read_workspace_schema_hint(&workspace_path, &mut report);
    report.schema =
        evaluate_schema_compatibility(legacy_spec, legacy_version, &args.expected_schema);

    let config = load_legacy_config(&config_path, &mut report);

    let markdown_paths = discover_markdown_paths(&args.legacy_root, &mut report);
    report.source_paths.markdown = markdown_paths.clone();

    let markdown_records = load_markdown_records(&markdown_paths, &mut report);
    report.counts.markdown_records = markdown_records.len();

    let sqlite_records = match load_sqlite_records(&sqlite_path, &mut report) {
        Ok(records) => records,
        Err(error) => {
            report.fatal = true;
            report.errors.push(error);
            Vec::new()
        }
    };
    report.counts.sqlite_records = sqlite_records.len();

    let merged = merge_records(markdown_records, sqlite_records);
    report.counts.merged_records = merged.len();

    if !args.dry_run && !report.fatal {
        if let Err(error) = write_outputs(&args.target_root, &config, &merged) {
            report.fatal = true;
            report.errors.push(error);
        } else {
            report.counts.imported_records = merged.len();
        }
    }

    report
}

fn read_workspace_schema_hint(
    workspace_path: &Path,
    report: &mut MigrationReport,
) -> (Option<String>, Option<String>) {
    if !workspace_path.exists() {
        return (None, None);
    }

    let contents = match fs::read_to_string(workspace_path) {
        Ok(contents) => contents,
        Err(error) => {
            report.errors.push(format!(
                "failed to read workspace schema hint '{}': {error}",
                workspace_path.display()
            ));
            return (None, None);
        }
    };

    let legacy_spec = first_non_empty_line(&contents);
    let legacy_version = legacy_spec
        .as_deref()
        .and_then(extract_version_token)
        .map(str::to_string);

    (legacy_spec, legacy_version)
}

fn evaluate_schema_compatibility(
    legacy_spec: Option<String>,
    legacy_version: Option<String>,
    expected_schema: &str,
) -> SchemaCompatibility {
    let (status, compatible) = match legacy_version.as_deref() {
        Some(version) => match check_compatibility_from_str(expected_schema, version) {
            Ok(report) => match report.level {
                CompatLevel::Exact => ("exact", true),
                CompatLevel::Compatible => ("compatible", true),
                CompatLevel::LegacyBridge => ("legacy-bridge", true),
                CompatLevel::Incompatible => ("mismatch", false),
            },
            Err(_) => ("invalid", false),
        },
        None => ("unknown", false),
    };

    SchemaCompatibility {
        legacy_spec,
        legacy_version,
        expected_schema: expected_schema.to_string(),
        status,
        compatible,
    }
}

fn first_non_empty_line(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn extract_version_token(spec: &str) -> Option<&str> {
    let mut start = None;
    let mut end = 0;

    for (index, ch) in spec.char_indices() {
        if start.is_none() {
            if ch.is_ascii_digit() {
                start = Some(index);
                end = index + ch.len_utf8();
            }
            continue;
        }

        if ch.is_ascii_digit() || ch == '.' {
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }

    let start = start?;
    let token = spec[start..end].trim_matches('.');
    if token.is_empty() { None } else { Some(token) }
}
