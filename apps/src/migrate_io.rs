use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::hex_util::hex_encode;
use crate::migrate_types::{
    DEFAULT_ENDPOINT, DEFAULT_PROFILE, LegacyConfig, MemoryRecord, MigrationReport,
};

pub fn load_legacy_config(config_path: &Path, report: &mut MigrationReport) -> LegacyConfig {
    if !config_path.exists() {
        report.fatal = true;
        report.errors.push(format!(
            "missing required legacy config '{}'",
            config_path.display()
        ));
        return LegacyConfig {
            profile: DEFAULT_PROFILE.to_string(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
        };
    }

    let contents = match fs::read_to_string(config_path) {
        Ok(contents) => contents,
        Err(error) => {
            report.fatal = true;
            report.errors.push(format!(
                "failed to read legacy config '{}': {error}",
                config_path.display()
            ));
            return LegacyConfig {
                profile: DEFAULT_PROFILE.to_string(),
                endpoint: DEFAULT_ENDPOINT.to_string(),
            };
        }
    };

    let mut profile: Option<String> = None;
    let mut endpoint: Option<String> = None;

    for (line_number, raw_line) in contents.lines().enumerate() {
        let stripped = strip_inline_comment(raw_line);
        let line = stripped.trim();
        if line.is_empty() {
            continue;
        }

        let (key, raw_value) = match line.split_once('=') {
            Some((key, value)) => (key.trim(), value.trim()),
            None => {
                report.errors.push(format!(
                    "invalid config line {} in '{}': '{}'",
                    line_number + 1,
                    config_path.display(),
                    raw_line.trim()
                ));
                continue;
            }
        };

        let value = match parse_config_value(raw_value) {
            Ok(value) => value,
            Err(error) => {
                report.errors.push(format!(
                    "invalid config value for '{key}' at {}:{}: {error}",
                    config_path.display(),
                    line_number + 1
                ));
                continue;
            }
        };

        match key {
            "profile" => profile = Some(value),
            "endpoint" => endpoint = Some(value),
            _ => {
                report.errors.push(format!(
                    "unknown config key '{}' at {}:{} (ignored)",
                    key,
                    config_path.display(),
                    line_number + 1
                ));
            }
        }
    }

    let profile = match profile {
        Some(value) => value,
        None => {
            report.errors.push(format!(
                "missing 'profile' in '{}' (defaulting to '{}')",
                config_path.display(),
                DEFAULT_PROFILE
            ));
            DEFAULT_PROFILE.to_string()
        }
    };

    let endpoint = match endpoint {
        Some(value) => value,
        None => {
            report.errors.push(format!(
                "missing 'endpoint' in '{}' (defaulting to '{}')",
                config_path.display(),
                DEFAULT_ENDPOINT
            ));
            DEFAULT_ENDPOINT.to_string()
        }
    };

    LegacyConfig { profile, endpoint }
}

pub fn write_outputs(
    target_root: &Path,
    config: &LegacyConfig,
    records: &[MemoryRecord],
) -> Result<(), String> {
    let config_path = target_root.join("config.toml");
    let memory_path = target_root.join("memory").join("MEMORY.md");

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create config directory '{}': {error}",
                parent.display()
            )
        })?;
    }

    if let Some(parent) = memory_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create memory directory '{}': {error}",
                parent.display()
            )
        })?;
    }

    let config_body = format!(
        "profile = \"{}\"\nendpoint = \"{}\"\n",
        toml_escape(&config.profile),
        toml_escape(&config.endpoint)
    );
    fs::write(&config_path, config_body)
        .map_err(|error| format!("failed to write '{}': {error}", config_path.display()))?;

    let mut memory_body = String::from(
        "# ZeroClaw Markdown Memory\n\n<!-- format: zeroclaw-memory-markdown-v1 -->\n",
    );
    for record in records {
        let _ = writeln!(
            memory_body,
            "- key_hex={};updated_at={};value_hex={}",
            hex_encode(&record.key),
            record.updated_at,
            hex_encode(&record.value)
        );
    }
    fs::write(&memory_path, memory_body)
        .map_err(|error| format!("failed to write '{}': {error}", memory_path.display()))?;

    Ok(())
}

fn strip_inline_comment(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_quotes = false;
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' && in_quotes {
            out.push(ch);
            escaped = true;
            continue;
        }

        if ch == '"' {
            out.push(ch);
            in_quotes = !in_quotes;
            continue;
        }

        if ch == '#' && !in_quotes {
            break;
        }

        out.push(ch);
    }

    out
}

fn parse_config_value(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(String::from("empty value"));
    }

    if let Some(inner) = trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return parse_double_quoted(inner);
    }

    if trimmed.starts_with('"') || trimmed.ends_with('"') {
        return Err(String::from("unterminated quoted value"));
    }

    if let Some(inner) = trimmed
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
    {
        return Ok(inner.to_string());
    }

    Ok(trimmed.to_string())
}

fn parse_double_quoted(inner: &str) -> Result<String, String> {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        let next = chars
            .next()
            .ok_or_else(|| String::from("unterminated escape sequence"))?;
        match next {
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            _ => out.push(next),
        }
    }

    Ok(out)
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
