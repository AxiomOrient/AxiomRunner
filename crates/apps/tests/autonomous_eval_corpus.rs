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
        "axonrunner-eval-{label}-{}-{tick}.{extension}",
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

fn assert_representative_goal_run(
    fixture_name: &str,
    expected_flow: &str,
    expected_done_conditions: usize,
    expected_verifiers: usize,
) {
    let workspace = unique_path(fixture_name, "dir");
    let run = run_cli_with_env(
        &[
            "run",
            fixture_goal_path(fixture_name).to_str().expect("utf8 path"),
        ],
        &[(
            "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
            workspace.to_str().expect("utf8 path"),
        )],
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[(
            "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
            workspace.to_str().expect("utf8 path"),
        )],
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    assert!(stdout_of(&run).contains("phase=completed outcome=success"));
    assert!(stdout_of(&replay).contains("replay step id="));

    let plan = fs::read_to_string(workspace.join(".axonrunner/artifacts/cli-1.plan.md"))
        .expect("plan artifact should exist");
    assert!(plan.contains(&format!("verifier_flow={expected_flow}")));
    assert!(plan.contains(&format!(
        "queued_done_conditions={expected_done_conditions}"
    )));
    assert!(plan.contains(&format!("queued_verifiers={expected_verifiers}")));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn autonomous_eval_corpus_representative_runs_remain_green() {
    let mut passed = 0usize;
    let total = 9usize;

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
                "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        let replay = run_cli_with_env(
            &["replay", "run-1"],
            &[(
                "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
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
                    "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
                    workspace.to_str().expect("utf8 path"),
                ),
                (
                    "AXONRUNNER_RUNTIME_STATE_PATH",
                    state_path.to_str().expect("utf8 path"),
                ),
            ],
        );
        let resume = run_cli_with_env(
            &["resume", "run-1"],
            &[
                (
                    "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
                    workspace.to_str().expect("utf8 path"),
                ),
                (
                    "AXONRUNNER_RUNTIME_STATE_PATH",
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
                "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        let replay = run_cli_with_env(
            &["replay", "run-1"],
            &[(
                "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
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
                    "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
                    workspace.to_str().expect("utf8 path"),
                ),
                (
                    "AXONRUNNER_RUNTIME_STATE_PATH",
                    state_path.to_str().expect("utf8 path"),
                ),
            ],
        );
        let resume = run_cli_with_env(
            &["resume", "run-1"],
            &[
                (
                    "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
                    workspace.to_str().expect("utf8 path"),
                ),
                (
                    "AXONRUNNER_RUNTIME_STATE_PATH",
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
        fs::create_dir_all(workspace.join(".axonrunner")).expect("lock dir should exist");
        fs::write(
            workspace.join(".axonrunner/runtime.lock"),
            "pid=999 command=run\n",
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
                "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        let status = run_cli_with_env(
            &["status"],
            &[(
                "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
                workspace.to_str().expect("utf8 path"),
            )],
        );
        assert_eq!(run.status.code(), Some(6), "stderr:\n{}", stderr_of(&run));
        assert!(stderr_of(&run).contains("workspace lock is active"));
        assert!(status.status.success(), "stderr:\n{}", stderr_of(&status));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
    }

    assert_representative_goal_run("rust_service.json", "build>test>lint", 2, 3);
    passed += 1;

    assert_representative_goal_run("node_api.json", "build>test>lint", 2, 3);
    passed += 1;

    assert_representative_goal_run("nextjs_app.json", "build>test>lint>generic", 2, 4);
    passed += 1;

    assert_representative_goal_run("python_fastapi.json", "test>generic", 2, 2);
    passed += 1;

    assert_eq!(
        passed, total,
        "autonomous eval corpus must keep all representative runs green"
    );
}
