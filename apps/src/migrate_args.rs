use std::path::PathBuf;

use crate::migrate_types::{CliArgs, DEFAULT_EXPECTED_SCHEMA};

const USAGE: &str = "usage:\n  migrate --legacy-root <path> --target-root <path> [options]\n\noptions:\n  --legacy-root <path>      required\n  --target-root <path>      required\n  --dry-run                 validate and report without writing target files\n  --report <path>           write the same JSON report to file\n  --expected-schema <ver>   expected schema version (default 2.0.0)";

pub fn parse_args(args: Vec<String>) -> Result<CliArgs, String> {
    let mut legacy_root: Option<PathBuf> = None;
    let mut target_root: Option<PathBuf> = None;
    let mut dry_run = false;
    let mut report_path: Option<PathBuf> = None;
    let mut expected_schema = String::from(DEFAULT_EXPECTED_SCHEMA);

    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];

        if arg == "--dry-run" {
            dry_run = true;
            index += 1;
            continue;
        }

        if arg == "--legacy-root" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| String::from("--legacy-root requires a path value"))?;
            legacy_root = Some(parse_path_option("--legacy-root", value)?);
            index += 1;
            continue;
        }

        if arg == "--target-root" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| String::from("--target-root requires a path value"))?;
            target_root = Some(parse_path_option("--target-root", value)?);
            index += 1;
            continue;
        }

        if arg == "--report" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| String::from("--report requires a path value"))?;
            report_path = Some(parse_path_option("--report", value)?);
            index += 1;
            continue;
        }

        if arg == "--expected-schema" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| String::from("--expected-schema requires a value"))?;
            expected_schema = parse_expected_schema(value)?;
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--legacy-root=") {
            legacy_root = Some(parse_path_option("--legacy-root", value)?);
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--target-root=") {
            target_root = Some(parse_path_option("--target-root", value)?);
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--report=") {
            report_path = Some(parse_path_option("--report", value)?);
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--expected-schema=") {
            expected_schema = parse_expected_schema(value)?;
            index += 1;
            continue;
        }

        return Err(format!("unknown option '{arg}'\n{USAGE}"));
    }

    let legacy_root =
        legacy_root.ok_or_else(|| format!("missing required option --legacy-root\n{USAGE}"))?;
    let target_root =
        target_root.ok_or_else(|| format!("missing required option --target-root\n{USAGE}"))?;

    Ok(CliArgs {
        legacy_root,
        target_root,
        dry_run,
        report_path,
        expected_schema,
    })
}

fn parse_path_option(name: &str, value: &str) -> Result<PathBuf, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{name} requires a non-empty path"));
    }
    Ok(PathBuf::from(trimmed))
}

fn parse_expected_schema(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(String::from("--expected-schema requires a non-empty value"));
    }
    Ok(trimmed.to_string())
}
