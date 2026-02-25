#[path = "../src/parse_util.rs"]
#[allow(dead_code)]
mod parse_util;
#[path = "../src/config_loader.rs"]
#[allow(dead_code)]
mod config_loader;
#[path = "../src/dev_guard.rs"]
#[allow(dead_code)]
mod dev_guard;

mod common;

use common::*;
use config_loader::AppConfig;
use dev_guard::{GuardError, enforce_current_build, enforce_release_gate};

#[test]
fn release_gate_s2_blocks_dev_profile_in_release() {
    let config = AppConfig {
        profile: String::from("dev"),
        endpoint: String::from("http://127.0.0.1:8080"),
        provider: String::from("mock-local"),
        channel: None,
        tools: None,
    };

    let result = enforce_release_gate(&config, true);

    assert_eq!(result, Err(GuardError::DevProfileBlockedInRelease));
}

#[test]
fn release_gate_s2_ignores_non_dev_profile_in_release() {
    let config = AppConfig {
        profile: String::from("prod"),
        endpoint: String::from("http://127.0.0.1:8080"),
        provider: String::from("mock-local"),
        channel: None,
        tools: None,
    };

    let result = enforce_release_gate(&config, true);

    assert!(result.is_ok());
}

#[test]
fn release_gate_s2_treats_dev_profile_case_insensitively() {
    let config = AppConfig {
        profile: String::from("DeV"),
        endpoint: String::from("http://127.0.0.1:8080"),
        provider: String::from("mock-local"),
        channel: None,
        tools: None,
    };

    let result = enforce_release_gate(&config, true);

    assert_eq!(result, Err(GuardError::DevProfileBlockedInRelease));
}

#[test]
fn release_gate_s2_current_build_contract_preserves_dev_minimal_mode() {
    let config = AppConfig {
        profile: String::from("dev"),
        endpoint: String::from("http://127.0.0.1:8080"),
        provider: String::from("mock-local"),
        channel: None,
        tools: None,
    };

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
fn release_gate_s2_cli_build_profile_boundary_is_enforced() {
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
            Some(2),
            "stdout:\n{stdout}\n\nstderr:\n{stderr}"
        );
        assert!(stderr.contains("release gate blocked startup"));
        assert!(stderr.contains("profile=dev is blocked in release builds"));
    }
}

#[test]
fn release_gate_s2_rejects_legacy_cli_bypass_flag() {
    let output = run_cli(&["--profile=dev", "--allow-dev-in-release", "status"]);
    let stderr = stderr_of(&output);

    assert_eq!(output.status.code(), Some(2), "stderr:\n{stderr}");
    assert!(stderr.contains("unknown option '--allow-dev-in-release'"));
}

#[test]
fn release_gate_s2_legacy_env_bypass_signal_does_not_allow_release_startup() {
    let output = run_cli_with_env(
        &["--profile=dev", "status"],
        &[("AXIOM_ALLOW_DEV_IN_RELEASE", "true")],
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
            Some(2),
            "stdout:\n{stdout}\n\nstderr:\n{stderr}"
        );
        assert!(stderr.contains("release gate blocked startup"));
        assert!(stderr.contains("profile=dev is blocked in release builds"));
    }
}

#[test]
fn release_gate_s2_rejects_legacy_file_bypass_key() {
    let config_path = write_temp_config("release_gate_s2", "profile=dev\nallow_dev_in_release=true\n");
    let config_arg = format!("--config-file={}", config_path.display());
    let args = [config_arg.as_str(), "status"];
    let output = run_cli(&args);
    let stderr = stderr_of(&output);

    let _ = std::fs::remove_file(&config_path);

    assert_eq!(output.status.code(), Some(2), "stderr:\n{stderr}");
    assert!(stderr.contains("config error:"));
    assert!(stderr.contains("unknown config key 'allow_dev_in_release'"));
}
