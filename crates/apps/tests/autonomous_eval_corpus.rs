mod common;

use common::{run_cli_with_env, stderr_of, stdout_of};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_path(label: &str, extension: &str) -> PathBuf {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "axiomrunner-eval-{label}-{}-{tick}.{extension}",
        std::process::id()
    ))
}

fn fixture_goal_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("goals")
        .join(name)
}

fn fixture_workspace_template(name: &str) -> Option<PathBuf> {
    let templates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("workspaces");
    match name {
        "rust_service.json" => Some(templates.join("rust_service")),
        "node_api.json" | "nextjs_app.json" => Some(templates.join("node_common")),
        "python_fastapi.json" => Some(templates.join("python_fastapi")),
        _ => None,
    }
}

fn assert_representative_goal_run(
    fixture_name: &str,
    expected_flow: &str,
    expected_done_conditions: usize,
    expected_verifiers: usize,
) {
    let workspace = unique_path(fixture_name, "dir");
    scaffold_fixture_workspace(&workspace, fixture_name);
    let run = run_cli_with_env(
        &[
            "run",
            fixture_goal_path(fixture_name).to_str().expect("utf8 path"),
        ],
        &[(
            "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
            workspace.to_str().expect("utf8 path"),
        )],
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[(
            "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
            workspace.to_str().expect("utf8 path"),
        )],
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    assert!(stdout_of(&run).contains("phase=completed outcome=success"));
    assert!(stdout_of(&replay).contains("replay step id="));

    let plan = fs::read_to_string(workspace.join(".axiomrunner/artifacts/cli-1.plan.md"))
        .expect("plan artifact should exist");
    assert!(plan.contains(&format!("verifier_flow={expected_flow}")));
    assert!(plan.contains(&format!(
        "queued_done_conditions={expected_done_conditions}"
    )));
    assert!(plan.contains(&format!("queued_verifiers={expected_verifiers}")));

    let _ = fs::remove_dir_all(workspace);
}

fn scaffold_fixture_workspace(workspace: &Path, fixture_name: &str) {
    fs::create_dir_all(workspace).expect("workspace should exist");
    if let Some(template) = fixture_workspace_template(fixture_name) {
        copy_dir_all(&template, workspace).expect("fixture template should copy");
    }
}

fn copy_dir_all(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = to.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[test]
fn autonomous_eval_corpus_representative_runs_remain_green() {
    let mut passed = 0usize;
    let total = 12usize;

    {
        let workspace = unique_path("eval-intake-workspace", "dir");
        let run = run_cli_with_env(
            &[
                "run",
                fixture_goal_path("intake.json")
                    .to_str()
                    .expect("utf8 path"),
            ],
            &[(
                "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        let replay = run_cli_with_env(
            &["replay", "run-1"],
            &[(
                "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
        assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
        assert!(stdout_of(&run).contains("phase=completed outcome=success"));
        assert!(stdout_of(&replay).contains("replay step id="));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
    }

    {
        let workspace = unique_path("eval-approval-workspace", "dir");
        let state_path = unique_path("eval-approval-state", "snapshot");
        let run = run_cli_with_env(
            &[
                "run",
                fixture_goal_path("approval.json")
                    .to_str()
                    .expect("utf8 path"),
            ],
            &[
                (
                    "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                    workspace.to_str().expect("utf8 path"),
                ),
                (
                    "AXIOMRUNNER_RUNTIME_STATE_PATH",
                    state_path.to_str().expect("utf8 path"),
                ),
            ],
        );
        let resume = run_cli_with_env(
            &["resume", "run-1"],
            &[
                (
                    "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                    workspace.to_str().expect("utf8 path"),
                ),
                (
                    "AXIOMRUNNER_RUNTIME_STATE_PATH",
                    state_path.to_str().expect("utf8 path"),
                ),
            ],
        );
        assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
        assert!(resume.status.success(), "stderr:\n{}", stderr_of(&resume));
        assert!(stdout_of(&run).contains("approval_required_before_execution"));
        assert!(stdout_of(&resume).contains("phase=completed outcome=success"));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
        let _ = fs::remove_file(state_path);
    }

    {
        let workspace = unique_path("eval-budget-workspace", "dir");
        let run = run_cli_with_env(
            &[
                "run",
                fixture_goal_path("budget_exhausted.json")
                    .to_str()
                    .expect("utf8 path"),
            ],
            &[(
                "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        let replay = run_cli_with_env(
            &["replay", "run-1"],
            &[(
                "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
        assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
        assert!(stdout_of(&run).contains("budget_exhausted_before_execution"));
        assert!(stdout_of(&replay).contains("outcome=budget_exhausted"));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
    }

    {
        let workspace = unique_path("eval-on-risk-workspace", "dir");
        let state_path = unique_path("eval-on-risk-state", "snapshot");
        let run = run_cli_with_env(
            &[
                "run",
                fixture_goal_path("on_risk.json")
                    .to_str()
                    .expect("utf8 path"),
            ],
            &[
                (
                    "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                    workspace.to_str().expect("utf8 path"),
                ),
                (
                    "AXIOMRUNNER_RUNTIME_STATE_PATH",
                    state_path.to_str().expect("utf8 path"),
                ),
            ],
        );
        let resume = run_cli_with_env(
            &["resume", "run-1"],
            &[
                (
                    "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                    workspace.to_str().expect("utf8 path"),
                ),
                (
                    "AXIOMRUNNER_RUNTIME_STATE_PATH",
                    state_path.to_str().expect("utf8 path"),
                ),
            ],
        );
        assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
        assert!(resume.status.success(), "stderr:\n{}", stderr_of(&resume));
        assert!(stdout_of(&run).contains("approval_required_before_execution"));
        assert!(stdout_of(&resume).contains("phase=completed outcome=success"));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
        let _ = fs::remove_file(state_path);
    }

    {
        let workspace = unique_path("eval-lock-workspace", "dir");
        fs::create_dir_all(workspace.join(".axiomrunner")).expect("lock dir should exist");
        fs::write(
            workspace.join(".axiomrunner/runtime.lock"),
            format!("pid={} command=run\n", std::process::id()),
        )
        .expect("lock file should exist");
        let run = run_cli_with_env(
            &[
                "run",
                fixture_goal_path("intake.json")
                    .to_str()
                    .expect("utf8 path"),
            ],
            &[(
                "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        let status = run_cli_with_env(
            &["status"],
            &[(
                "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        assert_eq!(run.status.code(), Some(6), "stderr:\n{}", stderr_of(&run));
        assert!(stderr_of(&run).contains("workspace lock is active"));
        assert!(status.status.success(), "stderr:\n{}", stderr_of(&status));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
    }

    {
        let workspace = unique_path("eval-provider-blocked-workspace", "dir");
        let missing_bin = workspace.join("missing-codex");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        let doctor = run_cli_with_env(
            &["doctor", "--json"],
            &[
                (
                    "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                    workspace.to_str().expect("utf8 path"),
                ),
                ("AXIOMRUNNER_RUNTIME_PROVIDER", "codek"),
                (
                    "AXIOMRUNNER_CODEX_BIN",
                    missing_bin.to_str().expect("utf8 path"),
                ),
            ],
        );
        assert!(doctor.status.success(), "stderr:\n{}", stderr_of(&doctor));
        let doctor_json: serde_json::Value =
            serde_json::from_str(&stdout_of(&doctor)).expect("doctor json should parse");
        assert_eq!(doctor_json["runtime"]["provider_state"], "blocked");
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
    }

    {
        let workspace = unique_path("eval-weak-verifier-workspace", "dir");
        let goal_file = unique_path("eval-weak-verifier-goal", "json");
        fs::write(
            &goal_file,
            r#"{
  "summary": "Surface weak default verifier honestly",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "workspace consistency", "detail": "workspace consistency review" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
        )
        .expect("weak verifier goal should exist");
        let run = run_cli_with_env(
            &["run", goal_file.to_str().expect("utf8 path")],
            &[(
                "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        let replay = run_cli_with_env(
            &["replay", "run-1"],
            &[(
                "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
        assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
        assert!(stdout_of(&run).contains("phase=blocked outcome=blocked"));
        assert!(stdout_of(&run).contains("reason=verification_weak:workspace consistency"));
        assert!(stdout_of(&replay).contains("replay verification status=verification_weak"));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
        let _ = fs::remove_file(goal_file);
    }

    {
        let workspace = unique_path("eval-pack-required-workspace", "dir");
        let goal_file = unique_path("eval-pack-required-goal", "json");
        fs::write(
            &goal_file,
            r#"{
  "summary": "Surface pack required verifier honestly",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "domain verification", "detail": "representative domain path" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
        )
        .expect("pack required goal should exist");
        let run = run_cli_with_env(
            &["run", goal_file.to_str().expect("utf8 path")],
            &[(
                "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        let replay = run_cli_with_env(
            &["replay", "run-1"],
            &[(
                "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
        assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
        assert!(stdout_of(&run).contains("phase=blocked outcome=blocked"));
        assert!(stdout_of(&run).contains("reason=pack_required:domain verification"));
        assert!(stdout_of(&replay).contains("replay verification status=pack_required"));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
        let _ = fs::remove_file(goal_file);
    }

    assert_representative_goal_run("rust_service.json", "build>test>lint", 2, 3);
    passed += 1;

    assert_representative_goal_run("node_api.json", "generic>lint>test>build", 2, 3);
    passed += 1;

    assert_representative_goal_run("nextjs_app.json", "lint>generic>test>build", 2, 4);
    passed += 1;

    assert_representative_goal_run("python_fastapi.json", "generic>test", 2, 2);
    passed += 1;

    assert_eq!(
        passed, total,
        "autonomous eval corpus must keep all representative runs green"
    );
}
