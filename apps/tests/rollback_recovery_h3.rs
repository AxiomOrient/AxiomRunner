use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "axiom-rollback-recovery-{label}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("test root should be created");
    root
}

fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/rollback_recovery.sh")
}

fn as_utf8(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("output should be UTF-8")
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directories should exist");
    }
    fs::write(path, contents).expect("file should be written");
}

#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = fs::metadata(path)
        .expect("file metadata should exist")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("permissions should be set");
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) {}

fn shell_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\"'\"'"))
}

fn run_script(args: &[String], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(script_path());
    cmd.args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.output()
        .expect("rollback_recovery.sh should execute successfully")
}

fn json_bool_field(json: &str, key: &str) -> Option<bool> {
    let token = format!("\"{key}\":");
    let start = json.find(&token)? + token.len();
    let tail = json[start..].trim_start();

    if tail.starts_with("true") {
        Some(true)
    } else if tail.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn json_u64_field(json: &str, key: &str) -> Option<u64> {
    let token = format!("\"{key}\":");
    let start = json.find(&token)? + token.len();
    let tail = json[start..].trim_start();
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();

    if digits.is_empty() {
        None
    } else {
        digits.parse::<u64>().ok()
    }
}

fn default_args(
    workspace_root: &Path,
    runtime_root: &Path,
    snapshot_root: &Path,
    health_file: &Path,
    report_file: &Path,
) -> Vec<String> {
    vec![
        String::from("--workspace-root"),
        workspace_root.display().to_string(),
        String::from("--runtime-root"),
        runtime_root.display().to_string(),
        String::from("--snapshot-root"),
        snapshot_root.display().to_string(),
        String::from("--health-file"),
        health_file.display().to_string(),
        String::from("--report"),
        report_file.display().to_string(),
    ]
}

#[test]
fn rollback_recovery_h3_fault_injection_recovers_with_retry() {
    let workspace_root = test_root("success");
    let runtime_root = workspace_root.join("runtime");
    let snapshot_root = workspace_root.join("snapshot");
    let health_file = workspace_root.join("runtime/health.status");
    let report_file = workspace_root.join("reports/recovery.json");
    let probe_script = workspace_root.join("probe_once_fail.sh");
    let probe_state = workspace_root.join("probe_state.marker");

    fs::create_dir_all(runtime_root.join("memory")).expect("runtime memory dir should exist");
    fs::create_dir_all(snapshot_root.join("memory")).expect("snapshot memory dir should exist");
    fs::create_dir_all(report_file.parent().expect("report parent should exist"))
        .expect("report parent should be created");

    write_file(
        &snapshot_root.join("config.toml"),
        "profile = \"safe\"\nversion = 2\n",
    );
    write_file(
        &snapshot_root.join("memory/MEMORY.md"),
        "# Snapshot Memory\n- rollback baseline\n",
    );

    write_file(
        &runtime_root.join("config.toml"),
        "profile = \"broken\"\nversion = 999\n",
    );
    write_file(
        &runtime_root.join("memory/MEMORY.md"),
        "# Runtime Memory\n- stale and corrupted\n",
    );
    write_file(&health_file, "state=degraded\n");

    write_file(
        &probe_script,
        "#!/usr/bin/env bash
set -euo pipefail
state_file=\"$1\"
health_file=\"$2\"
expected=\"$3\"

if [[ ! -f \"${state_file}\" ]]; then
  echo \"first-attempt-failed\" > \"${state_file}\"
  exit 1
fi

printf 'state=%s\\n' \"${expected}\" > \"${health_file}\"
",
    );
    set_executable(&probe_script);

    let probe_cmd = format!(
        "{} {} {} {}",
        shell_quote(&probe_script.display().to_string()),
        shell_quote(&probe_state.display().to_string()),
        shell_quote(&health_file.display().to_string()),
        shell_quote("running")
    );

    let args = default_args(
        &workspace_root,
        &runtime_root,
        &snapshot_root,
        &health_file,
        &report_file,
    );

    let output = run_script(
        &args,
        &[
            ("RECOVERY_MAX_RETRIES", "3"),
            ("RECOVERY_BACKOFF_MS", "10"),
            ("RECOVERY_TIMEOUT_MS", "5000"),
            ("RECOVERY_EXPECT_HEALTH", "running"),
            ("RECOVERY_PROBE_CMD", probe_cmd.as_str()),
        ],
    );

    let stdout = as_utf8(&output.stdout);
    let stderr = as_utf8(&output.stderr);
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );

    let runtime_config =
        fs::read_to_string(runtime_root.join("config.toml")).expect("runtime config should exist");
    let runtime_memory = fs::read_to_string(runtime_root.join("memory/MEMORY.md"))
        .expect("runtime memory should exist");

    assert_eq!(runtime_config, "profile = \"safe\"\nversion = 2\n");
    assert_eq!(runtime_memory, "# Snapshot Memory\n- rollback baseline\n");

    let report = fs::read_to_string(&report_file).expect("report file should be written");
    assert_eq!(stdout.trim(), report.trim());
    assert_eq!(json_bool_field(&report, "recovered"), Some(true));

    let attempts = json_u64_field(&report, "attempts").expect("attempts should be in report");
    assert!(
        attempts >= 2,
        "attempts should include retry, got {attempts}"
    );

    let health = fs::read_to_string(&health_file).expect("health file should be written by probe");
    assert!(health.contains("running"));

    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn rollback_recovery_h3_fails_when_runtime_root_is_outside_workspace() {
    let workspace_root = test_root("boundary-workspace");
    let outside_root = test_root("boundary-outside");

    let runtime_root = outside_root.join("runtime");
    let snapshot_root = workspace_root.join("snapshot");
    let health_file = workspace_root.join("runtime/health.status");
    let report_file = workspace_root.join("reports/recovery.json");

    fs::create_dir_all(runtime_root.join("memory")).expect("outside runtime should exist");
    fs::create_dir_all(snapshot_root.join("memory")).expect("snapshot memory dir should exist");
    fs::create_dir_all(
        report_file
            .parent()
            .expect("report parent should be available"),
    )
    .expect("report parent should be created");
    fs::create_dir_all(
        health_file
            .parent()
            .expect("health parent should be available"),
    )
    .expect("health parent should be created");

    write_file(&snapshot_root.join("config.toml"), "profile = \"safe\"\n");
    write_file(&snapshot_root.join("memory/MEMORY.md"), "# Snapshot\n");
    write_file(&health_file, "state=running\n");

    let args = default_args(
        &workspace_root,
        &runtime_root,
        &snapshot_root,
        &health_file,
        &report_file,
    );

    let output = run_script(
        &args,
        &[
            ("RECOVERY_MAX_RETRIES", "2"),
            ("RECOVERY_BACKOFF_MS", "1"),
            ("RECOVERY_TIMEOUT_MS", "2000"),
        ],
    );

    let stdout = as_utf8(&output.stdout);
    let stderr = as_utf8(&output.stderr);
    assert!(
        !output.status.success(),
        "expected boundary failure\nstdout:\n{stdout}\n\nstderr:\n{stderr}"
    );

    assert!(stdout.contains("runtime root is outside workspace root"));

    let report =
        fs::read_to_string(&report_file).expect("report should be written on boundary failure");
    assert_eq!(json_bool_field(&report, "recovered"), Some(false));
    assert_eq!(json_u64_field(&report, "attempts"), Some(0));

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(outside_root);
}
