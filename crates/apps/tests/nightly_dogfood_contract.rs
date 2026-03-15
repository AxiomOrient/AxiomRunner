use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_path(label: &str, extension: &str) -> std::path::PathBuf {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "axiomrunner-nightly-{label}-{}-{tick}.{extension}",
        std::process::id()
    ))
}

#[test]
fn nightly_dogfood_script_writes_log_bundle_for_one_fixture() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = repo_root.join("../../scripts/nightly_dogfood.sh");
    let log_root = unique_path("logs", "dir");
    let timestamp = "20260314T000000Z";

    let output = Command::new("sh")
        .arg(&script)
        .env(
            "AXIOMRUNNER_NIGHTLY_BIN",
            env!("CARGO_BIN_EXE_axiomrunner_apps"),
        )
        .env("AXIOMRUNNER_NIGHTLY_SKIP_BUILD", "1")
        .env("AXIOMRUNNER_NIGHTLY_FIXTURES", "rust_service.json")
        .env("AXIOMRUNNER_NIGHTLY_LOG_ROOT", &log_root)
        .env("AXIOMRUNNER_NIGHTLY_TIMESTAMP", timestamp)
        .output()
        .expect("nightly dogfood script should run");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );

    let run_root = log_root.join(timestamp);
    let summary = fs::read_to_string(run_root.join("summary.txt")).expect("summary should exist");
    assert!(summary.contains("fixture=rust_service.json"));
    assert!(summary.contains("run_rc=0"));
    assert!(summary.contains("replay_rc=0"));
    assert!(summary.contains("doctor_rc=0"));
    assert!(summary.contains("failed_intents=0"));
    assert!(summary.contains("false_success_intents=0"));
    assert!(summary.contains("false_done_intents=0"));
    assert!(summary.contains("failures=0"));

    assert!(run_root.join("logs/rust_service.run.stdout.log").exists());
    assert!(
        run_root
            .join("logs/rust_service.replay.stdout.log")
            .exists()
    );
    assert!(run_root.join("logs/rust_service.doctor.json").exists());

    let _ = fs::remove_dir_all(log_root);
}
