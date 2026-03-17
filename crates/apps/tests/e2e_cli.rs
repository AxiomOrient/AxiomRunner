mod common;
use common::resolve_cli_bin;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CLI_USAGE: &str = "\
usage:
  axiomrunner_apps [global-options] <command> [command-args]

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
  help";

const SANITIZED_ENV_KEYS: &[&str] = &[
    "AXIOMRUNNER_PROFILE",
    "AXIOMRUNNER_CODEX_BIN",
    "AXIOMRUNNER_RUNTIME_PROVIDER",
    "AXIOMRUNNER_RUNTIME_PROVIDER_MODEL",
    "AXIOMRUNNER_RUNTIME_MAX_TOKENS",
    "AXIOMRUNNER_RUNTIME_COMMAND_TIMEOUT_MS",
    "AXIOMRUNNER_RUNTIME_MEMORY_PATH",
    "AXIOMRUNNER_RUNTIME_STATE_PATH",
    "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
    "AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
    "AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION",
    "AXIOMRUNNER_RUNTIME_TOOL_LOG_PATH",
    "AXIOMRUNNER_EXPERIMENTAL_OPENAI",
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
        (*key == "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE").then(|| PathBuf::from(*value))
    });
    let explicit_artifact_workspace = env.iter().find_map(|(key, value)| {
        (*key == "AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE").then(|| PathBuf::from(*value))
    });
    let tool_workspace = explicit_tool_workspace
        .clone()
        .unwrap_or_else(|| canonical_home.join(".axiomrunner").join("workspace"));
    let artifact_workspace = explicit_artifact_workspace
        .clone()
        .unwrap_or_else(|| tool_workspace.clone());

    let mut cmd = Command::new(resolve_cli_bin());
    cmd.env("HOME", &canonical_home).args(args);
    for key in SANITIZED_ENV_KEYS {
        cmd.env_remove(key);
    }
    if inject_runtime_defaults && explicit_tool_workspace.is_none() {
        cmd.env("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", &tool_workspace);
    }
    if inject_runtime_defaults
        && !env
            .iter()
            .any(|(key, _)| *key == "AXIOMRUNNER_RUNTIME_TOOL_LOG_PATH")
    {
        cmd.env(
            "AXIOMRUNNER_RUNTIME_TOOL_LOG_PATH",
            artifact_workspace.join("runtime.log"),
        );
    }
    for (key, value) in env {
        cmd.env(key, value);
    }

    let output = cmd.output().expect("axiomrunner_apps binary should run");
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
        "axiomrunner-e2e-cli-{label}-{}-{tick}.{extension}",
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
    run_checked_command("git", &["config", "user.name", "AxiomRunner Test"], path);
    fs::write(path.join("README.md"), "fixture\n").expect("fixture file should exist");
    run_checked_command("git", &["add", "README.md"], path);
    run_checked_command("git", &["commit", "-m", "init"], path);
}

