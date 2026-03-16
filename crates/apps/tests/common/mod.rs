#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const SANITIZED_ENV_KEYS: &[&str] = &[
    "AXIOMRUNNER_PROFILE",
    "AXIOMRUNNER_RUNTIME_PROVIDER",
    "AXIOMRUNNER_RUNTIME_PROVIDER_MODEL",
    "AXIOMRUNNER_RUNTIME_MAX_TOKENS",
    "AXIOMRUNNER_RUNTIME_MEMORY_PATH",
    "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
    "AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE",
    "AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION",
    "AXIOMRUNNER_RUNTIME_TOOL_LOG_PATH",
    "OPENAI_API_KEY",
];

fn isolated_cli_home(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    path.push(format!(
        "axiomrunner_apps_test_home_{}_{}_{}",
        label,
        std::process::id(),
        nonce
    ));
    std::fs::create_dir_all(&path).expect("isolated home directory should be created");
    path
}

pub fn resolve_cli_bin() -> PathBuf {
    let compiled = PathBuf::from(env!("CARGO_BIN_EXE_axiomrunner_apps"));
    if compiled.is_file() {
        return compiled;
    }

    fallback_cli_bin_from_current_exe().unwrap_or(compiled)
}

pub fn fallback_cli_bin_from_current_exe() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    current
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "deps"))
        .and_then(Path::parent)
        .map(|dir| dir.join(format!("axiomrunner_apps{}", std::env::consts::EXE_SUFFIX)))
        .filter(|path| path.is_file())
}

fn run_with_isolated_env(args: &[&str], env: &[(&str, &str)], label: &str) -> Output {
    let home = isolated_cli_home(label);
    let tool_workspace = home.join(".axiomrunner").join("workspace");
    let artifact_workspace = env
        .iter()
        .find_map(|(key, value)| {
            (*key == "AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE")
                .then(|| std::path::PathBuf::from(*value))
        })
        .unwrap_or_else(|| tool_workspace.clone());
    let mut cmd = Command::new(resolve_cli_bin());
    cmd.env("HOME", &home).args(args);
    for key in SANITIZED_ENV_KEYS {
        cmd.env_remove(key);
    }
    if !env
        .iter()
        .any(|(key, _)| *key == "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE")
    {
        cmd.env("AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE", &tool_workspace);
    }
    if !env
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
        "axiomrunner_apps_{}_{}_{}.cfg",
        label,
        std::process::id(),
        nonce
    ));
    std::fs::write(&path, contents).expect("temporary config file should be created");
    path
}
