use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn run_perf(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_perf_suite"))
        .args(args)
        .output()
        .expect("perf_suite binary should run")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

fn unique_report_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!("axonrunner_perf_suite_{nonce}.json"))
}

#[test]
fn perf_suite_smoke_stdout_json() {
    let output = run_perf(&["--iterations", "2", "--records", "4", "--warmup", "1"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");

    let trimmed = stdout.trim();
    assert!(trimmed.starts_with('{') && trimmed.ends_with('}'));
    assert!(trimmed.contains("\"suite\":\"perf_suite_v1\""));
    assert!(trimmed.contains("\"core_reduce_path\""));
    assert!(trimmed.contains("\"memory_recall_path\""));
    assert!(trimmed.contains("\"gateway_validation_request_path\""));
    assert!(trimmed.contains("\"channel_serve_path\""));
    assert!(trimmed.contains("\"queue_peak_depth\":"));
    assert!(trimmed.contains("\"p50_ns_per_iteration\":"));
    assert!(trimmed.contains("\"p95_ns_per_iteration\":"));
    assert!(trimmed.contains("\"p50_ns_per_operation\":"));
    assert!(trimmed.contains("\"p95_ns_per_operation\":"));
}

#[test]
fn perf_suite_smoke_file_json() {
    let output_path = unique_report_path();
    let output_path_str = output_path
        .to_str()
        .expect("temp output path should be valid UTF-8");

    let output = run_perf(&[
        "--iterations",
        "1",
        "--records",
        "3",
        "--warmup",
        "0",
        "--output",
        output_path_str,
    ]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.trim().is_empty(), "stdout should be empty: {stdout}");
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");

    let report = std::fs::read_to_string(&output_path).expect("output file should exist");
    assert!(report.trim().starts_with('{') && report.trim().ends_with('}'));
    assert!(report.contains("\"results\":["));
    assert!(report.contains("\"queue_peak_depth\":"));
    assert!(report.contains("\"p50_ns_per_iteration\":"));
    assert!(report.contains("\"p95_ns_per_iteration\":"));
    assert!(report.contains("\"p50_ns_per_operation\":"));
    assert!(report.contains("\"p95_ns_per_operation\":"));

    let _ = std::fs::remove_file(&output_path);
}
