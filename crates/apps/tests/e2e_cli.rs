use axonrunner_adapters::{MemoryAdapter, memory::MarkdownMemoryAdapter};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CLI_USAGE: &str = "\
usage:
  axonrunner_apps [global-options] <command> [command-args]

global-options:
  --config-file <path>
  --profile=<name>
  --provider=<id>
  --provider-model=<name>
  --workspace=<path>
  --state-path=<path>
  --command-allowlist=<cmds>
  --actor=<id>  (default: system)

commands:
  run <goal-file>
  status [run-id|latest]
  replay [run-id|latest]
  resume [run-id|latest]
  abort [run-id|latest]
  doctor [--json]
  health
  help

compatibility:
  batch [--reset-state] <intent-spec>...
  read <key>
  write <key> <value>
  remove <key>
  freeze
  halt

legacy intent-spec:
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
    "AXONRUNNER_RUNTIME_STATE_PATH",
    "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
    "AXONRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
    "AXONRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION",
    "AXONRUNNER_RUNTIME_TOOL_LOG_PATH",
    "AXONRUNNER_EXPERIMENTAL_OPENAI",
    "OPENAI_API_KEY",
];

fn run_cli(args: &[&str]) -> Output {
    run_cli_with_env(args, &[], "default")
}

fn run_cli_with_env(args: &[&str], env: &[(&str, &str)], label: &str) -> Output {
    run_cli_internal(args, env, label, true)
}

fn run_cli_without_runtime_defaults(args: &[&str], env: &[(&str, &str)], label: &str) -> Output {
    run_cli_internal(args, env, label, false)
}

fn run_cli_internal(
    args: &[&str],
    env: &[(&str, &str)],
    label: &str,
    inject_runtime_defaults: bool,
) -> Output {
    let home = unique_path(&format!("home-{label}"), "dir");
    fs::create_dir_all(&home).expect("isolated home directory should be writable");
    let canonical_home = fs::canonicalize(&home).unwrap_or(home.clone());
    let explicit_tool_workspace = env.iter().find_map(|(key, value)| {
        (*key == "AXONRUNNER_RUNTIME_TOOL_WORKSPACE").then(|| PathBuf::from(*value))
    });
    let explicit_artifact_workspace = env.iter().find_map(|(key, value)| {
        (*key == "AXONRUNNER_RUNTIME_ARTIFACT_WORKSPACE").then(|| PathBuf::from(*value))
    });
    let tool_workspace = explicit_tool_workspace
        .clone()
        .unwrap_or_else(|| canonical_home.join(".axonrunner").join("workspace"));
    let artifact_workspace = explicit_artifact_workspace
        .clone()
        .unwrap_or_else(|| tool_workspace.clone());

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_axonrunner_apps"));
    cmd.env("HOME", &canonical_home).args(args);
    for key in SANITIZED_ENV_KEYS {
        cmd.env_remove(key);
    }
    if inject_runtime_defaults && explicit_tool_workspace.is_none() {
        cmd.env("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", &tool_workspace);
    }
    if inject_runtime_defaults
        && !env
            .iter()
            .any(|(key, _)| *key == "AXONRUNNER_RUNTIME_TOOL_LOG_PATH")
    {
        cmd.env(
            "AXONRUNNER_RUNTIME_TOOL_LOG_PATH",
            artifact_workspace.join("runtime.log"),
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

fn stdout_lines(output: &Output) -> Vec<String> {
    stdout_of(output)
        .lines()
        .map(|line| line.to_owned())
        .collect()
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

fn fake_cli_script(label: &str, stdout: &str) -> PathBuf {
    let path = unique_path(label, "sh");
    fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", stdout))
        .expect("fake cli should be written");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&path)
            .expect("metadata should exist")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("permissions should be updated");
    }
    path
}

fn run_checked_command(program: &str, args: &[&str], cwd: &Path) {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("command should run");
    assert!(
        output.status.success(),
        "command failed: {} {:?}\nstdout:\n{}\nstderr:\n{}",
        program,
        args,
        stdout_of(&output),
        stderr_of(&output)
    );
}

fn init_git_repo(path: &Path) {
    fs::create_dir_all(path).expect("repo directory should exist");
    run_checked_command("git", &["init"], path);
    run_checked_command("git", &["config", "user.email", "test@example.com"], path);
    run_checked_command("git", &["config", "user.name", "AxonRunner Test"], path);
    fs::write(path.join("README.md"), "fixture\n").expect("fixture file should exist");
    run_checked_command("git", &["add", "README.md"], path);
    run_checked_command("git", &["commit", "-m", "init"], path);
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
    assert!(stdout.contains("run intent_id=cli-1 phase=completed outcome=success"));
    assert!(stdout.contains("query intent_id=cli-2 key=alpha value=42"));
    assert!(stdout.contains("batch completed count=6"));
}

#[test]
fn e2e_cli_can_separate_execution_and_artifact_workspaces() {
    let workspace = unique_path("artifact-separation-workspace", "dir");
    let artifact_workspace = unique_path("artifact-separation-artifacts", "dir");
    let output = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            (
                "AXONRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
                path_str(&artifact_workspace),
            ),
        ],
        "artifact-separation",
    );
    let stderr = stderr_of(&output);
    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            (
                "AXONRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
                path_str(&artifact_workspace),
            ),
        ],
        "artifact-separation-replay",
    );

    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    assert!(artifact_workspace.join("runtime.log").exists());
    assert!(
        artifact_workspace
            .join(".axonrunner/trace/events.jsonl")
            .exists()
    );
    assert!(
        artifact_workspace
            .join(".axonrunner/artifacts/cli-1.report.md")
            .exists()
    );
    assert!(!workspace.join("runtime.log").exists());
    assert!(!workspace.join(".axonrunner/trace/events.jsonl").exists());
    assert!(
        !workspace
            .join(".axonrunner/artifacts/cli-1.report.md")
            .exists()
    );
    assert!(stdout_of(&replay).contains("replay artifact_index count=1"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_dir_all(artifact_workspace);
}

#[test]
fn e2e_cli_goal_run_can_use_isolated_git_worktree() {
    let repo_root = unique_path("git-worktree-repo", "dir");
    let artifact_workspace = unique_path("git-worktree-artifacts", "dir");
    let goal_file = unique_path("git-worktree-goal", "json");
    init_git_repo(&repo_root);
    fs::write(
        &goal_file,
        r#"{
  "summary": "Verify isolated workspace binding",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "pwd", "detail": "pwd" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&repo_root)),
            (
                "AXONRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
                path_str(&artifact_workspace),
            ),
            ("AXONRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION", "1"),
        ],
        "git-worktree-run",
    );
    let stderr = stderr_of(&run);

    assert!(run.status.success(), "stderr:\n{stderr}");
    let commands_dir = artifact_workspace.join(".axonrunner/commands");
    let command_artifact_path = fs::read_dir(&commands_dir)
        .expect("command artifacts should exist")
        .map(|entry| entry.expect("entry should exist").path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .expect("command artifact json should exist");
    let command_artifact =
        fs::read_to_string(&command_artifact_path).expect("command artifact should be readable");
    let command_json: serde_json::Value =
        serde_json::from_str(&command_artifact).expect("artifact should be valid json");
    let pwd_stdout = command_json["stdout"]
        .as_str()
        .expect("stdout should be present")
        .trim();

    assert_ne!(pwd_stdout, repo_root.display().to_string());
    assert!(pwd_stdout.contains("run-1"));
    assert!(stdout_of(&run).contains("phase=completed outcome=success"));

    let _ = fs::remove_dir_all(repo_root);
    let _ = fs::remove_dir_all(artifact_workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_failed_isolated_run_writes_rollback_metadata() {
    let repo_root = unique_path("rollback-repo", "dir");
    let artifact_workspace = unique_path("rollback-artifacts", "dir");
    init_git_repo(&repo_root);
    let canonical_repo_root = fs::canonicalize(&repo_root).unwrap_or(repo_root.clone());

    let run = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&repo_root)),
            (
                "AXONRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
                path_str(&artifact_workspace),
            ),
            ("AXONRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION", "1"),
            ("AXONRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXONRUNNER_CODEX_BIN", "/definitely-missing-codex-binary"),
        ],
        "rollback-run",
    );
    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&repo_root)),
            (
                "AXONRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
                path_str(&artifact_workspace),
            ),
            ("AXONRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION", "1"),
        ],
        "rollback-replay",
    );

    assert_eq!(run.status.code(), Some(6));
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    let rollback_path = artifact_workspace.join(".axonrunner/artifacts/cli-1.rollback.json");
    assert!(rollback_path.exists(), "rollback metadata should exist");
    let report_path = artifact_workspace.join(".axonrunner/artifacts/cli-1.report.md");
    let report = fs::read_to_string(&report_path).expect("rollback report should be readable");
    let rollback_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&rollback_path).expect("rollback metadata should be readable"),
    )
    .expect("rollback metadata should be valid json");
    assert_eq!(rollback_json["schema"], "axonrunner.rollback.v1");
    assert_eq!(
        rollback_json["restore_path"],
        canonical_repo_root.display().to_string()
    );
    assert!(
        rollback_json["cleanup_path"]
            .as_str()
            .expect("cleanup path should exist")
            .contains("run-1")
    );
    let replay_stdout = stdout_of(&replay);
    assert!(replay_stdout.contains("replay rollback metadata="));
    assert!(replay_stdout.contains(canonical_repo_root.display().to_string().as_str()));
    assert!(report.contains("rollback=metadata="));
    assert!(report.contains("cli-1.rollback.json"));
    assert!(report.contains("restore_path="));

    let _ = fs::remove_dir_all(repo_root);
    let _ = fs::remove_dir_all(artifact_workspace);
}

