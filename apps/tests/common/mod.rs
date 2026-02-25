#![allow(dead_code)]

use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .args(args)
        .output()
        .expect("axiom_apps binary should run")
}

pub fn run_cli_with_env(args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_axiom_apps"));
    cmd.args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.output().expect("axiom_apps binary should run")
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
        "axiom_apps_{}_{}_{}.cfg",
        label,
        std::process::id(),
        nonce
    ));
    std::fs::write(&path, contents).expect("temporary config file should be created");
    path
}
