use std::process::Command;

fn extract_u32_field(json: &str, field: &str) -> Option<u32> {
    let needle = format!("\"{field}\":");
    let start = json.find(&needle)? + needle.len();
    let digits: String = json[start..]
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<u32>().ok()
}

#[test]
fn h2_parallel() {
    let output = Command::new(env!("CARGO_BIN_EXE_h2_verify"))
        .args([
            "--apps-bin",
            env!("CARGO_BIN_EXE_axiom_apps"),
            "--allowed-diff",
            "0",
        ])
        .output()
        .expect("h2_verify should run");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert_eq!(extract_u32_field(&stdout, "scenario_count"), Some(19));
    assert_eq!(extract_u32_field(&stdout, "diff_count"), Some(0));
}