#[test]
fn e2e_cli_accepts_spaced_global_workspace_option() {
    let workspace = unique_path("spaced-workspace", "dir");
    fs::create_dir_all(&workspace).expect("workspace should exist");

    let output = run_cli_without_runtime_defaults(
        &["--workspace", path_str(&workspace), "doctor", "--json"],
        &[],
        "spaced-workspace",
    );

    assert!(output.status.success(), "stderr:\n{}", stderr_of(&output));
    assert!(stdout_of(&output).contains(path_str(&workspace)));

    let _ = fs::remove_dir_all(workspace);
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
    { "label": "report", "evidence": "report_artifact_exists" }
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
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&repo_root)),
            (
                "AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
                path_str(&artifact_workspace),
            ),
            ("AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION", "1"),
        ],
        "git-worktree-run",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&repo_root)),
            (
                "AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
                path_str(&artifact_workspace),
            ),
            ("AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION", "1"),
        ],
        "git-worktree-status",
    );
    let stderr = stderr_of(&run);

    assert!(run.status.success(), "stderr:\n{stderr}");
    assert!(status.status.success(), "stderr:\n{}", stderr_of(&status));
    let commands_dir = artifact_workspace.join(".axiomrunner/commands");
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
    assert!(stdout_of(&status).contains("execution_workspace="));
    assert!(stdout_of(&status).contains("run-1"));
    assert!(
        artifact_workspace
            .join(".axiomrunner/artifacts/cli-1.checkpoint.json")
            .exists()
    );

    let _ = fs::remove_dir_all(repo_root);
    let _ = fs::remove_dir_all(artifact_workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_failed_isolated_run_writes_rollback_metadata() {
    let repo_root = unique_path("rollback-repo", "dir");
    let artifact_workspace = unique_path("rollback-artifacts", "dir");
    let goal_file = unique_path("rollback-goal", "json");
    init_git_repo(&repo_root);
    let canonical_repo_root = fs::canonicalize(&repo_root).unwrap_or(repo_root.clone());
    fs::write(
        &goal_file,
        r#"{
  "summary": "Need explicit workflow pack before execution",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "domain verification", "detail": "representative domain path" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&repo_root)),
            (
                "AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
                path_str(&artifact_workspace),
            ),
            ("AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION", "1"),
        ],
        "rollback-run",
    );
    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&repo_root)),
            (
                "AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
                path_str(&artifact_workspace),
            ),
            ("AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION", "1"),
        ],
        "rollback-replay",
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(stdout_of(&run).contains("phase=blocked outcome=blocked"));
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    let checkpoint_path = artifact_workspace.join(".axiomrunner/artifacts/cli-1.checkpoint.json");
    let rollback_path = artifact_workspace.join(".axiomrunner/artifacts/cli-1.rollback.json");
    assert!(checkpoint_path.exists(), "checkpoint metadata should exist");
    assert!(rollback_path.exists(), "rollback metadata should exist");
    let report_path = artifact_workspace.join(".axiomrunner/artifacts/cli-1.report.md");
    let report = fs::read_to_string(&report_path).expect("rollback report should be readable");
    let checkpoint_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&checkpoint_path).expect("checkpoint metadata should be readable"),
    )
    .expect("checkpoint metadata should be valid json");
    let rollback_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&rollback_path).expect("rollback metadata should be readable"),
    )
    .expect("rollback metadata should be valid json");
    assert_eq!(checkpoint_json["schema"], "axiomrunner.checkpoint.v1");
    assert_eq!(
        checkpoint_json["restore_path"],
        canonical_repo_root.display().to_string()
    );
    assert_eq!(rollback_json["schema"], "axiomrunner.rollback.v1");
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
    assert!(replay_stdout.contains("replay checkpoint metadata="));
    assert!(replay_stdout.contains("replay rollback metadata="));
    assert!(replay_stdout.contains(canonical_repo_root.display().to_string().as_str()));
    assert!(report.contains("checkpoint=metadata="));
    assert!(report.contains("cli-1.checkpoint.json"));
    assert!(report.contains("rollback=metadata="));
    assert!(report.contains("cli-1.rollback.json"));
    assert!(report.contains("restore_path="));

    let _ = fs::remove_dir_all(repo_root);
    let _ = fs::remove_dir_all(artifact_workspace);
    let _ = fs::remove_file(goal_file);
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
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "workspace files", "detail": "ls ." }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
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
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-replay",
    );

    assert!(status.status.success());
    assert!(replay.status.success());
    assert!(stdout_of(&status).contains("status run run_id=run-1 phase=completed outcome=success"));
    assert!(stdout_of(&status).contains("approval_state=not_required"));
    assert!(stdout_of(&status).contains("reason_code=verification_passed"));
    assert!(stdout_of(&status).contains("artifact_summary="));
    assert!(stdout_of(&replay).contains("replay run run_id=run-1 phase=completed outcome=success"));
    assert!(
        stdout_of(&replay)
            .contains("replay verification status=passed summary=goal_done_conditions_verified")
    );
    assert!(stdout_of(&replay).contains("replay health failed_intents=0 false_success_intents=0 false_done_intents=0 latest_failure=none"));
    assert!(
        stdout_of(&replay).contains("replay stages provider=skipped memory=skipped tool=applied")
    );
    assert!(stdout_of(&replay).contains("replay step id="));
    assert!(stdout_of(&replay).contains("label=validate goal contract"));
    assert!(
        fs::read_to_string(workspace.join(".axiomrunner/artifacts/cli-1.plan.md"))
            .expect("plan artifact should exist")
            .contains("goal=Ship one bounded goal package")
    );
    let plan = fs::read_to_string(workspace.join(".axiomrunner/artifacts/cli-1.plan.md"))
        .expect("plan artifact should exist");
    assert!(plan.contains("run_id=run-1"));
    assert!(plan.contains("workflow_pack=goal-default-v1"));
    assert!(plan.contains("verifier_flow=generic"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_goal_file_on_risk_requires_approval_before_execution() {
    let workspace = unique_path("goal-file-always-workspace", "dir");
    let state_path = unique_path("goal-file-always-state", "snapshot");
    let goal_file = unique_path("goal-file-always", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Wait for always approval before execution",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "workspace files", "detail": "ls ." }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "always"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-always-run",
    );
    assert!(run.status.success());
    assert!(stdout_of(&run).contains(
        "run intent_id=cli-1 phase=waiting_approval outcome=approval_required reason=approval_required_before_execution"
    ));
    let pending_report =
        fs::read_to_string(workspace.join(".axiomrunner/artifacts/cli-1.report.md"))
            .expect("pending approval report should exist");
    let pending_trace = fs::read_to_string(workspace.join(".axiomrunner/trace/events.jsonl"))
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
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-always-resume",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-always-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-always-replay",
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
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "workspace files", "detail": "ls ." }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "always"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-approval-run",
    );
    let pending_status = run_cli_with_env(
        &["status", "run-1"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-approval-pending-status",
    );
    let pending_doctor = run_cli_with_env(
        &["doctor"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
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
    assert!(
        stdout_of(&pending_status)
            .contains("approval_state=required verifier_state=skipped verifier_strength=skipped")
    );
    assert!(stdout_of(&pending_doctor).contains("doctor pending_run run_id=run-1"));
    assert!(
        stdout_of(&pending_doctor)
            .contains("approval_state=required verifier_state=skipped verifier_strength=skipped")
    );
    let pending_replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-approval-pending-replay",
    );
    assert!(pending_replay.status.success());
    assert!(
        stdout_of(&pending_replay)
            .contains("approval_state=required verifier_state=skipped verifier_strength=skipped")
    );

    let resume = run_cli_with_env(
        &["resume", "run-1"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-approval-resume",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-approval-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
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
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "workspace files", "detail": "ls ." }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "always"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-abort-run",
    );
    assert!(run.status.success());

    let abort = run_cli_with_env(
        &["abort", "run-1"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-abort-command",
    );
    let second_abort = run_cli_with_env(
        &["abort", "run-1"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-abort-command-again",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "goal-file-abort-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-abort-replay",
    );
    let trace = fs::read_to_string(workspace.join(".axiomrunner/trace/events.jsonl"))
        .expect("trace should be readable");
    let latest = trace
        .lines()
        .last()
        .expect("trace should contain latest event");
    let latest: serde_json::Value =
        serde_json::from_str(latest).expect("latest trace event should parse");

    assert!(abort.status.success());
    assert!(
        stdout_of(&abort)
            .contains("abort run_id=run-1 phase=aborted outcome=aborted reason=operator_abort")
    );
    assert!(!second_abort.status.success());
    assert!(stderr_of(&second_abort).contains(
        "abort only supports pending goal-file control runs; no pending control run is available"
    ));
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
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "workspace files", "detail": "ls ." }
  ],
  "budget": { "max_steps": 1, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-budget-run",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-budget-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
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
        stdout_of(&replay)
            .contains("reason_code=budget_exhausted_before_execution reason_detail=none")
    );
    let report = fs::read_to_string(workspace.join(".axiomrunner/artifacts/cli-1.report.md"))
        .expect("budget report should exist");
    assert!(report.contains("provider=skipped"));
    assert!(report.contains("tool=skipped"));
    assert!(report.contains("run_reason_code=budget_exhausted_before_execution"));
    assert!(report.contains("run_reason_detail=none"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_goal_file_blocks_when_token_budget_is_already_exhausted() {
    let workspace = unique_path("goal-file-token-budget-workspace", "dir");
    let goal_file = unique_path("goal-file-token-budget", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Token budget exhaustion before execution",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "workspace files", "detail": "ls ." }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 64 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-token-budget-run",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-token-budget-status",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-token-budget-replay",
    );

    assert!(run.status.success());
    assert!(stdout_of(&run).contains(
        "run intent_id=cli-1 phase=blocked outcome=budget_exhausted reason=budget_exhausted_before_execution_tokens:4096>64"
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
        stdout_of(&replay)
            .contains("reason_code=budget_exhausted_before_execution_tokens reason_detail=4096>64")
    );
    let report = fs::read_to_string(workspace.join(".axiomrunner/artifacts/cli-1.report.md"))
        .expect("token budget report should exist");
    assert!(report.contains("run_reason_code=budget_exhausted_before_execution_tokens"));
    assert!(report.contains("run_reason_detail=4096>64"));

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
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "workspace files", "detail": "ls ." }
  ],
  "budget": { "max_steps": 6, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_COMMAND_ALLOWLIST", "git"),
        ],
        "goal-file-repair-budget-run",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
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
fn e2e_cli_goal_file_zero_repair_budget_does_not_claim_attempt() {
    let workspace = unique_path("goal-file-zero-repair-budget-workspace", "dir");
    let goal_file = unique_path("goal-file-zero-repair-budget", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Repair budget exhausted before any retry",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axiomrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_COMMAND_ALLOWLIST", "git"),
        ],
        "goal-file-zero-repair-budget-run",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-file-zero-repair-budget-replay",
    );

    assert!(run.status.success());
    assert!(stdout_of(&run).contains("phase=blocked outcome=budget_exhausted"));
    assert!(stdout_of(&run).contains("repair_budget_exhausted:attempts=0/0"));
    assert!(replay.status.success());
    assert!(stdout_of(&replay).contains("replay repair attempted=false status=budget_exhausted"));
    assert!(stdout_of(&replay).contains("repair_budget_exhausted:attempts=0/0"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_workspace_lock_blocks_mutating_commands_but_allows_status_reads() {
    let workspace = unique_path("workspace-lock-workspace", "dir");
    fs::create_dir_all(workspace.join(".axiomrunner")).expect("lock dir should exist");
    fs::write(
        workspace.join(".axiomrunner/runtime.lock"),
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
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axiomrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "workspace-lock-run",
    );
    let status = run_cli_with_env(
        &["status"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "workspace-lock-status",
    );

    assert_eq!(run.status.code(), Some(6));
    assert!(stderr_of(&run).contains("workspace lock is active"));
    assert!(status.status.success());
    assert!(
        stdout_of(&status)
            .contains("status revision=0 mode=active last_intent=- last_decision=- last_policy=-")
    );

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_workspace_lock_recovers_stale_pid_and_runs() {
    let workspace = unique_path("workspace-stale-lock-workspace", "dir");
    fs::create_dir_all(workspace.join(".axiomrunner")).expect("lock dir should exist");
    fs::write(
        workspace.join(".axiomrunner/runtime.lock"),
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
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "workspace files", "detail": "ls ." }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "workspace-stale-lock-run",
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(stdout_of(&run).contains("phase=completed outcome=success"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_resume_rejects_non_pending_state_with_clear_error() {
    let workspace = unique_path("resume-no-pending-workspace", "dir");
    let state_path = unique_path("resume-no-pending-state", "snapshot");
    let resume = run_cli_with_env(
        &["resume", "latest"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "resume-no-pending",
    );

    assert!(!resume.status.success());
    assert!(stderr_of(&resume).contains(
        "resume only supports pending goal-file approval runs; no pending approval run is available"
    ));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(state_path);
}

#[test]
fn e2e_cli_resume_rejects_after_pending_run_was_already_completed() {
    let workspace = unique_path("resume-after-complete-workspace", "dir");
    let state_path = unique_path("resume-after-complete-state", "snapshot");
    let goal_file = unique_path("resume-after-complete-goal", "json");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Resume only applies to waiting approval pending runs",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "workspace files", "detail": "ls ." }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "always"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "resume-after-complete-run",
    );
    let first_resume = run_cli_with_env(
        &["resume", "latest"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "resume-after-complete-first-resume",
    );
    let second_resume = run_cli_with_env(
        &["resume", "latest"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "resume-after-complete-second-resume",
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(
        first_resume.status.success(),
        "stderr:\n{}",
        stderr_of(&first_resume)
    );
    assert!(!second_resume.status.success());
    assert!(stderr_of(&second_resume).contains(
        "resume only supports pending goal-file approval runs; no pending approval run is available"
    ));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(state_path);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_run_missing_goal_file_surfaces_file_not_found_directly() {
    let workspace = unique_path("missing-goal-workspace", "dir");
    let output = run_cli_with_env(
        &["run", "./missing-goal.json"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
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
  "entry_goal": "goal",
  "recommended_verifier_flow": ["lint", "build"],
  "allowed_tools": [{"operation": "run_command", "scope": "workspace"}],
  "verifier_rules": [
    {
      "label": "build-pass",
      "profile": "build",
      "command": { "program": "pwd", "args": [] },
      "artifact_expectation": "build path exists",
      "required": true
    },
    {
      "label": "lint-pass",
      "profile": "lint",
      "command": { "program": "pwd", "args": [] },
      "artifact_expectation": "lint path exists",
      "required": true
    }
  ],
  "approval_mode": "never"
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
    {{ "label": "report", "evidence": "report_artifact_exists" }}
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
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-pack-run",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-pack-replay",
    );
    let report = fs::read_to_string(workspace.join(".axiomrunner/artifacts/cli-1.report.md"))
        .expect("report should exist");

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    assert!(stdout_of(&run).contains("phase=completed outcome=success"));
    let lint_idx = report
        .find("\"label\":\"lint-pass\"")
        .expect("lint verifier should exist");
    let build_idx = report
        .find("\"label\":\"build-pass\"")
        .expect("build verifier should exist");
    assert!(
        lint_idx < build_idx,
        "recommended verifier flow should drive execution order"
    );

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
    let _ = fs::remove_file(pack_file);
}

#[test]
fn e2e_cli_default_goal_pack_blocks_when_verification_is_pack_required() {
    let workspace = unique_path("goal-pack-required-workspace", "dir");
    let goal_file = unique_path("goal-pack-required-goal", "json");
    fs::create_dir_all(&workspace).expect("workspace should exist");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Need a domain-specific workflow pack",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "domain verification", "detail": "representative domain path" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-pack-required-run",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-pack-required-replay",
    );
    let verify = fs::read_to_string(workspace.join(".axiomrunner/artifacts/cli-1.verify.md"))
        .expect("verify artifact should exist");
    let report = fs::read_to_string(workspace.join(".axiomrunner/artifacts/cli-1.report.md"))
        .expect("report artifact should exist");

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    assert!(stdout_of(&run).contains("phase=blocked outcome=blocked"));
    assert!(stdout_of(&run).contains("reason=pack_required:domain verification"));
    assert!(stdout_of(&replay).contains("replay verification status=pack_required"));
    assert!(stdout_of(&replay).contains("\"strength\":\"pack_required\""));
    assert!(stdout_of(&replay).contains("verifier_strength=pack_required"));
    assert!(verify.contains("verifier_evidence="));
    assert!(verify.contains("\"command\":\"ls .\""));
    assert!(verify.contains("\"artifact_path\":\""));
    assert!(verify.contains("\"stdout_summary\":\""));
    assert!(report.contains("verifier_evidence="));
    assert!(report.contains("verifier_strength=pack_required"));
    assert!(report.contains("verifier_summary=pack_required:domain verification"));
    assert!(report.contains("verifier_non_executed_reason=pack_required:domain verification"));
    assert!(report.contains("summary=phase=blocked outcome=blocked"));
    assert!(report.contains("risk=blocked"));
    assert!(report.contains("next_action=inspect verifier summary and unblock the run"));
    assert!(report.contains(
        "\"expectation\":\"pack_required fallback probe for verifier `domain verification`\""
    ));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_default_goal_pack_blocks_when_verification_is_weak() {
    let workspace = unique_path("goal-pack-weak-workspace", "dir");
    let goal_file = unique_path("goal-pack-weak-goal", "json");
    fs::create_dir_all(&workspace).expect("workspace should exist");
    fs::write(
        &goal_file,
        r#"{
  "summary": "Need honest weak verification visibility",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "workspace consistency", "detail": "workspace consistency review" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", path_str(&goal_file)],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-pack-weak-run",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "goal-pack-weak-replay",
    );
    let report = fs::read_to_string(workspace.join(".axiomrunner/artifacts/cli-1.report.md"))
        .expect("report artifact should exist");

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    assert!(stdout_of(&run).contains("phase=blocked outcome=blocked"));
    assert!(stdout_of(&run).contains("reason=verification_weak:workspace consistency"));
    assert!(stdout_of(&replay).contains("replay verification status=verification_weak"));
    assert!(stdout_of(&replay).contains("verifier_strength=verification_weak"));
    assert!(report.contains("verifier_strength=verification_weak"));
    assert!(report.contains("verifier_summary=verification_weak:workspace consistency"));
    assert!(
        report.contains("verifier_non_executed_reason=verification_weak:workspace consistency")
    );

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
}

#[test]
fn e2e_cli_constraints_block_external_verifier_commands() {
    let workspace = unique_path("constraint-external-workspace", "dir");
    let goal_file = unique_path("constraint-external-goal", "json");
    let pack_file = unique_path("constraint-external-pack", "json");
    fs::create_dir_all(&workspace).expect("workspace should exist");
    fs::write(
        &pack_file,
        r#"{
  "pack_id": "external-check-pack",
  "version": "1",
  "entry_goal": "goal",
  "recommended_verifier_flow": ["generic"],
  "allowed_tools": [{"operation": "run_command", "scope": "workspace"}],
  "verifier_rules": [
    {
      "label": "external-check",
      "profile": "generic",
      "command": { "program": "curl", "args": ["https://example.com"] },
      "artifact_expectation": "external command should not run",
      "required": true
    }
  ],
  "approval_mode": "never"
}"#,
    )
    .expect("pack file should be written");
    fs::write(
        &goal_file,
        format!(
            r#"{{
  "summary": "Block external verifier command by constraint",
  "workspace_root": "/workspace",
  "constraints": [
    {{ "label": "external_commands", "detail": "deny" }}
  ],
  "done_conditions": [
    {{ "label": "report", "evidence": "report_artifact_exists" }}
  ],
  "verification_checks": [
    {{ "label": "placeholder", "detail": "ls ." }}
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
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "constraint-external-run",
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "constraint-external-replay",
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    assert!(stdout_of(&run).contains("phase=blocked outcome=blocked"));
    assert!(stdout_of(&run).contains("reason=policy=constraint_external_commands"));
    assert!(stdout_of(&replay).contains("policy=constraint_external_commands"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
    let _ = fs::remove_file(pack_file);
}

#[test]
fn e2e_cli_constraints_block_destructive_verifier_commands() {
    let workspace = unique_path("constraint-destructive-workspace", "dir");
    let goal_file = unique_path("constraint-destructive-goal", "json");
    let pack_file = unique_path("constraint-destructive-pack", "json");
    fs::create_dir_all(&workspace).expect("workspace should exist");
    fs::write(
        &pack_file,
        r#"{
  "pack_id": "destructive-check-pack",
  "version": "1",
  "entry_goal": "goal",
  "recommended_verifier_flow": ["generic"],
  "allowed_tools": [{"operation": "run_command", "scope": "workspace"}],
  "verifier_rules": [
    {
      "label": "destructive-check",
      "profile": "generic",
      "command": { "program": "rm", "args": ["build-cache"] },
      "artifact_expectation": "destructive command should not run",
      "required": true
    }
  ],
  "approval_mode": "never"
}"#,
    )
    .expect("pack file should be written");
    fs::write(
        &goal_file,
        format!(
            r#"{{
  "summary": "Block destructive verifier command by constraint",
  "workspace_root": "/workspace",
  "constraints": [
    {{ "label": "destructive_commands", "detail": "deny" }}
  ],
  "done_conditions": [
    {{ "label": "report", "evidence": "report_artifact_exists" }}
  ],
  "verification_checks": [
    {{ "label": "placeholder", "detail": "ls ." }}
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
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "constraint-destructive-run",
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(stdout_of(&run).contains("phase=blocked outcome=blocked"));
    assert!(stdout_of(&run).contains("reason=policy=constraint_destructive_commands"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
    let _ = fs::remove_file(pack_file);
}

#[test]
fn e2e_cli_constraints_block_verifier_path_outside_scope() {
    let workspace = unique_path("constraint-path-workspace", "dir");
    let goal_file = unique_path("constraint-path-goal", "json");
    let pack_file = unique_path("constraint-path-pack", "json");
    fs::create_dir_all(&workspace).expect("workspace should exist");
    fs::write(
        &pack_file,
        r#"{
  "pack_id": "path-scope-pack",
  "version": "1",
  "entry_goal": "goal",
  "recommended_verifier_flow": ["generic"],
  "allowed_tools": [{"operation": "run_command", "scope": "workspace"}],
  "verifier_rules": [
    {
      "label": "scope-check",
      "profile": "generic",
      "command": { "program": "ls", "args": ["src"] },
      "artifact_expectation": "scope command should stay inside tests",
      "required": true
    }
  ],
  "approval_mode": "never"
}"#,
    )
    .expect("pack file should be written");
    fs::write(
        &goal_file,
        format!(
            r#"{{
  "summary": "Block verifier path outside allowed scope",
  "workspace_root": "/workspace",
  "constraints": [
    {{ "label": "path_scope", "detail": "tests" }}
  ],
  "done_conditions": [
    {{ "label": "report", "evidence": "report_artifact_exists" }}
  ],
  "verification_checks": [
    {{ "label": "placeholder", "detail": "ls ." }}
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
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "constraint-path-run",
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(stdout_of(&run).contains("phase=blocked outcome=blocked"));
    assert!(stdout_of(&run).contains("reason=policy=constraint_path_scope"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(goal_file);
    let _ = fs::remove_file(pack_file);
}

#[test]
fn e2e_cli_constraints_escalate_high_risk_verifier_to_pending_approval() {
    let workspace = unique_path("constraint-approval-workspace", "dir");
    let state_path = unique_path("constraint-approval-state", "snapshot");
    let goal_file = unique_path("constraint-approval-goal", "json");
    let pack_file = unique_path("constraint-approval-pack", "json");
    init_git_repo(&workspace);
    fs::write(
        &pack_file,
        r#"{
  "pack_id": "approval-escalation-pack",
  "version": "1",
  "entry_goal": "goal",
  "recommended_verifier_flow": ["generic"],
  "allowed_tools": [{"operation": "run_command", "scope": "workspace"}],
  "verifier_rules": [
    {
      "label": "git-status",
      "profile": "generic",
      "command": { "program": "git", "args": ["status", "--short"] },
      "artifact_expectation": "git status should run only after approval",
      "required": true
    }
  ],
  "approval_mode": "never"
}"#,
    )
    .expect("pack file should be written");
    fs::write(
        &goal_file,
        format!(
            r#"{{
  "summary": "Escalate high-risk verifier command to approval",
  "workspace_root": "/workspace",
  "constraints": [
    {{ "label": "approval_escalation", "detail": "required" }}
  ],
  "done_conditions": [
    {{ "label": "report", "evidence": "report_artifact_exists" }}
  ],
  "verification_checks": [
    {{ "label": "placeholder", "detail": "ls ." }}
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
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "constraint-approval-run",
    );
    let status = run_cli_with_env(
        &["status", "run-1"],
        &[
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
        ],
        "constraint-approval-status",
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(status.status.success(), "stderr:\n{}", stderr_of(&status));
    assert!(stdout_of(&run).contains(
        "phase=waiting_approval outcome=approval_required reason=approval_required_before_execution:constraint_approval_escalation"
    ));
    assert!(stdout_of(&status).contains("approval_state=required"));
    assert!(stdout_of(&status).contains("last_policy=constraint_approval_escalation"));
    assert!(stdout_of(&status).contains("reason_code=approval_required_before_execution"));
    assert!(stdout_of(&status).contains("reason_detail=constraint_approval_escalation"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_file(state_path);
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
            ("AXIOMRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXIOMRUNNER_CODEX_BIN", "/definitely-missing-codex-binary"),
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
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
    assert!(stdout.contains("lock_state="));
    assert!(stdout.contains("command_allowlist="));
    assert!(stdout.contains("constraint_enforcement="));
    assert!(stdout.contains("workspace_path="));
    assert!(stdout.contains("artifact_path="));
    assert!(stdout.contains("memory_path="));
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
            ("AXIOMRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXIOMRUNNER_CODEX_BIN", path_str(&fake_cli)),
            ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
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
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
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
    assert!(json["runtime"]["lock_state"].as_str().is_some());
    assert!(json["runtime"]["latest_pack"].as_str().is_some());
    assert!(json["runtime"]["constraint_enforcement"].as_str().is_some());
    assert_eq!(json["checks"][0]["name"], "workspace_dir");
    assert!(json["paths"]["workspace_path"].as_str().is_some());
    assert!(json["paths"]["artifact_path"].as_str().is_some());
    assert!(json["paths"]["memory_path"].as_str().is_some());
    assert!(json["paths"]["trace_events_path"].as_str().is_some());

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_rejects_unknown_runtime_provider_from_env() {
    let output = run_cli_with_env(
        &["status"],
        &[("AXIOMRUNNER_RUNTIME_PROVIDER", "openrouter")],
        "invalid-provider-env",
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(output.status.code(), Some(5));
    assert!(stdout.is_empty());
    assert!(stderr.contains("runtime init error:"));
    assert!(stderr.contains("unknown runtime provider 'openrouter'"));
    assert!(stderr.contains("AXIOMRUNNER_RUNTIME_PROVIDER"));
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
fn e2e_cli_replay_missing_target_is_runtime_error() {
    let workspace = unique_path("replay-missing-workspace", "dir");
    fs::create_dir_all(&workspace).expect("workspace should exist");

    let replay = run_cli_with_env(
        &["replay", "missing-intent"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
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
fn e2e_cli_status_and_replay_render_aborted_outcome_from_trace() {
    let workspace = unique_path("aborted-trace-workspace", "dir");
    fs::create_dir_all(workspace.join(".axiomrunner/trace")).expect("trace dir should exist");
    let trace = serde_json::json!({
        "schema": "axiomrunner.trace.intent.v1",
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
            "step_journal": [],
            "provider_cwd": "/tmp/aborted-workspace",
            "execution_workspace": "/tmp/aborted-workspace",
            "phase": "aborted",
            "outcome": "aborted",
            "reason": "operator_abort",
            "approval_state": "not_required",
            "verifier_state": "passed",
            "verifier_summary": "operator_abort",
            "elapsed_ms": 0_u64,
            "plan_summary": "intent_id=cli-abort goal outcome=accepted",
            "planned_steps": 4,
            "repair": {
                "attempted": false,
                "attempts": 0_u64,
                "status": "skipped",
                "summary": "not_needed"
            },
            "checkpoint": serde_json::Value::Null,
            "rollback": serde_json::Value::Null
        },
        "artifacts": {
            "plan": ".axiomrunner/artifacts/cli-abort.plan.md",
            "apply": ".axiomrunner/artifacts/cli-abort.apply.md",
            "verify": ".axiomrunner/artifacts/cli-abort.verify.md",
            "report": ".axiomrunner/artifacts/cli-abort.report.md"
        },
        "report_written": true,
        "report_error": serde_json::Value::Null
    });
    fs::write(
        workspace.join(".axiomrunner/trace/events.jsonl"),
        format!(
            "{}\n",
            serde_json::to_string(&trace).expect("trace should serialize")
        ),
    )
    .expect("trace log should be written");

    let status = run_cli_with_env(
        &["status"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        "aborted-status",
    );
    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
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
    assert!(stdout_of(&replay).contains("next_action=decide whether to restart with a new run"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_replay_counts_false_done_runs() {
    let workspace = unique_path("replay-false-done-workspace", "dir");
    fs::create_dir_all(workspace.join(".axiomrunner/trace")).expect("trace dir should exist");
    let trace = serde_json::json!({
        "schema": "axiomrunner.trace.intent.v1",
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
            "step_journal": [],
            "provider_cwd": "/tmp/workspace",
            "execution_workspace": "/tmp/workspace",
            "phase": "failed",
            "outcome": "failed",
            "reason": "done_condition_missing_report_artifact:report",
            "approval_state": "not_required",
            "verifier_state": "failed",
            "verifier_summary": "done_condition_missing_report_artifact:report",
            "elapsed_ms": 0_u64,
            "plan_summary": "intent_id=cli-false-done goal",
            "planned_steps": 4,
            "repair": {
                "attempted": false,
                "attempts": 0_u64,
                "status": "skipped",
                "summary": "verification_passed"
            },
            "checkpoint": serde_json::Value::Null,
            "rollback": serde_json::Value::Null
        },
        "artifacts": {
            "plan": ".axiomrunner/artifacts/cli-false-done.plan.md",
            "apply": ".axiomrunner/artifacts/cli-false-done.apply.md",
            "verify": ".axiomrunner/artifacts/cli-false-done.verify.md",
            "report": ".axiomrunner/artifacts/cli-false-done.report.md"
        },
        "report_written": true,
        "report_error": serde_json::Value::Null
    });
    fs::write(
        workspace.join(".axiomrunner/trace/events.jsonl"),
        format!(
            "{}\n",
            serde_json::to_string(&trace).expect("trace should serialize")
        ),
    )
    .expect("trace log should be written");

    let replay = run_cli_with_env(
        &["replay", "latest"],
        &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
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
            ("AXIOMRUNNER_RUNTIME_PROVIDER", "codek"),
            ("AXIOMRUNNER_CODEX_BIN", "/definitely-missing-codex-binary"),
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
fn e2e_cli_health_reports_blocked_openai_provider_without_api_key() {
    let output = run_cli_with_env(
        &["health"],
        &[("AXIOMRUNNER_RUNTIME_PROVIDER", "openai")],
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
            ("AXIOMRUNNER_RUNTIME_PROVIDER", "openai"),
            ("AXIOMRUNNER_EXPERIMENTAL_OPENAI", "1"),
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
