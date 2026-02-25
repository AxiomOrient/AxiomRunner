use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

fn run_migrate(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_migrate"))
        .args(args)
        .output()
        .expect("migrate binary should run")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

fn unique_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    let pid = std::process::id();
    std::env::temp_dir().join(format!("axiom_migrate_h1_{name}_{pid}_{nonce}"))
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("path should be UTF-8")
}

fn write_markdown_records(path: &Path, records: &[(&str, &str, u64)]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory should be creatable");
    }

    let mut body = String::from(
        "# ZeroClaw Markdown Memory\n\n<!-- format: zeroclaw-memory-markdown-v1 -->\n",
    );
    for (key, value, updated_at) in records {
        let _ = writeln!(
            body,
            "- key_hex={};updated_at={};value_hex={}",
            hex_encode(key),
            updated_at,
            hex_encode(value)
        );
    }

    fs::write(path, body).expect("markdown memory file should be writable");
}

fn write_sqlite_records(path: &Path, records: &[(&str, &str, u64)]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("sqlite directory should be creatable");
    }

    let mut connection = Connection::open(path).expect("sqlite db should open for seeding");
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS memory(\
             key_hex TEXT PRIMARY KEY NOT NULL, \
             value_hex TEXT NOT NULL, \
             updated_at INTEGER NOT NULL\
             );",
        )
        .expect("sqlite schema should be creatable");

    let transaction = connection
        .transaction()
        .expect("sqlite transaction should start");
    for (key, value, updated_at) in records {
        let updated_at = i64::try_from(*updated_at).expect("test timestamp should fit i64");
        transaction
            .execute(
                "INSERT OR REPLACE INTO memory(key_hex, value_hex, updated_at) VALUES (?1, ?2, ?3);",
                params![hex_encode(key), hex_encode(value), updated_at],
            )
            .expect("sqlite seed row should be insertable");
    }
    transaction
        .commit()
        .expect("sqlite seed transaction should commit");
}

fn read_markdown_records(path: &Path) -> HashMap<String, (String, u64)> {
    let content = fs::read_to_string(path).expect("memory output should be readable");
    let mut out = HashMap::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if !line.starts_with("- key_hex=") {
            continue;
        }

        let payload = line
            .strip_prefix("- ")
            .expect("memory lines should include '- ' prefix");
        let mut key_hex: Option<String> = None;
        let mut value_hex: Option<String> = None;
        let mut updated_at: Option<u64> = None;

        for pair in payload.split(';') {
            let mut split = pair.splitn(2, '=');
            let key = split.next().unwrap_or_default().trim();
            let value = split.next().unwrap_or_default().trim();

            match key {
                "key_hex" => key_hex = Some(value.to_string()),
                "value_hex" => value_hex = Some(value.to_string()),
                "updated_at" => {
                    updated_at = Some(
                        value
                            .parse::<u64>()
                            .expect("updated_at should parse from output"),
                    )
                }
                _ => {}
            }
        }

        let key = hex_decode(&key_hex.expect("key_hex must be present"))
            .expect("key hex in output should decode");
        let value = hex_decode(&value_hex.expect("value_hex must be present"))
            .expect("value hex in output should decode");
        let timestamp = updated_at.expect("updated_at must be present");
        out.insert(key, (value, timestamp));
    }

    out
}