#[test]
fn golden_provider_boundary_blocks_missing_codek_binary() {
    let workspace = unique_path("golden-codek-blocked-workspace", "dir");
    let output = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[
            ("AXONRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXONRUNNER_CODEX_BIN", "/definitely-missing-codex-binary"),
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
        ],
        "golden-codek-blocked",
    );
    let stderr = stderr_of(&output);

    assert_eq!(output.status.code(), Some(6));
    assert!(stderr.contains("runtime execution failed intent_id=cli-1 stage=provider"));
    assert!(workspace.join(".axonrunner/trace/events.jsonl").exists());
    assert!(
        workspace
            .join(".axonrunner/artifacts/cli-1.report.md")
            .exists()
    );

    let _ = fs::remove_dir_all(workspace);
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
    let recall = reader
        .get("recall:last_run/cli-1")
        .expect("recall read should succeed")
        .expect("recall summary should be persisted");
    assert_eq!(record.value, "42");
    assert!(recall.value.contains("run_id=run-1"));
    assert!(recall.value.contains("outcome=success"));
    assert!(
        recall
            .value
            .contains(".axonrunner/artifacts/cli-1.report.md")
    );
    assert!(recall.value.contains("changed_paths="));

    let log_path = workspace.join("runtime.log");
    let log = fs::read_to_string(&log_path).expect("tool log should exist");
    assert!(log.contains("intent=cli-1 kind=write key=alpha"));
    let report = fs::read_to_string(workspace.join(".axonrunner/artifacts/cli-1.report.md"))
        .expect("report artifact should exist");
    let plan = fs::read_to_string(workspace.join(".axonrunner/artifacts/cli-1.plan.md"))
        .expect("plan artifact should exist");
    let verify = fs::read_to_string(workspace.join(".axonrunner/artifacts/cli-1.verify.md"))
        .expect("verify artifact should exist");
    let trace = fs::read_to_string(workspace.join(".axonrunner/trace/events.jsonl"))
        .expect("trace event log should exist");
    assert!(report.contains("kind=write"));
    assert!(report.contains("run_phase=completed"));
    assert!(report.contains("run_outcome=success"));
    assert!(report.contains("run_reason_code=verification_passed"));
    assert!(report.contains("run_reason_detail=none"));
    assert!(report.contains("outcome=accepted"));
    assert!(plan.contains("policy=allowed"));
    assert!(plan.contains("planned_steps=4"));
    assert!(verify.contains("first_failure=none"));
    assert!(verify.contains("status=passed"));
    assert!(verify.contains("repair_status=skipped"));
    assert!(verify.contains("state_fact=alpha:42"));
    assert!(verify.contains("changed_paths="));
    let trace_event: serde_json::Value =
        serde_json::from_str(trace.lines().last().expect("trace line should exist"))
            .expect("trace line should be valid json");
    assert_eq!(trace_event["intent_id"], "cli-1");
    assert_eq!(trace_event["tool"], "applied");
    assert_eq!(trace_event["report_written"], true);
    assert_eq!(trace_event["verification"]["status"], "passed");
    assert_eq!(trace_event["run"]["run_id"], "run-1");
    assert_eq!(trace_event["run"]["phase"], "completed");
    assert_eq!(trace_event["run"]["outcome"], "success");
    assert_eq!(trace_event["run"]["planned_steps"], 4);
    assert_eq!(trace_event["run"]["step_ids"][0], "run-1/step-1-planning");
    assert!(trace_event["patch_artifacts"].as_array().is_some());
    assert!(
        trace_event["patch_artifacts"]
            .as_array()
            .expect("patch artifacts should be an array")
            .len()
            >= 5
    );

    let _ = fs::remove_file(memory_path);
    let _ = fs::remove_file(log_path);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_replay_latest_summarizes_recent_trace() {
    let workspace = unique_path("replay-latest-workspace", "dir");
    let output = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "replay-latest-write",
    );
    assert!(output.status.success());

    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "replay-latest-read",
    );
    let stdout = stdout_of(&replay);
    let stderr = stderr_of(&replay);

    assert!(replay.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("replay intent_id=cli-1 count=1"));
    assert!(stdout.contains("kind=write"));
    assert!(stdout.contains("policy=allowed"));
    assert!(stdout.contains("replay verification status=passed"));
    assert!(stdout.contains("replay run run_id=run-1 phase=completed outcome=success"));
    assert!(stdout.contains("replay repair attempted=false status=skipped"));
    assert!(stdout.contains("run-1/step-1-planning"));
    assert!(stdout.contains("replay step id="));
    assert!(stdout.contains("phase=verifying"));
    assert!(stdout.contains("artifacts"));
    assert!(stdout.contains("replay artifact_index count=1"));
    assert!(stdout.contains("replay patch target="));
    assert!(stdout.contains("before="));
    assert!(stdout.contains("after="));
    assert!(stdout.contains("replay patch after_excerpt="));
    assert!(
        stdout.contains(
            "replay summary failed_intents=0 false_success_intents=0 false_done_intents=0"
        )
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_status_and_health_are_minimal() {
    let workspace = unique_path("status-run-workspace", "dir");
    let _run = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "status-run-write",
    );
    let status = run_cli_with_env(
        &["status"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "status-run-status",
    );
    let health = run_cli(&["health"]);

    let status_stdout = stdout_of(&status);
    let health_stdout = stdout_of(&health);

    assert!(status.status.success());
    assert!(health.status.success());
    assert!(status_stdout.contains("status revision="));
    assert!(status_stdout.contains("status runtime provider_id="));
    assert!(status_stdout.contains("status run run_id=run-1 phase=completed outcome=success"));
    assert!(status_stdout.contains("provider_state=ready"));
    assert!(health_stdout.contains("health ok=true"));
    assert!(health_stdout.contains("provider_state=ready"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_goal_file_run_persists_run_id_and_supports_status_and_replay_by_run_id() {
    let workspace = unique_path("goal-file-workspace", "dir");
    let goal_file = unique_path("goal-file", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Ship one bounded goal package",
  "workspace_root": "/workspace",
  "constraints": [
    { "label": "non-goal", "detail": "no multi-agent orchestration" }
  ],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axonrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-run",
    );
    let run_stdout = stdout_of(&run);
    let run_stderr = stderr_of(&run);

    assert!(run.status.success(), "stderr:\n{run_stderr}");
    assert!(run_stdout.contains("intent id=cli-1 kind=goal outcome=accepted"));
    assert!(run_stdout.contains(
        "run intent_id=cli-1 phase=completed outcome=success reason=verification_passed"
    ));

    let status = run_cli_with_env(
        &["status", "run-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-replay",
    );

    assert!(status.status.success());
    assert!(replay.status.success());
    assert!(stdout_of(&status).contains("status run run_id=run-1 phase=completed outcome=success"));
    assert!(stdout_of(&replay).contains("replay run run_id=run-1 phase=completed outcome=success"));
    assert!(
        stdout_of(&replay)
            .contains("replay verification status=passed summary=goal_done_conditions_verified")
    );
    assert!(
        stdout_of(&replay).contains("replay stages provider=skipped memory=skipped tool=applied")
    );
    assert!(stdout_of(&replay).contains("replay step id="));
    assert!(stdout_of(&replay).contains("label=validate goal contract"));
    assert!(
        fs::read_to_string(workspace.join(".axonrunner/artifacts/cli-1.plan.md"))
            .expect("plan artifact should exist")
            .contains("goal=Ship one bounded goal package")
    );

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_goal_file_on_risk_requires_approval_before_execution() {
    let workspace = unique_path("goal-file-on-risk-workspace", "dir");
    let state_path = unique_path("goal-file-on-risk-state", "snapshot");
    let goal_file = unique_path("goal-file-on-risk", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Wait for on-risk approval before execution",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axonrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "on-risk"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-on-risk-run",
    );
    assert!(run.status.success());
    assert!(stdout_of(&run).contains(
        "run intent_id=cli-1 phase=waiting_approval outcome=approval_required reason=approval_required_before_execution"
    ));
    let pending_report = fs::read_to_string(workspace.join(".axonrunner/artifacts/cli-1.report.md"))
        .expect("pending approval report should exist");
    let pending_trace = fs::read_to_string(workspace.join(".axonrunner/trace/events.jsonl"))
        .expect("pending approval trace should exist");
    let pending_event: serde_json::Value = serde_json::from_str(
        pending_trace
            .lines()
            .last()
            .expect("pending trace line should exist"),
    )
    .expect("pending trace line should be valid json");
    assert!(pending_report.contains("provider=skipped"));
    assert!(pending_report.contains("tool=skipped"));
    assert_eq!(pending_event["provider"], "skipped");
    assert_eq!(pending_event["tool"], "skipped");

    let resume = run_cli_with_env(
        &["resume", "run-1"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-on-risk-resume",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-on-risk-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-on-risk-replay",
    );

    assert!(resume.status.success());
    assert!(stdout_of(&resume).contains(
        "resume run_id=run-1 phase=completed outcome=success reason=verification_passed"
    ));
    assert!(status.status.success());
    assert!(stdout_of(&status).contains("status run run_id=run-1 phase=completed outcome=success"));
    assert!(replay.status.success());
    assert!(stdout_of(&replay).contains("replay run run_id=run-1 phase=completed outcome=success"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(state_path);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_goal_file_approval_can_resume_from_pending_run() {
    let workspace = unique_path("goal-file-approval-workspace", "dir");
    let state_path = unique_path("goal-file-approval-state", "snapshot");
    let goal_file = unique_path("goal-file-approval", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Wait for approval before execution",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axonrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "always"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-approval-run",
    );
    let pending_status = run_cli_with_env(
        &["status", "run-1"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-approval-pending-status",
    );
    let pending_doctor = run_cli_with_env(
        &["doctor"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-approval-pending-doctor",
    );
    assert!(run.status.success());
    assert!(pending_status.status.success());
    assert!(pending_doctor.status.success());
    assert!(stdout_of(&run).contains(
        "run intent_id=cli-1 phase=waiting_approval outcome=approval_required reason=approval_required_before_execution"
    ));
    assert!(stdout_of(&pending_status).contains("status pending_run run_id=run-1"));
    assert!(stdout_of(&pending_status).contains("approval_state=required verifier_state=skipped"));
    assert!(stdout_of(&pending_doctor).contains("doctor pending_run run_id=run-1"));
    assert!(stdout_of(&pending_doctor).contains("approval_state=required verifier_state=skipped"));
    let pending_replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-approval-pending-replay",
    );
    assert!(pending_replay.status.success());
    assert!(
        stdout_of(&pending_replay)
            .contains("approval_state=required verifier_state=skipped")
    );

    let resume = run_cli_with_env(
        &["resume", "run-1"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-approval-resume",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-approval-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-approval-replay",
    );

    assert!(resume.status.success());
    assert!(stdout_of(&resume).contains(
        "resume run_id=run-1 phase=completed outcome=success reason=verification_passed"
    ));
    assert!(status.status.success());
    assert!(stdout_of(&status).contains("status run run_id=run-1 phase=completed outcome=success"));
    assert!(replay.status.success());
    assert!(stdout_of(&replay).contains("replay run run_id=run-1 phase=completed outcome=success"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(state_path);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_goal_file_pending_run_can_abort_cleanly() {
    let workspace = unique_path("goal-file-abort-workspace", "dir");
    let state_path = unique_path("goal-file-abort-state", "snapshot");
    let goal_file = unique_path("goal-file-abort", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Abort pending approval run",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axonrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "always"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-abort-run",
    );
    assert!(run.status.success());

    let abort = run_cli_with_env(
        &["abort", "run-1"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-abort-command",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-abort-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-abort-replay",
    );
    let trace = fs::read_to_string(workspace.join(".axonrunner/trace/events.jsonl"))
        .expect("trace should be readable");
    let latest = trace.lines().last().expect("trace should contain latest event");
    let latest: serde_json::Value =
        serde_json::from_str(latest).expect("latest trace event should parse");

    assert!(abort.status.success());
    assert!(
        stdout_of(&abort)
            .contains("abort run_id=run-1 phase=aborted outcome=aborted reason=operator_abort")
    );
    assert!(status.status.success());
    assert!(stdout_of(&status).contains("status run run_id=run-1 phase=aborted outcome=aborted"));
    assert!(replay.status.success());
    assert!(stdout_of(&replay).contains("replay run run_id=run-1 phase=aborted outcome=aborted"));
    assert_eq!(latest["provider"], "skipped");
    assert_eq!(latest["memory"], "skipped");
    assert_eq!(latest["tool"], "skipped");
    assert_eq!(
        latest["verification"]["summary"],
        serde_json::Value::String(String::from("operator_abort"))
    );

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(state_path);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_goal_file_blocks_when_step_budget_is_already_exhausted() {
    let workspace = unique_path("goal-file-budget-workspace", "dir");
    let goal_file = unique_path("goal-file-budget", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Budget exhaustion before execution",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axonrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 1, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-budget-run",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-budget-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-budget-replay",
    );

    assert!(run.status.success());
    assert!(stdout_of(&run).contains(
        "run intent_id=cli-1 phase=blocked outcome=budget_exhausted reason=budget_exhausted_before_execution"
    ));
    assert!(status.status.success());
    assert!(
        stdout_of(&status)
            .contains("status run run_id=run-1 phase=blocked outcome=budget_exhausted")
    );
    assert!(replay.status.success());
    assert!(
        stdout_of(&replay)
            .contains("replay run run_id=run-1 phase=blocked outcome=budget_exhausted")
    );
    assert!(
        stdout_of(&replay).contains(
            "reason_code=budget_exhausted_before_execution reason_detail=none"
        )
    );
    let report = fs::read_to_string(workspace.join(".axonrunner/artifacts/cli-1.report.md"))
        .expect("budget report should exist");
    assert!(report.contains("provider=skipped"));
    assert!(report.contains("tool=skipped"));
    assert!(report.contains("run_reason_code=budget_exhausted_before_execution"));
    assert!(report.contains("run_reason_detail=none"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_goal_file_uses_bounded_repair_budget() {
    let workspace = unique_path("goal-file-repair-budget-workspace", "dir");
    let goal_file = unique_path("goal-file-repair-budget", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Repair budget exhaustion after verifier failure",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axonrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 6, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_COMMAND_ALLOWLIST", "git"),
        ],
        "goal-file-repair-budget-run",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-repair-budget-replay",
    );

    assert!(run.status.success());
    assert!(stdout_of(&run).contains("phase=blocked outcome=budget_exhausted"));
    assert!(stdout_of(&run).contains("repair_budget_exhausted:attempts=1/1"));
    assert!(replay.status.success());
    assert!(
        stdout_of(&replay)
            .contains("replay run run_id=run-1 phase=blocked outcome=budget_exhausted")
    );
    assert!(
        stdout_of(&replay)
            .contains("replay repair attempted=true attempts=1 status=budget_exhausted")
    );
    assert!(stdout_of(&replay).contains("repair_budget_exhausted:attempts=1/1"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_workspace_lock_blocks_mutating_commands_but_allows_status_reads() {
    let workspace = unique_path("workspace-lock-workspace", "dir");
    fs::create_dir_all(workspace.join(".axonrunner")).expect("lock dir should exist");
    fs::write(
        workspace.join(".axonrunner/runtime.lock"),
        format!("pid={} command=run\n", std::process::id()),
    )
    .expect("lock file should exist");

    let goal_file = unique_path("workspace-lock-goal", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Blocked by single writer lock",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axonrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "workspace-lock-run",
    );
    let status = run_cli_with_env(
        &["status"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "workspace-lock-status",
    );

    assert_eq!(run.status.code(), Some(6));
    assert!(stderr_of(&run).contains("workspace lock is active"));
    assert!(status.status.success());
    assert!(stdout_of(&status).contains("status revision=0 mode=active facts=0 denied=0 audit=0"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_workspace_lock_recovers_stale_pid_and_runs() {
    let workspace = unique_path("workspace-stale-lock-workspace", "dir");
    fs::create_dir_all(workspace.join(".axonrunner")).expect("lock dir should exist");
    fs::write(
        workspace.join(".axonrunner/runtime.lock"),
        "pid=999999 command=run\n",
    )
    .expect("stale lock file should exist");

    let goal_file = unique_path("workspace-stale-lock-goal", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Recover stale single writer lock",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axonrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "workspace-stale-lock-run",
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(stdout_of(&run).contains("phase=completed outcome=success"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_rejected_control_action_surfaces_approval_required_outcome() {
    let workspace = unique_path("approval-required-workspace", "dir");
    let state_path = unique_path("approval-required-state", "snapshot");
    let run = run_cli_with_env(
        &["--actor=alice", "freeze"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "approval-required-run",
    );
    let status = run_cli_with_env(
        &["status"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "approval-required-status",
    );
    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "approval-required-replay",
    );
    assert!(run.status.success());
    assert!(status.status.success());
    assert!(replay.status.success());
    assert!(
        stdout_of(&run)
            .contains("run intent_id=cli-1 phase=waiting_approval outcome=approval_required")
    );
    assert!(
        stdout_of(&status)
            .contains("status run run_id=run-1 phase=waiting_approval outcome=approval_required")
    );
    assert!(
        stdout_of(&replay)
            .contains("replay run run_id=run-1 phase=waiting_approval outcome=approval_required")
    );

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(state_path);
}

#[test]
fn e2e_cli_resume_rejects_non_pending_state_with_clear_error() {
    let workspace = unique_path("resume-no-pending-workspace", "dir");
    let state_path = unique_path("resume-no-pending-state", "snapshot");
    let resume = run_cli_with_env(
        &["resume", "latest"],
        &[
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "resume-no-pending",
    );

    assert!(!resume.status.success());
    assert!(
        stderr_of(&resume).contains(
            "resume only supports pending goal-file approval runs; no pending approval run is available"
        )
    );

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(state_path);
}

#[test]
fn e2e_cli_run_missing_goal_file_surfaces_file_not_found_directly() {
    let workspace = unique_path("missing-goal-workspace", "dir");
    let output = run_cli_with_env(
        &["run", "./missing-goal.json"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "missing-goal",
    );

    assert!(!output.status.success());
    assert!(stderr_of(&output).contains("goal file not found: ./missing-goal.json"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_goal_file_executes_external_workflow_pack_verifier_sequence() {
    let workspace = unique_path("goal-pack-workspace", "dir");
    let goal_file = unique_path("goal-pack-goal", "json");
    let pack_file = unique_path("goal-pack-manifest", "json");
    fs::write(
        &pack_file,
        r#"{
  "pack_id": "custom-pack",
  "version": "1",
  "description": "custom verifier pack",
  "entry_goal": "goal",
  "planner_hints": ["prefer ordered verifier flow"],
  "recommended_verifier_flow": ["lint", "build"],
  "allowed_tools": [{"operation": "run_command", "scope": "workspace"}],
  "verifier_rules": [
    {
      "label": "build-pass",
      "profile": "build",
      "command_example": "pwd",
      "artifact_expectation": "build path exists",
      "required": true
    },
    {
      "label": "lint-pass",
      "profile": "lint",
      "command_example": "pwd",
      "artifact_expectation": "lint path exists",
      "required": true
    }
  ],
  "risk_policy": {"approval_mode": "never", "max_mutating_steps": 5}
}"#,
    )
    .expect("pack file should be written");
    fs::write(
        &goal_file,
        format!(
            r#"{{
  "summary": "Use external workflow pack",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    {{ "label": "report", "evidence": "report artifact exists" }}
  ],
  "verification_checks": [
    {{ "label": "placeholder", "detail": "pwd" }}
  ],
  "budget": {{ "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 }},
  "approval_mode": "never",
  "workflow_pack": "{}"
}}"#,
            pack_file.display()
        ),
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-pack-run",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-pack-replay",
    );
    let report = fs::read_to_string(workspace.join(".axonrunner/artifacts/cli-1.report.md"))
        .expect("report should exist");

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    assert!(stdout_of(&run).contains("phase=completed outcome=success"));
    let lint_idx = report.find("verifier=lint-pass").expect("lint verifier should exist");
    let build_idx = report.find("verifier=build-pass").expect("build verifier should exist");
    assert!(lint_idx < build_idx, "recommended verifier flow should drive execution order");

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
    let _ = fs::remove_file(pack_file);
}

#[test]
fn e2e_cli_help_matches_supported_surface() {
    let long = run_cli(&["--help"]);
    let short = run_cli(&["help"]);

    let long_stdout = stdout_of(&long);
    let short_stdout = stdout_of(&short);
    let long_stderr = stderr_of(&long);
    let short_stderr = stderr_of(&short);

    assert!(long.status.success());
    assert!(short.status.success());
    assert_eq!(long_stdout, format!("{CLI_USAGE}\n"));
    assert_eq!(short_stdout, format!("{CLI_USAGE}\n"));
    assert!(long_stderr.is_empty());
    assert!(short_stderr.is_empty());
}

#[test]
fn e2e_cli_doctor_reports_blocked_codek_binary_and_paths() {
    let workspace = unique_path("doctor-codek-workspace", "dir");
    let state_path = unique_path("doctor-codek-state", "snapshot");
    let output = run_cli_with_env(
        &["doctor"],
        &[
            ("AXONRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXONRUNNER_CODEX_BIN", "/definitely-missing-codex-binary"),
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "doctor-codek",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("doctor ok=false"));
    assert!(stdout.contains("provider_id=codek"));
    assert!(stdout.contains("provider_state=blocked"));
    assert!(stdout.contains("reason=binary_not_found"));
    assert!(stdout.contains("doctor paths"));
    assert!(stdout.contains("doctor check name=provider_probe state=fail"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(state_path);
}

#[test]
fn e2e_cli_doctor_reports_codex_version_and_compatibility_for_old_binary() {
    let workspace = unique_path("doctor-old-codek-workspace", "dir");
    let state_path = unique_path("doctor-old-codek-state", "snapshot");
    let fake_cli = fake_cli_script("doctor-old-codek", "codex 0.103.9");
    let output = run_cli_with_env(
        &["doctor"],
        &[
            ("AXONRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXONRUNNER_CODEX_BIN", path_str(&fake_cli)),
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "doctor-old-codek",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(stdout.contains("doctor ok=false"));
    assert!(stdout.contains("provider_state=blocked"));
    assert!(stdout.contains("version=codex_0.103.9") || stdout.contains("version=codex 0.103.9"));
    assert!(stdout.contains("compatibility=blocked"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(state_path);
    let _ = fs::remove_file(fake_cli);
}

#[test]
fn e2e_cli_doctor_json_is_machine_readable() {
    let workspace = unique_path("doctor-json-workspace", "dir");
    let output = run_cli_with_env(
        &["doctor", "--json"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "doctor-json",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());

    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("doctor json should be valid");
    assert_eq!(json["provider_id"], "mock-local");
    assert_eq!(json["runtime"]["provider_state"], "ready");
    assert!(json["runtime"]["async_host_detail"].as_str().is_some());
    assert_eq!(json["checks"][0]["name"], "workspace_dir");
    assert!(json["paths"]["trace_events_path"].as_str().is_some());

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn golden_truth_surface_doctor_text_contract_for_ready_mock_local_is_stable() {
    let workspace = unique_path("doctor-golden-workspace", "dir");
    let state_path = unique_path("doctor-golden-state", "snapshot");
    let memory_path = unique_path("doctor-golden-memory", "md");

    let run = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[
            ("AXONRUNNER_RUNTIME_MEMORY_PATH", path_str(&memory_path)),
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "doctor-golden-write",
    );
    assert!(run.status.success());

    let doctor = run_cli_with_env(
        &["doctor"],
        &[
            ("AXONRUNNER_RUNTIME_MEMORY_PATH", path_str(&memory_path)),
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "doctor-golden-read",
    );
    let stderr = stderr_of(&doctor);
    assert!(doctor.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    let canonical_workspace = fs::canonicalize(&workspace).unwrap_or(workspace.clone());

    let expected = vec![
        String::from(
            "doctor ok=true profile=prod provider_id=mock-local provider_model=mock-local provider_experimental=false",
        ),
        String::from("doctor state revision=4 mode=active facts=1 denied=0 audit=1"),
        String::from("doctor runtime provider_state=ready memory_state=ready tool_state=ready"),
        format!(
            "doctor detail provider_detail=provider=mock-local memory_detail=path={} tool_detail=workspace={}",
            memory_path.display(),
            canonical_workspace.display()
        ),
        String::from(
            "doctor async_host detail=init_mode=configured,worker_threads=2,max_in_flight=8,timeout_ms=none",
        ),
        format!(
            "doctor paths workspace={} state_path={} trace_events_path={} tool_log_path={}",
            workspace.display(),
            state_path.display(),
            workspace.join(".axonrunner/trace/events.jsonl").display(),
            workspace.join("runtime.log").display()
        ),
        format!(
            "doctor check name=workspace_dir state=ok detail=exists,path={}",
            workspace.display()
        ),
        format!(
            "doctor check name=state_parent_dir state=ok detail=exists,path={}",
            state_path
                .parent()
                .expect("state parent should exist")
                .display()
        ),
        format!(
            "doctor check name=trace_parent_dir state=ok detail=exists,path={}",
            workspace.join(".axonrunner/trace").display()
        ),
        format!(
            "doctor check name=state_snapshot state=ok detail=present,path={}",
            state_path.display()
        ),
        format!(
            "doctor check name=trace_log state=ok detail=events=1,path={}",
            workspace.join(".axonrunner/trace/events.jsonl").display()
        ),
        String::from("doctor check name=provider_probe state=ok detail=provider=mock-local"),
    ];
    assert_eq!(stdout_lines(&doctor), expected);

    let _ = fs::remove_file(memory_path);
    let _ = fs::remove_file(state_path);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_read_runs_through_canonical_query_path_and_persisted_state() {
    let state_path = unique_path("state-read-canonical", "snapshot");

    let write = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path))],
        "read-canonical-write",
    );
    let read = run_cli_with_env(
        &["run", "read:alpha"],
        &[("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path))],
        "read-canonical-read",
    );

    let write_stdout = stdout_of(&write);
    let read_stdout = stdout_of(&read);

    assert!(write.status.success());
    assert!(read.status.success());
    assert!(write_stdout.contains("intent id=cli-1 kind=write outcome=accepted"));
    assert!(read_stdout.contains("intent id=cli-2 kind=read outcome=accepted"));
    assert!(read_stdout.contains("query intent_id=cli-2 key=alpha value=42"));

    let _ = fs::remove_file(state_path);
}

#[test]
fn golden_persisted_control_state_freeze_blocks_later_write() {
    let state_path = unique_path("golden-freeze-state", "snapshot");

    let freeze = run_cli_with_env(
        &["run", "freeze"],
        &[("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path))],
        "golden-freeze-write",
    );
    let write = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path))],
        "golden-freeze-read",
    );

    let freeze_stdout = stdout_of(&freeze);
    let write_stdout = stdout_of(&write);

    assert!(freeze.status.success());
    assert!(write.status.success());
    assert!(freeze_stdout.contains("kind=freeze outcome=accepted"));
    assert!(write_stdout.contains("kind=write outcome=rejected"));
    assert!(write_stdout.contains("policy=readonly_mutation"));

    let _ = fs::remove_file(state_path);
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

    assert_eq!(output.status.code(), Some(5));
    assert!(stdout.is_empty());
    assert!(stderr.contains("runtime init error:"));
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

    assert_eq!(output.status.code(), Some(5));
    assert!(stdout.is_empty());
    assert!(stderr.contains("runtime init error:"));
    assert!(stderr.contains("unknown runtime provider 'openrouter'"));
}

#[test]
fn golden_workspace_contract_blocks_runtime_when_workspace_is_undetermined() {
    let output = run_cli_without_runtime_defaults(&["status"], &[], "workspace-missing");
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(output.status.code(), Some(5));
    assert!(stdout.is_empty());
    assert!(stderr.contains("runtime init error:"));
    assert!(stderr.contains("runtime tool workspace is not configured"));
    assert!(stderr.contains("--workspace"));
}

#[test]
fn e2e_cli_run_uses_codek_provider_selection() {
    let memory_path = unique_path("codek-provider-memory", "md");
    let workspace = unique_path("codek-provider-workspace", "dir");
    let output = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[
            ("AXONRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXONRUNNER_CODEX_BIN", "/definitely-missing-codex-binary"),
            ("AXONRUNNER_RUNTIME_MEMORY_PATH", path_str(&memory_path)),
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
        ],
        "codek-provider-write",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(output.status.code(), Some(6), "stderr:\n{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("runtime execution error:"));
    assert!(stderr.contains("runtime execution failed intent_id=cli-1 stage=provider"));
    assert!(stderr.contains("codex_runtime.connect"));
    let failed_report = fs::read_to_string(workspace.join(".axonrunner/artifacts/cli-1.report.md"))
        .expect("failed run report artifact should exist");
    let failed_trace = fs::read_to_string(workspace.join(".axonrunner/trace/events.jsonl"))
        .expect("failed run trace event log should exist");
    assert!(failed_report.contains("tool=skipped"));
    assert!(failed_report.contains("outcome=accepted"));
    assert!(failed_report.contains("run_phase=blocked"));
    assert!(failed_report.contains("run_outcome=blocked"));
    assert!(failed_report.contains("provider_health_state=blocked"));
    let failed_event: serde_json::Value = serde_json::from_str(
        failed_trace
            .lines()
            .last()
            .expect("trace line should exist"),
    )
    .expect("trace line should be valid json");
    assert_eq!(failed_event["intent_id"], "cli-1");
    assert_eq!(failed_event["provider"], "failed");
    assert_eq!(failed_event["report_written"], true);
    assert_eq!(failed_event["first_failure"]["stage"], "provider");
    assert_eq!(failed_event["verification"]["status"], "failed");
    assert_eq!(failed_event["run"]["run_id"], "run-1");
    assert_eq!(failed_event["run"]["phase"], "blocked");
    assert_eq!(failed_event["run"]["outcome"], "blocked");
    assert!(
        failed_event["patch_artifacts"]
            .as_array()
            .expect("patch artifacts should be an array")
            .len()
            >= 4
    );

    let reader = MarkdownMemoryAdapter::new(memory_path.clone()).expect("memory file should load");
    assert!(
        reader
            .get("alpha")
            .expect("memory read should succeed")
            .is_none(),
        "memory write should be skipped on provider failure"
    );
    assert!(
        !workspace.join("runtime.log").exists(),
        "tool write should be skipped on provider failure"
    );

    let _ = fs::remove_file(memory_path);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_replay_specific_intent_reports_failure_boundary() {
    let workspace = unique_path("replay-failure-workspace", "dir");
    let output = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[
            ("AXONRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXONRUNNER_CODEX_BIN", "/definitely-missing-codex-binary"),
            ("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
        ],
        "replay-failure-write",
    );
    assert_eq!(output.status.code(), Some(6));

    let replay = run_cli_with_env(
        &["replay", "cli-1"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "replay-failure-read",
    );
    let stdout = stdout_of(&replay);
    let stderr = stderr_of(&replay);

    assert!(replay.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("replay intent_id=cli-1 count=1"));
    assert!(stdout.contains("replay failure stage=provider"));
    assert!(stdout.contains("replay verification status=failed"));
    assert!(stdout.contains("replay run run_id=run-1 phase=blocked outcome=blocked"));
    assert!(stdout.contains("replay artifact_index count=1"));
    assert!(stdout.contains("replay patch target="));
    assert!(stdout.contains("before="));
    assert!(stdout.contains("after="));
    assert!(
        stdout.contains(
            "replay summary failed_intents=1 false_success_intents=1 false_done_intents=0"
        )
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_replay_missing_target_is_runtime_error() {
    let workspace = unique_path("replay-missing-workspace", "dir");
    fs::create_dir_all(&workspace).expect("workspace should exist");

    let replay = run_cli_with_env(
        &["replay", "missing-intent"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "replay-missing-read",
    );
    let stdout = stdout_of(&replay);
    let stderr = stderr_of(&replay);

    assert_eq!(replay.status.code(), Some(6), "stderr:\n{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("runtime execution error:"));
    assert!(stderr.contains("replay target not found"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn golden_schema_legacy_trace_replay_remains_compatible() {
    let workspace = unique_path("replay-legacy-workspace", "dir");
    fs::create_dir_all(workspace.join(".axonrunner/trace")).expect("trace dir should exist");
    let legacy = serde_json::json!({
        "schema": "axonrunner.trace.intent.v1",
        "timestamp_ms": 1_u64,
        "actor_id": "system",
        "intent_id": "cli-legacy",
        "kind": "write",
        "outcome": "accepted",
        "policy_code": "allowed",
        "effect_count": 1,
        "revision": 1,
        "mode": "active",
        "provider": "applied",
        "memory": "applied",
        "tool": "applied",
        "tool_outputs": ["log=runtime.log"],
        "first_failure": serde_json::Value::Null,
        "artifacts": {
            "plan": ".axonrunner/artifacts/cli-legacy.plan.md",
            "apply": ".axonrunner/artifacts/cli-legacy.apply.md",
            "verify": ".axonrunner/artifacts/cli-legacy.verify.md",
            "report": ".axonrunner/artifacts/cli-legacy.report.md"
        },
        "report_written": true,
        "report_error": serde_json::Value::Null
    });
    fs::write(
        workspace.join(".axonrunner/trace/events.jsonl"),
        format!(
            "{}\n",
            serde_json::to_string(&legacy).expect("legacy trace should serialize")
        ),
    )
    .expect("trace log should be written");

    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "replay-legacy-read",
    );
    let stdout = stdout_of(&replay);
    let stderr = stderr_of(&replay);

    assert!(replay.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("replay intent_id=cli-legacy count=1"));
    assert!(stdout.contains("replay verification status=unknown"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_status_and_replay_render_aborted_outcome_from_trace() {
    let workspace = unique_path("aborted-trace-workspace", "dir");
    fs::create_dir_all(workspace.join(".axonrunner/trace")).expect("trace dir should exist");
    let trace = serde_json::json!({
        "schema": "axonrunner.trace.intent.v1",
        "timestamp_ms": 1_u64,
        "actor_id": "system",
        "intent_id": "cli-abort",
        "kind": "write",
        "outcome": "accepted",
        "policy_code": "allowed",
        "effect_count": 1,
        "revision": 1,
        "mode": "active",
        "provider": "applied",
        "memory": "applied",
        "tool": "applied",
        "tool_outputs": ["log=runtime.log"],
        "first_failure": serde_json::Value::Null,
        "verification": {
            "status": "passed",
            "summary": "report_written=true"
        },
        "patch_artifacts": [],
        "run": {
            "run_id": "run-abort",
            "step_ids": ["run-abort/step-1-planning"],
            "provider_cwd": "/tmp/aborted-workspace",
            "phase": "aborted",
            "outcome": "aborted",
            "reason": "operator_abort",
            "plan_summary": "intent_id=cli-abort legacy_write key=alpha outcome=accepted",
            "planned_steps": 4,
            "repair": {
                "attempted": false,
                "status": "skipped",
                "summary": "not_needed"
            }
        },
        "artifacts": {
            "plan": ".axonrunner/artifacts/cli-abort.plan.md",
            "apply": ".axonrunner/artifacts/cli-abort.apply.md",
            "verify": ".axonrunner/artifacts/cli-abort.verify.md",
            "report": ".axonrunner/artifacts/cli-abort.report.md"
        },
        "report_written": true,
        "report_error": serde_json::Value::Null
    });
    fs::write(
        workspace.join(".axonrunner/trace/events.jsonl"),
        format!(
            "{}\n",
            serde_json::to_string(&trace).expect("trace should serialize")
        ),
    )
    .expect("trace log should be written");

    let status = run_cli_with_env(
        &["status"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "aborted-status",
    );
    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "aborted-replay",
    );

    assert!(status.status.success());
    assert!(replay.status.success());
    assert!(
        stdout_of(&status).contains("status run run_id=run-abort phase=aborted outcome=aborted")
    );
    assert!(
        stdout_of(&replay).contains("replay run run_id=run-abort phase=aborted outcome=aborted")
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn golden_replay_legacy_text_contract_is_stable() {
    let workspace = unique_path("replay-legacy-golden-workspace", "dir");
    fs::create_dir_all(workspace.join(".axonrunner/trace")).expect("trace dir should exist");
    let legacy = serde_json::json!({
        "schema": "axonrunner.trace.intent.v1",
        "timestamp_ms": 1_u64,
        "actor_id": "system",
        "intent_id": "cli-legacy",
        "kind": "write",
        "outcome": "accepted",
        "policy_code": "allowed",
        "effect_count": 1,
        "revision": 1,
        "mode": "active",
        "provider": "applied",
        "memory": "applied",
        "tool": "applied",
        "tool_outputs": ["log=runtime.log"],
        "first_failure": serde_json::Value::Null,
        "artifacts": {
            "plan": ".axonrunner/artifacts/cli-legacy.plan.md",
            "apply": ".axonrunner/artifacts/cli-legacy.apply.md",
            "verify": ".axonrunner/artifacts/cli-legacy.verify.md",
            "report": ".axonrunner/artifacts/cli-legacy.report.md"
        },
        "report_written": true,
        "report_error": serde_json::Value::Null
    });
    fs::write(
        workspace.join(".axonrunner/trace/events.jsonl"),
        format!(
            "{}\n",
            serde_json::to_string(&legacy).expect("legacy trace should serialize")
        ),
    )
    .expect("trace log should be written");

    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "replay-legacy-golden-read",
    );
    let stderr = stderr_of(&replay);

    assert!(replay.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    assert_eq!(
        stdout_lines(&replay),
        vec![
            String::from(
                "replay intent_id=cli-legacy count=1 revision=1 mode=active kind=write outcome=accepted policy=allowed",
            ),
            String::from(
                "replay stages provider=applied memory=applied tool=applied report_written=true",
            ),
            String::from(
                "replay verification status=unknown summary=legacy_trace_without_verification",
            ),
            String::from(
                "replay artifacts plan=.axonrunner/artifacts/cli-legacy.plan.md apply=.axonrunner/artifacts/cli-legacy.apply.md verify=.axonrunner/artifacts/cli-legacy.verify.md report=.axonrunner/artifacts/cli-legacy.report.md",
            ),
            String::from(
                "replay artifact_index count=1 latest_report=.axonrunner/artifacts/cli-legacy.report.md",
            ),
            String::from(
                "replay summary failed_intents=0 false_success_intents=0 false_done_intents=0"
            ),
        ]
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_replay_counts_false_done_runs() {
    let workspace = unique_path("replay-false-done-workspace", "dir");
    fs::create_dir_all(workspace.join(".axonrunner/trace")).expect("trace dir should exist");
    let trace = serde_json::json!({
        "schema": "axonrunner.trace.intent.v1",
        "timestamp_ms": 1_u64,
        "actor_id": "system",
        "intent_id": "cli-false-done",
        "kind": "goal",
        "outcome": "accepted",
        "policy_code": "allowed",
        "effect_count": 0,
        "revision": 1,
        "mode": "active",
        "provider": "skipped",
        "memory": "skipped",
        "tool": "applied",
        "tool_outputs": ["verifier=workspace"],
        "first_failure": serde_json::Value::Null,
        "verification": {
            "status": "failed",
            "summary": "done_condition_missing_report_artifact:report"
        },
        "patch_artifacts": [],
        "run": {
            "run_id": "run-false-done",
            "step_ids": ["run-false-done/step-1-planning"],
            "provider_cwd": "/tmp/workspace",
            "phase": "failed",
            "outcome": "failed",
            "reason": "done_condition_missing_report_artifact:report",
            "plan_summary": "intent_id=cli-false-done goal",
            "planned_steps": 4,
            "repair": {
                "attempted": false,
                "status": "skipped",
                "summary": "verification_passed"
            }
        },
        "artifacts": {
            "plan": ".axonrunner/artifacts/cli-false-done.plan.md",
            "apply": ".axonrunner/artifacts/cli-false-done.apply.md",
            "verify": ".axonrunner/artifacts/cli-false-done.verify.md",
            "report": ".axonrunner/artifacts/cli-false-done.report.md"
        },
        "report_written": true,
        "report_error": serde_json::Value::Null
    });
    fs::write(
        workspace.join(".axonrunner/trace/events.jsonl"),
        format!(
            "{}\n",
            serde_json::to_string(&trace).expect("trace should serialize")
        ),
    )
    .expect("trace log should be written");

    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[("AXONRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "replay-false-done-read",
    );
    let stdout = stdout_of(&replay);

    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    assert!(stdout.contains(
        "replay verification status=failed summary=done_condition_missing_report_artifact:report"
    ));
    assert!(
        stdout.contains(
            "replay summary failed_intents=0 false_success_intents=1 false_done_intents=1"
        )
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_status_reports_blocked_codek_provider() {
    let output = run_cli_with_env(
        &["status"],
        &[
            ("AXONRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXONRUNNER_CODEX_BIN", "/definitely-missing-codex-binary"),
        ],
        "codek-provider-status",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("provider_state=blocked"));
    assert!(stdout.contains("reason=binary_not_found"));
}

#[test]
fn e2e_cli_freeze_state_persists_across_processes() {
    let state_path = unique_path("state-freeze", "snapshot");

    let freeze = run_cli_with_env(
        &["freeze"],
        &[("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path))],
        "state-freeze-freeze",
    );
    let write = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path))],
        "state-freeze-write",
    );
    let status = run_cli_with_env(
        &["status"],
        &[("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path))],
        "state-freeze-status",
    );

    let freeze_stdout = stdout_of(&freeze);
    let write_stdout = stdout_of(&write);
    let status_stdout = stdout_of(&status);

    assert!(freeze.status.success());
    assert!(write.status.success());
    assert!(status.status.success());
    assert!(freeze_stdout.contains("intent id=cli-1 kind=freeze outcome=accepted"));
    assert!(write_stdout.contains("intent id=cli-2 kind=write outcome=rejected"));
    assert!(write_stdout.contains("policy=readonly_mutation"));
    assert!(status_stdout.contains("mode=read_only"));

    let _ = fs::remove_file(state_path);
}

#[test]
fn e2e_cli_halt_state_persists_and_halt_is_idempotent() {
    let state_path = unique_path("state-halt", "snapshot");

    let first_halt = run_cli_with_env(
        &["halt"],
        &[("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path))],
        "state-halt-first",
    );
    let second_halt = run_cli_with_env(
        &["halt"],
        &[("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path))],
        "state-halt-second",
    );
    let write = run_cli_with_env(
        &["run", "write:alpha=42"],
        &[("AXONRUNNER_RUNTIME_STATE_PATH", path_str(&state_path))],
        "state-halt-write",
    );

    let first_stdout = stdout_of(&first_halt);
    let second_stdout = stdout_of(&second_halt);
    let write_stdout = stdout_of(&write);

    assert!(first_halt.status.success());
    assert!(second_halt.status.success());
    assert!(write.status.success());
    assert!(first_stdout.contains("intent id=cli-1 kind=halt outcome=accepted"));
    assert!(second_stdout.contains("intent id=cli-2 kind=halt outcome=accepted"));
    assert!(write_stdout.contains("intent id=cli-3 kind=write outcome=rejected"));
    assert!(write_stdout.contains("policy=runtime_halted"));

    let _ = fs::remove_file(state_path);
}

#[test]
fn e2e_cli_health_reports_blocked_openai_provider_without_api_key() {
    let output = run_cli_with_env(
        &["health"],
        &[("AXONRUNNER_RUNTIME_PROVIDER", "openai")],
        "openai-provider-health",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("health ok=false"));
    assert!(stdout.contains("provider_state=blocked"));
    assert!(stdout.contains("reason=experimental_provider_disabled"));
}

#[test]
fn e2e_cli_health_reports_missing_api_key_after_openai_experimental_opt_in() {
    let output = run_cli_with_env(
        &["health"],
        &[
            ("AXONRUNNER_RUNTIME_PROVIDER", "openai"),
            ("AXONRUNNER_EXPERIMENTAL_OPENAI", "1"),
        ],
        "openai-provider-health-opt-in",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("health ok=false"));
    assert!(stdout.contains("provider_state=blocked"));
    assert!(stdout.contains("reason=missing_openai_api_key"));
}

#[test]
fn e2e_cli_config_file_can_set_workspace_state_path_and_provider_model() {
    let state_path = unique_path("config-state", "snapshot");
    let workspace = unique_path("config-workspace", "dir");
    let config_path = unique_path("runtime-config", "cfg");
    let config_contents = format!(
        "profile=prod\nprovider=mock-local\nprovider_model=mock-plan\nworkspace={}\nstate_path={}\n",
        workspace.display(),
        state_path.display()
    );
    fs::write(&config_path, config_contents).expect("config file should be written");

    let config_arg = format!("--config-file={}", config_path.display());
    let write = run_cli_without_runtime_defaults(
        &[config_arg.as_str(), "run", "write:alpha=42"],
        &[],
        "config-file-write",
    );
    let status = run_cli_without_runtime_defaults(
        &[config_arg.as_str(), "status"],
        &[],
        "config-file-status",
    );

    let write_stdout = stdout_of(&write);
    let status_stdout = stdout_of(&status);

    assert!(write.status.success());
    assert!(status.status.success());
    assert!(write_stdout.contains("intent id=cli-1 kind=write outcome=accepted"));
    assert!(status_stdout.contains("provider_model=mock-plan"));
    assert!(workspace.join("runtime.log").exists());
    assert!(state_path.exists());

    let _ = fs::remove_file(config_path);
    let _ = fs::remove_file(state_path);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_unknown_command_stderr_stable() {
    let output = run_cli(&["gateway"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout.is_empty());
    assert!(stderr.contains("parse error:"));
    assert!(stderr.contains("unknown command 'gateway'"));
    assert!(stderr.contains(CLI_USAGE));
}
