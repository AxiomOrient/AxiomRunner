use axonrunner_adapters::{MemoryAdapter, memory::MarkdownMemoryAdapter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CLI_USAGE: &str = "\
usage:
  axonrunner_apps [global-options] <command> [command-args]

global-options:
  --config-file <path>
  --profile=<name>
  --actor=<id>  (default: system)

commands:
  run <intent-spec>
  status
  batch [--reset-state] <intent-spec>...
  health

intent-spec:
  read:<key>
  write:<key>=<value>
  remove:<key>
  freeze
  halt";

const SANITIZED_ENV_KEYS: &[&str] = &[
    "AXONRUNNER_PROFILE",
    "AXONRUNNER_CODEX_BIN",
    "AXONRUNNER_RUNTIME_PROVIDER",
    "AXONRUNNER_RUNTIME_PROVIDER_MODEL",
    "AXONRUNNER_RUNTIME_MAX_TOKENS",
    "AXONRUNNER_RUNTIME_MEMORY_PATH",
    "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
    "AXONRUNNER_RUNTIME_TOOL_LOG_PATH",
    "OPENAI_API_KEY",
];

fn run_cli(args: &[&str]) -> Output {
    run_cli_with_env(args, &[], "default")
}

fn run_cli_with_env(args: &[&str], env: &[(&str, &str)], label: &str) -> Output {
    let home = unique_path(&format!("home-{label}"), "dir");
    fs::create_dir_all(&home).expect("isolated home directory should be writable");
    let canonical_home = fs::canonicalize(&home).unwrap_or(home.clone());
    let tool_workspace = env
        .iter()
        .find_map(|(key, value)| {
            (*key == "AXONRUNNER_RUNTIME_TOOL_WORKSPACE").then(|| PathBuf::from(*value))
        })
        .unwrap_or_else(|| canonical_home.join(".axonrunner").join("workspace"));

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_axonrunner_apps"));
    cmd.env("HOME", &canonical_home).args(args);
    for key in SANITIZED_ENV_KEYS {
        cmd.env_remove(key);
    }
    if !env
        .iter()
        .any(|(key, _)| *key == "AXONRUNNER_RUNTIME_TOOL_LOG_PATH")
    {
        cmd.env(
            "AXONRUNNER_RUNTIME_TOOL_LOG_PATH",
            tool_workspace.join("runtime.log"),
        );
    }
    for (key, value) in env {
        cmd.env(key, value);
    }

    let output = cmd.output().expect("axonrunner_apps binary should run");
    let _ = fs::remove_dir_all(&canonical_home);
    output
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

fn unique_path(label: &str, extension: &str) -> PathBuf {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!(
        "axonrunner-e2e-cli-{label}-{}-{tick}.{extension}",
        std::process::id()
    ))
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("path should be UTF-8")
}

#[test]
fn e2e_cli_batch_pipeline_flow() {
    let output = run_cli(&[
        "batch",
        "write:alpha=42",
        "read:alpha",
        "remove:alpha",
        "read:alpha",
        "freeze",
        "halt",
    ]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("intent id=cli-1 kind=write outcome=accepted"));
    assert!(stdout.contains("read key=alpha value=42"));
    assert!(stdout.contains("batch completed count=6"));
}

#[test]
fn e2e_cli_run_composes_provider_memory_and_tool() {
    let memory_path = unique_path("compose-memory", "md");
    let workspace = unique_path("compose-workspace", "dir");

    let output = run_cli_with_env(
        &["--actor=system", "run", "write:alpha=42"],
        &[
            ("AXONRUNNER_RUNTIME_MEMORY_PATH", path_str(&memory_path)),
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
        ],
        "write-compose",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("intent id=cli-1 kind=write outcome=accepted"));

    let reader = MarkdownMemoryAdapter::new(memory_path.clone()).expect("memory file should load");
    let record = reader
        .get("alpha")
        .expect("memory read should succeed")
        .expect("alpha should be persisted");
    assert_eq!(record.value, "42");

    let log_path = workspace.join("runtime.log");
    let log = fs::read_to_string(&log_path).expect("tool log should exist");
    assert!(log.contains("intent=cli-1 kind=write key=alpha"));

    let _ = fs::remove_file(memory_path);
    let _ = fs::remove_file(log_path);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_status_and_health_are_minimal() {
    let status = run_cli(&["status"]);
    let health = run_cli(&["health"]);

    let status_stdout = stdout_of(&status);
    let health_stdout = stdout_of(&health);

    assert!(status.status.success());
    assert!(health.status.success());
    assert!(status_stdout.contains("status revision="));
    assert!(status_stdout.contains("status runtime provider_id="));
    assert!(health_stdout.contains("health ok=true"));
}

#[test]
fn e2e_cli_run_aliases_legacy_single_intent_commands() {
    let write = run_cli(&["run", "write:alpha=42"]);
    let freeze = run_cli(&["freeze"]);

    let write_stdout = stdout_of(&write);
    let freeze_stdout = stdout_of(&freeze);

    assert!(write.status.success());
    assert!(freeze.status.success());
    assert!(write_stdout.contains("intent id=cli-1 kind=write outcome=accepted"));
    assert!(freeze_stdout.contains("intent id=cli-1 kind=freeze"));
}

#[test]
fn e2e_cli_rejects_unknown_runtime_provider_from_env() {
    let output = run_cli_with_env(
        &["status"],
        &[("AXONRUNNER_RUNTIME_PROVIDER", "openrouter")],
        "invalid-provider-env",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout.is_empty());
    assert!(stderr.contains("runtime initialization error"));
    assert!(stderr.contains("unknown runtime provider 'openrouter'"));
    assert!(stderr.contains("AXONRUNNER_RUNTIME_PROVIDER"));
}

#[test]
fn e2e_cli_rejects_unknown_runtime_provider_from_config_file() {
    let config_path = unique_path("invalid-provider", "cfg");
    fs::write(&config_path, "provider=openrouter\n").expect("config file should be written");
    let config_arg = format!("--config-file={}", config_path.display());
    let output = run_cli(&[config_arg.as_str(), "status"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    let _ = fs::remove_file(&config_path);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout.is_empty());
    assert!(stderr.contains("runtime initialization error"));
    assert!(stderr.contains("unknown runtime provider 'openrouter'"));
}

#[test]
fn e2e_cli_run_uses_codek_provider_selection() {
    let output = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[
            ("AXONRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXONRUNNER_CODEX_BIN", "/definitely-missing-codex-binary"),
        ],
        "codek-provider-write",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(stdout.contains("intent id=cli-1 kind=write outcome=accepted"));
    assert!(stderr.contains("runtime_compose failed intent_id=cli-1 stage=provider"));
    assert!(stderr.contains("codex_runtime.connect"));
}

#[test]
fn e2e_cli_unknown_command_stderr_stable() {
    let output = run_cli(&["gateway"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout.is_empty());
    assert!(stderr.contains("unknown command 'gateway'"));
    assert!(stderr.contains(CLI_USAGE));
}
