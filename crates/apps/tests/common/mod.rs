#![allow(dead_code)]

use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const SANITIZED_ENV_KEYS: &[&str] = &[
    "AXONRUNNER_RUNTIME_MEMORY_PATH",
    "AXONRUNNER_RUNTIME_TOOL_WORKSPACE",
    "AXONRUNNER_CHANNEL_STORE_PATH",
    "AXONRUNNER_DAEMON_HEALTH_PATH",
    "AXONRUNNER_DAEMON_HEALTH_STATE_PATH",
    "AXONRUNNER_SERVICE_STATE_PATH",
    "AXONRUNNER_ONBOARD_STATE_PATH",
    "AXONRUNNER_ONBOARD_WORKSPACE_PATH",
];

fn isolated_cli_home(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    path.push(format!(
        "axonrunner_apps_test_home_{}_{}_{}",
        label,
        std::process::id(),
        nonce
    ));
    std::fs::create_dir_all(&path).expect("isolated home directory should be created");
    path
}

fn run_with_isolated_env(args: &[&str], env: &[(&str, &str)], label: &str) -> Output {
    let home = isolated_cli_home(label);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_axonrunner_apps"));
    cmd.env("HOME", &home).args(args);
    for key in SANITIZED_ENV_KEYS {
        cmd.env_remove(key);
    }
    for (key, value) in env {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("axonrunner_apps binary should run");
    let _ = std::fs::remove_dir_all(&home);
    output
}

pub fn run_cli(args: &[&str]) -> Output {
    run_with_isolated_env(args, &[], "default")
}

pub fn run_cli_with_env(args: &[&str], env: &[(&str, &str)]) -> Output {
    run_with_isolated_env(args, env, "with_env")
}

pub fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

pub fn stderr_of(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

pub fn write_temp_config(label: &str, contents: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    path.push(format!(
        "axonrunner_apps_{}_{}_{}.cfg",
        label,
        std::process::id(),
        nonce
    ));
    std::fs::write(&path, contents).expect("temporary config file should be created");
    path
}
