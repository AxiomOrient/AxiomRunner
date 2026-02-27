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
        "axiom-transition-rehearsal-h4-{label}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("test root should be created");
    root
}

fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/run_h4_transition_rehearsal.sh")
}

fn rollback_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/rollback_recovery.sh")
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

fn run_script(args: &[String], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(script_path());
    cmd.args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.output()
        .expect("run_h4_transition_rehearsal.sh should execute")
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
        String::from("--apps-bin"),
        String::from(env!("CARGO_BIN_EXE_axiom_apps")),
        String::from("--h2-bin"),
        String::from(env!("CARGO_BIN_EXE_h2_verify")),
        String::from("--report"),
        report_file.display().to_string(),
    ]
}

#[test]
fn transition_rehearsal_h4_passes_with_no_data_loss_and_recovered_rollback() {
    let workspace_root = test_root("success");
    let runtime_root = workspace_root.join("runtime");
    let snapshot_root = workspace_root.join("snapshot");
    let health_file = workspace_root.join("runtime/health.status");
    let report_file = workspace_root.join("reports/h4_transition_report.json");

    fs::create_dir_all(runtime_root.join("memory")).expect("runtime memory dir should exist");
    fs::create_dir_all(snapshot_root.join("memory")).expect("snapshot memory dir should exist");
    fs::create_dir_all(report_file.parent().expect("report parent should exist"))
        .expect("report parent should be created");

    write_file(
        &snapshot_root.join("config.toml"),
        "profile = \"safe\"\nversion = 7\n",
    );
    write_file(
        &snapshot_root.join("memory/MEMORY.md"),
        "# Snapshot Memory\n- baseline fact\n",
    );

    write_file(
        &runtime_root.join("config.toml"),
        "profile = \"drifted\"\nversion = 11\n",
    );
    write_file(
        &runtime_root.join("memory/MEMORY.md"),
        "# Runtime Memory\n- stale fact\n",
    );
    write_file(&health_file, "state=running\n");

    let mut args = default_args(
        &workspace_root,
        &runtime_root,
        &snapshot_root,
        &health_file,
        &report_file,
    );
    args.push(String::from("--rollback-script"));
    args.push(rollback_script_path().display().to_string());

    let output = run_script(
        &args,
        &[("H4_ALLOWED_DIFF", "0"), ("H4_ROLLBACK_SLO_MS", "5000")],
    );

    let stdout = as_utf8(&output.stdout);
    let stderr = as_utf8(&output.stderr);
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );

    let report = fs::read_to_string(&report_file).expect("report should be written");
    assert_eq!(json_bool_field(&report, "passed"), Some(true));
    assert_eq!(json_u64_field(&report, "data_loss_files"), Some(0));
    assert_eq!(json_bool_field(&report, "rollback_recovered"), Some(true));
    assert!(
        json_u64_field(&report, "rollback_elapsed_ms").is_some(),
        "rollback_elapsed_ms should be present and numeric in report: {report}"
    );
    assert!(
        report.contains("\"errors\": ["),
        "report should include errors array marker: {report}"
    );
    assert!(
        !report.contains("\"rollback_elapsed_ms\": ,"),
        "report should not contain malformed rollback_elapsed_ms field: {report}"
    );

    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn transition_rehearsal_h4_fails_when_runtime_root_is_outside_workspace() {
    let workspace_root = test_root("boundary-workspace");
    let outside_root = test_root("boundary-outside");

    let runtime_root = outside_root.join("runtime");
    let snapshot_root = workspace_root.join("snapshot");
    let health_file = workspace_root.join("runtime/health.status");
    let report_file = workspace_root.join("reports/h4_transition_report.json");

    fs::create_dir_all(runtime_root.join("memory")).expect("outside runtime should exist");
    fs::create_dir_all(snapshot_root.join("memory")).expect("snapshot memory dir should exist");
    fs::create_dir_all(report_file.parent().expect("report parent should exist"))
        .expect("report parent should be created");

    write_file(
        &snapshot_root.join("config.toml"),
        "profile = \"safe\"\nversion = 7\n",
    );
    write_file(
        &snapshot_root.join("memory/MEMORY.md"),
        "# Snapshot Memory\n- baseline fact\n",
    );
    write_file(
        &runtime_root.join("config.toml"),
        "profile = \"drifted\"\nversion = 11\n",
    );
    write_file(
        &runtime_root.join("memory/MEMORY.md"),
        "# Runtime Memory\n- stale fact\n",
    );
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
        &[("H4_ALLOWED_DIFF", "0"), ("H4_ROLLBACK_SLO_MS", "5000")],
    );

    let stdout = as_utf8(&output.stdout);
    let stderr = as_utf8(&output.stderr);
    assert!(
        !output.status.success(),
        "expected boundary failure\nstdout:\n{stdout}\n\nstderr:\n{stderr}"
    );

    let report = fs::read_to_string(&report_file).expect("report should be written");
    assert_eq!(json_bool_field(&report, "passed"), Some(false));

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(outside_root);
}
