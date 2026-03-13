#[path = "../src/cli_command.rs"]
#[allow(dead_code)]
mod cli_command;
#[path = "../src/config_loader.rs"]
#[allow(dead_code)]
mod config_loader;
#[path = "../src/dev_guard.rs"]
#[allow(dead_code)]
mod dev_guard;
#[path = "../src/env_util.rs"]
#[allow(dead_code)]
mod env_util;
#[path = "../src/parse_util.rs"]
#[allow(dead_code)]
mod parse_util;

mod common;

use cli_command::USAGE;
use common::*;
use config_loader::AppConfig;
use dev_guard::{GuardError, enforce_current_build, enforce_release_gate};

fn mock_config(profile: &str) -> AppConfig {
    AppConfig {
        profile: String::from(profile),
        provider: String::from("mock-local"),
        provider_model: None,
        workspace: None,
        state_path: None,
        command_allowlist: None,
    }
}

#[test]
fn release_security_gate_blocks_dev_profile_in_release() {
    let config = mock_config("dev");

    let result = enforce_release_gate(&config, true);

    assert_eq!(result, Err(GuardError::DevProfileBlockedInRelease));
}

#[test]
fn release_security_gate_ignores_non_dev_profile_in_release() {
    let config = mock_config("prod");

    let result = enforce_release_gate(&config, true);

    assert!(result.is_ok());
}

#[test]
fn release_security_gate_treats_dev_profile_case_insensitively() {
    let config = mock_config("DeV");

    let result = enforce_release_gate(&config, true);

    assert_eq!(result, Err(GuardError::DevProfileBlockedInRelease));
}

#[test]
fn release_security_gate_current_build_contract_preserves_dev_minimal_mode() {
    let config = mock_config("dev");

    let result = enforce_current_build(&config);

    if cfg!(debug_assertions) {
        assert!(
            result.is_ok(),
            "debug builds should keep dev-minimal mode permissive"
        );
    } else {
        assert_eq!(result, Err(GuardError::DevProfileBlockedInRelease));
    }
}

#[test]
fn release_security_gate_cli_build_profile_boundary_is_enforced() {
    let output = run_cli(&["--profile=dev", "status"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    if cfg!(debug_assertions) {
        assert!(
            output.status.success(),
            "stdout:\n{stdout}\n\nstderr:\n{stderr}"
        );
        assert!(stdout.contains("status revision=0 mode=active facts=0 denied=0 audit=0"));
    } else {
        assert_eq!(
            output.status.code(),
            Some(4),
            "stdout:\n{stdout}\n\nstderr:\n{stderr}"
        );
        assert!(stderr.contains("release gate error:"));
        assert!(stderr.contains("profile=dev is blocked in release builds"));
    }
}

#[test]
fn release_security_gate_rejects_legacy_cli_bypass_flag() {
    let output = run_cli(&["--profile=dev", "--allow-dev-in-release", "status"]);
    let stderr = stderr_of(&output);

    assert_eq!(output.status.code(), Some(2), "stderr:\n{stderr}");
    assert!(stderr.contains("parse error:"));
    assert!(stderr.contains("unknown option '--allow-dev-in-release'"));
}

#[test]
fn release_security_gate_legacy_env_bypass_signal_does_not_allow_release_startup() {
    let output = run_cli_with_env(
        &["--profile=dev", "status"],
        &[("AXONRUNNER_ALLOW_DEV_IN_RELEASE", "true")],
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    if cfg!(debug_assertions) {
        assert!(
            output.status.success(),
            "stdout:\n{stdout}\n\nstderr:\n{stderr}"
        );
        assert!(stdout.contains("status revision=0 mode=active facts=0 denied=0 audit=0"));
    } else {
        assert_eq!(
            output.status.code(),
            Some(4),
            "stdout:\n{stdout}\n\nstderr:\n{stderr}"
        );
        assert!(stderr.contains("release gate error:"));
        assert!(stderr.contains("profile=dev is blocked in release builds"));
    }
}

#[test]
fn release_security_gate_rejects_legacy_file_bypass_key() {
    let config_path = write_temp_config(
        "release_security_gate",
        "profile=dev\nallow_dev_in_release=true\n",
    );
    let config_arg = format!("--config-file={}", config_path.display());
    let args = [config_arg.as_str(), "status"];
    let output = run_cli(&args);
    let stderr = stderr_of(&output);

    let _ = std::fs::remove_file(&config_path);

    assert_eq!(output.status.code(), Some(3), "stderr:\n{stderr}");
    assert!(stderr.contains("config error:"));
    assert!(stderr.contains("unknown config key 'allow_dev_in_release'"));
}

#[test]
fn release_security_gate_truth_surface_docs_match_retained_commands() {
    let readme = include_str!("../../../README.md");
    let capability_matrix = include_str!("../../../docs/CAPABILITY_MATRIX.md");
    let runbook = include_str!("../../../docs/RUNBOOK.md");
    let charter = include_str!("../../../docs/project-charter.md");

    for command in [
        "run", "batch", "doctor", "replay", "status", "health", "help",
    ] {
        assert!(
            USAGE.contains(command),
            "cli usage missing command: {command}"
        );
        assert!(
            readme.contains(command),
            "README missing command: {command}"
        );
        assert!(
            capability_matrix.contains(command),
            "capability matrix missing command: {command}"
        );
        assert!(
            runbook.contains(command),
            "runbook missing command: {command}"
        );
        assert!(
            charter.contains(command),
            "charter missing command: {command}"
        );
    }
}
