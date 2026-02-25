use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::hex_util::hex_decode;
use crate::migrate_types::{MemoryRecord, MigrationReport, SQLITE_QUERY};

pub fn discover_markdown_paths(legacy_root: &Path, report: &mut MigrationReport) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let root_memory = legacy_root.join("MEMORY.md");
    if root_memory.is_file() {
        paths.push(root_memory);
    }

    let memory_dir = legacy_root.join("memory");
    if memory_dir.exists() {
        match fs::read_dir(&memory_dir) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if path.is_file() && is_markdown_file(&path) {
                                paths.push(path);
                            }
                        }
                        Err(error) => {
                            report.errors.push(format!(
                                "failed to read entry under '{}': {error}",
                                memory_dir.display()
                            ));
                        }
                    }
                }
            }
            Err(error) => {
                report.errors.push(format!(
                    "failed to list markdown directory '{}': {error}",
                    memory_dir.display()
                ));
            }
        }
    }

    paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    paths
}

pub fn load_markdown_records(paths: &[PathBuf], report: &mut MigrationReport) -> Vec<MemoryRecord> {
    let mut records = Vec::new();

    for path in paths {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) => {
                report.errors.push(format!(
                    "failed to read markdown memory '{}': {error}",
                    path.display()
                ));
                continue;
            }
        };

        for (line_number, raw_line) in contents.lines().enumerate() {
            let line = raw_line.trim();
            if !line.starts_with("- key_hex=") {
                continue;
            }

            match parse_markdown_record(line) {
                Ok(record) => records.push(record),
                Err(error) => report.errors.push(format!(
                    "invalid markdown record at {}:{}: {error}",
                    path.display(),
                    line_number + 1
                )),
            }
        }
    }

    records
}

pub fn load_sqlite_records(
    sqlite_path: &Path,
    report: &mut MigrationReport,
) -> Result<Vec<MemoryRecord>, String> {
    if !sqlite_path.exists() {
        return Ok(Vec::new());
    }

    let connection = Connection::open(sqlite_path).map_err(|error| {
        format!(
            "failed to open sqlite db '{}': {error}",
            sqlite_path.display()
        )
    })?;
    let mut statement = connection.prepare(SQLITE_QUERY).map_err(|error| {
        format!(
            "sqlite query failed for '{}': {error}",
            sqlite_path.display()
        )
    })?;
    let mut rows = statement.query([]).map_err(|error| {
        format!(
            "sqlite query failed for '{}': {error}",
            sqlite_path.display()
        )
    })?;

    let mut records = Vec::new();
    let mut row_number = 0usize;
    while let Some(row) = rows.next().map_err(|error| {
        format!(
            "sqlite row iteration failed for '{}': {error}",
            sqlite_path.display()
        )
    })? {
        row_number += 1;

        let key_hex: String = match row.get(0) {
            Ok(value) => value,
            Err(error) => {
                report.errors.push(format!(
                    "invalid sqlite key_hex at {}:{}: {error}",
                    sqlite_path.display(),
                    row_number
                ));
                continue;
            }
        };

        let value_hex: String = match row.get(1) {
            Ok(value) => value,
            Err(error) => {
                report.errors.push(format!(
                    "invalid sqlite value_hex at {}:{}: {error}",
                    sqlite_path.display(),
                    row_number
                ));
                continue;
            }
        };

        let updated_at_raw: i64 = match row.get(2) {
            Ok(value) => value,
            Err(error) => {
                report.errors.push(format!(
                    "invalid sqlite updated_at at {}:{}: {error}",
                    sqlite_path.display(),
                    row_number
                ));
                continue;
            }
        };

        let key = match hex_decode(&key_hex) {
            Ok(value) => value,
            Err(error) => {
                report.errors.push(format!(
                    "invalid sqlite key at {}:{}: {error}",
                    sqlite_path.display(),
                    row_number
                ));
                continue;
            }
        };

        let value = match hex_decode(&value_hex) {
            Ok(value) => value,
            Err(error) => {
                report.errors.push(format!(
                    "invalid sqlite value at {}:{}: {error}",
                    sqlite_path.display(),
                    row_number
                ));
                continue;
            }
        };

        let updated_at = match u64::try_from(updated_at_raw) {
            Ok(timestamp) => timestamp,
            Err(_) => {
                report.errors.push(format!(
                    "invalid sqlite updated_at '{}' at {}:{}: must be non-negative integer",
                    updated_at_raw,
                    sqlite_path.display(),
                    row_number
                ));
                continue;
            }
        };

        records.push(MemoryRecord {
            key,
            value,
            updated_at,
        });
    }

    Ok(records)
}

pub fn merge_records(
    markdown_records: Vec<MemoryRecord>,
    sqlite_records: Vec<MemoryRecord>,
) -> Vec<MemoryRecord> {
    let mut merged: BTreeMap<String, MemoryRecord> = BTreeMap::new();

    for record in markdown_records.into_iter().chain(sqlite_records) {
        match merged.get(&record.key) {
            Some(existing) if record.updated_at <= existing.updated_at => {}
            _ => {
                merged.insert(record.key.clone(), record);
            }
        }
    }

    let mut records: Vec<MemoryRecord> = merged.into_values().collect();
    records.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.key.cmp(&b.key))
    });
    records
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn parse_markdown_record(line: &str) -> Result<MemoryRecord, String> {
    let payload = line
        .strip_prefix("- ")
        .ok_or_else(|| String::from("record line must start with '- '"))?;
    let mut key_hex: Option<String> = None;
    let mut value_hex: Option<String> = None;
    let mut updated_at: Option<u64> = None;

    for pair in payload.split(';') {
        let mut split = pair.splitn(2, '=');
        let field = split.next().unwrap_or_default().trim();
        let value = split.next().unwrap_or_default().trim();
        match field {
            "key_hex" => key_hex = Some(value.to_string()),
            "value_hex" => value_hex = Some(value.to_string()),
            "updated_at" => {
                let parsed = value
                    .parse::<u64>()
                    .map_err(|error| format!("invalid updated_at '{value}': {error}"))?;
                updated_at = Some(parsed);
            }
            _ => {}
        }
    }

    let key_hex = key_hex.ok_or_else(|| String::from("missing key_hex"))?;
    let value_hex = value_hex.ok_or_else(|| String::from("missing value_hex"))?;
    let updated_at = updated_at.ok_or_else(|| String::from("missing updated_at"))?;

    let key = hex_decode(&key_hex)?;
    let value = hex_decode(&value_hex)?;

    Ok(MemoryRecord {
        key,
        value,
        updated_at,
    })
}

