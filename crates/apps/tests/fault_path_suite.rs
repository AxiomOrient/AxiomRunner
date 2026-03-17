mod common;

use common::{run_cli_with_env, stderr_of, stdout_of};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_path(label: &str, extension: &str) -> PathBuf {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "axiomrunner-fault-{label}-{}-{tick}.{extension}",
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

#[test]
fn fault_path_suite_covers_provider_tool_and_workspace_substrates() {
    let mut passed = 0usize;
    let total = 6usize;

    {
        let workspace = unique_path("provider-blocked-workspace", "dir");
        let doctor = run_cli_with_env(
            &["doctor"],
            &[
                ("AXIOMRUNNER_RUNTIME_PROVIDER", "codek"),
                ("AXIOMRUNNER_CODEX_BIN", "/definitely-missing-codex-binary"),
                ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ],
        );
        assert!(doctor.status.success(), "stderr:\n{}", stderr_of(&doctor));
        assert!(stdout_of(&doctor).contains("provider_state=blocked"));
        assert!(stdout_of(&doctor).contains("provider_probe state=fail"));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
    }

    {
        let workspace = unique_path("provider-degraded-workspace", "dir");
        let fake_cli = fake_cli_script("provider-degraded", "codex unknown-version");
        let doctor = run_cli_with_env(
            &["doctor"],
            &[
                ("AXIOMRUNNER_RUNTIME_PROVIDER", "codek"),
                ("AXIOMRUNNER_CODEX_BIN", path_str(&fake_cli)),
                ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
            ],
        );

        assert!(doctor.status.success(), "stderr:\n{}", stderr_of(&doctor));
        assert!(stdout_of(&doctor).contains("provider_state=degraded"));
        assert!(stdout_of(&doctor).contains("provider_probe state=warn"));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
        let _ = fs::remove_file(fake_cli);
    }

    {
        let workspace = unique_path("tool-blocked-workspace", "dir");
        let run = run_cli_with_env(
            &[
                "run",
                fixture_goal_path("intake.json")
                    .to_str()
                    .expect("utf8 path"),
            ],
            &[
                ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
                ("AXIOMRUNNER_RUNTIME_COMMAND_ALLOWLIST", "git"),
            ],
        );
        let replay = run_cli_with_env(
            &["replay", "run-1"],
            &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        );

        assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
        assert!(stdout_of(&run).contains("phase=blocked outcome=budget_exhausted"));
        assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
        assert!(stdout_of(&replay).contains("stage=tool,message=runtime_compose.tool.run_command"));
        assert!(stdout_of(&replay).contains("false_success_intents=1"));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
    }

    {
        let workspace = unique_path("workspace-blocked-workspace", "dir");
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
            &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        );
        let status = run_cli_with_env(
            &["status"],
            &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        );

        assert_eq!(run.status.code(), Some(6), "stderr:\n{}", stderr_of(&run));
        assert!(stderr_of(&run).contains("workspace lock is active"));
        assert!(status.status.success(), "stderr:\n{}", stderr_of(&status));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
    }

    {
        let workspace = unique_path("workspace-stale-lock-workspace", "dir");
        fs::create_dir_all(workspace.join(".axiomrunner")).expect("lock dir should exist");
        fs::write(
            workspace.join(".axiomrunner/runtime.lock"),
            "pid=999999 command=run\n",
        )
        .expect("stale lock file should exist");
        let run = run_cli_with_env(
            &[
                "run",
                fixture_goal_path("intake.json")
                    .to_str()
                    .expect("utf8 path"),
            ],
            &[("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace))],
        );

        assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
        assert!(stdout_of(&run).contains("phase=completed outcome=success"));
        passed += 1;
        let _ = fs::remove_dir_all(workspace);
    }

    {
        let workspace = unique_path("state-persist-workspace", "dir");
        let state_parent_blocker = unique_path("state-parent-blocker", "file");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(&state_parent_blocker, "block").expect("state parent blocker should exist");
        let state_path = state_parent_blocker.join("state.snapshot");

        let run = run_cli_with_env(
            &[
                "run",
                fixture_goal_path("intake.json")
                    .to_str()
                    .expect("utf8 path"),
            ],
            &[
                ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
                ("AXIOMRUNNER_RUNTIME_STATE_PATH", path_str(&state_path)),
            ],
        );

        assert_eq!(run.status.code(), Some(5), "stderr:\n{}", stderr_of(&run));
        assert!(stderr_of(&run).contains("runtime init error:"));
        assert!(stderr_of(&run).contains("read state snapshot"));
        passed += 1;

        let _ = fs::remove_dir_all(workspace);
        let _ = fs::remove_file(state_parent_blocker);
    }

    {
        let workspace = unique_path("trace-persist-workspace", "dir");
        let artifact_workspace = unique_path("trace-persist-artifacts", "dir");
        fs::create_dir_all(workspace.join(".axiomrunner")).expect("workspace should exist");
        fs::create_dir_all(artifact_workspace.join(".axiomrunner"))
            .expect("artifact axiomrunner dir should exist");
        fs::write(artifact_workspace.join(".axiomrunner/trace"), "block")
            .expect("trace blocker should exist");

        let run = run_cli_with_env(
            &[
                "run",
                fixture_goal_path("intake.json")
                    .to_str()
                    .expect("utf8 path"),
            ],
            &[
                ("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", path_str(&workspace)),
                (
                    "AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
                    path_str(&artifact_workspace),
                ),
            ],
        );

        assert_eq!(run.status.code(), Some(6), "stderr:\n{}", stderr_of(&run));
        assert!(stderr_of(&run).contains("runtime execution error:"));
        assert!(stderr_of(&run).contains("runtime trace error:"));
        passed += 1;

        let _ = fs::remove_dir_all(workspace);
        let _ = fs::remove_dir_all(artifact_workspace);
    }

    assert_eq!(
        passed,
        total + 1,
        "fault path suite must keep all substrate failures visible"
    );
}