fn hex_encode(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(input: &str) -> Result<String, String> {
    if !input.len().is_multiple_of(2) {
        return Err(format!("odd-length hex payload: {input}"));
    }

    let mut bytes = Vec::with_capacity(input.len() / 2);
    let mut chars = input.chars();
    while let (Some(high), Some(low)) = (chars.next(), chars.next()) {
        let high = hex_nibble(high)?;
        let low = hex_nibble(low)?;
        bytes.push((high << 4) | low);
    }

    String::from_utf8(bytes).map_err(|error| format!("invalid utf8: {error}"))
}

fn hex_nibble(ch: char) -> Result<u8, String> {
    match ch {
        '0'..='9' => Ok((ch as u8) - b'0'),
        'a'..='f' => Ok((ch as u8) - b'a' + 10),
        'A'..='F' => Ok((ch as u8) - b'A' + 10),
        _ => Err(format!("invalid hex nibble: {ch}")),
    }
}

#[test]
fn migrate_h1_markdown_only_source() {
    let root = unique_root("markdown_only");
    let legacy_root = root.join("legacy");
    let target_root = root.join("target");
    let report_path = root.join("report.json");
    fs::create_dir_all(&legacy_root).expect("legacy root should be creatable");

    fs::write(
        legacy_root.join("config.toml"),
        "profile = \"dev\"\nendpoint=http://legacy.local\n",
    )
    .expect("config should be writable");
    fs::write(legacy_root.join("workspace"), "2.0.0\n").expect("workspace hint should be writable");
    write_markdown_records(
        &legacy_root.join("MEMORY.md"),
        &[("alpha", "one", 100), ("beta", "two", 250)],
    );

    let output = run_migrate(&[
        "--legacy-root",
        path_str(&legacy_root),
        "--target-root",
        path_str(&target_root),
        "--report",
        path_str(&report_path),
    ]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.trim().is_empty(), "stderr should be empty: {stderr}");
    assert!(stdout.contains("\"dry_run\":false"));
    assert!(stdout.contains("\"fatal\":false"));
    assert!(stdout.contains("\"markdown_records\":2"));
    assert!(stdout.contains("\"sqlite_records\":0"));
    assert!(stdout.contains("\"merged_records\":2"));
    assert!(stdout.contains("\"imported_records\":2"));
    assert!(stdout.contains("\"status\":\"exact\""));
    assert!(stdout.contains("\"errors\":[]"));

    let report_json = fs::read_to_string(&report_path).expect("report file should be written");
    assert_eq!(report_json, stdout.trim_end());

    let config_out = fs::read_to_string(target_root.join("config.toml"))
        .expect("target config should be written");
    assert_eq!(
        config_out,
        "profile = \"dev\"\nendpoint = \"http://legacy.local\"\n"
    );

    let memory_out = read_markdown_records(&target_root.join("memory").join("MEMORY.md"));
    assert_eq!(memory_out.len(), 2);
    assert_eq!(memory_out.get("alpha"), Some(&(String::from("one"), 100)));
    assert_eq!(memory_out.get("beta"), Some(&(String::from("two"), 250)));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn migrate_h1_sqlite_only_dry_run() {
    let root = unique_root("sqlite_only");
    let legacy_root = root.join("legacy");
    let target_root = root.join("target");
    fs::create_dir_all(&legacy_root).expect("legacy root should be creatable");

    fs::write(
        legacy_root.join("config.toml"),
        "profile=prod\nendpoint = \"http://sqlite.local\"\n",
    )
    .expect("config should be writable");
    fs::write(legacy_root.join("workspace"), "1.0.0\n").expect("workspace hint should be writable");
    write_sqlite_records(
        &legacy_root.join("memory").join("brain.db"),
        &[("sqlite_a", "value_a", 10), ("sqlite_b", "value_b", 20)],
    );

    let output = run_migrate(&[
        "--legacy-root",
        path_str(&legacy_root),
        "--target-root",
        path_str(&target_root),
        "--dry-run",
    ]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.trim().is_empty(), "stderr should be empty: {stderr}");
    assert!(stdout.contains("\"dry_run\":true"));
    assert!(stdout.contains("\"fatal\":false"));
    assert!(stdout.contains("\"markdown_records\":0"));
    assert!(stdout.contains("\"sqlite_records\":2"));
    assert!(stdout.contains("\"merged_records\":2"));
    assert!(stdout.contains("\"imported_records\":0"));
    assert!(stdout.contains("\"status\":\"legacy-bridge\""));
    assert!(stdout.contains("\"compatible\":true"));
    assert!(!target_root.join("config.toml").exists());
    assert!(!target_root.join("memory").join("MEMORY.md").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn migrate_h1_mixed_duplicate_key_resolution() {
    let root = unique_root("mixed");
    let legacy_root = root.join("legacy");
    let target_root = root.join("target");
    fs::create_dir_all(&legacy_root).expect("legacy root should be creatable");

    fs::write(
        legacy_root.join("config.toml"),
        "profile = \"prod\"\nendpoint = \"http://mixed.local\"\n",
    )
    .expect("config should be writable");
    fs::write(legacy_root.join("workspace"), "legacy-schema: 2.3.9\n")
        .expect("workspace hint should be writable");
    write_markdown_records(
        &legacy_root.join("MEMORY.md"),
        &[
            ("dup", "markdown_old", 100),
            ("gamma", "markdown_only", 150),
        ],
    );
    write_sqlite_records(
        &legacy_root.join("memory").join("brain.db"),
        &[("dup", "sqlite_new", 300), ("delta", "sqlite_only", 200)],
    );

    let output = run_migrate(&[
        "--legacy-root",
        path_str(&legacy_root),
        "--target-root",
        path_str(&target_root),
    ]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.trim().is_empty(), "stderr should be empty: {stderr}");
    assert!(stdout.contains("\"dry_run\":false"));
    assert!(stdout.contains("\"fatal\":false"));
    assert!(stdout.contains("\"markdown_records\":2"));
    assert!(stdout.contains("\"sqlite_records\":2"));
    assert!(stdout.contains("\"merged_records\":3"));
    assert!(stdout.contains("\"imported_records\":3"));
    assert!(stdout.contains("\"status\":\"compatible\""));
    assert!(stdout.contains("\"compatible\":true"));

    let memory_out = read_markdown_records(&target_root.join("memory").join("MEMORY.md"));
    assert_eq!(memory_out.len(), 3);
    assert_eq!(
        memory_out.get("dup"),
        Some(&(String::from("sqlite_new"), 300))
    );
    assert_eq!(
        memory_out.get("gamma"),
        Some(&(String::from("markdown_only"), 150))
    );
    assert_eq!(
        memory_out.get("delta"),
        Some(&(String::from("sqlite_only"), 200))
    );

    let _ = fs::remove_dir_all(root);
}
