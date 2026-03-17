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
#[path = "../src/goal_file.rs"]
#[allow(dead_code)]
mod goal_file;
#[path = "../src/parse_util.rs"]
#[allow(dead_code)]
mod parse_util;

mod common;

use cli_command::{RETAINED_COMMANDS, RETAINED_SURFACE, USAGE};
use common::*;
use config_loader::AppConfig;
use dev_guard::{GuardError, enforce_current_build, enforce_release_gate};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn unique_path(label: &str, extension: &str) -> PathBuf {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "axiomrunner-release-gate-{label}-{}-{tick}.{extension}",
        std::process::id()
    ))
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
        assert!(
            stdout.contains(
                "status revision=0 mode=active last_intent=- last_decision=- last_policy=-"
            )
        );
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
        &[("AXIOMRUNNER_ALLOW_DEV_IN_RELEASE", "true")],
    );
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    if cfg!(debug_assertions) {
        assert!(
            output.status.success(),
            "stdout:\n{stdout}\n\nstderr:\n{stderr}"
        );
        assert!(
            stdout.contains(
                "status revision=0 mode=active last_intent=- last_decision=- last_policy=-"
            )
        );
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
    let changelog = include_str!("../../../CHANGELOG.md");

    for command in *RETAINED_COMMANDS {
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
        assert!(
            changelog.contains(command),
            "changelog missing command: {command}"
        );
    }

    for doc in [readme, runbook] {
        assert!(
            doc.contains(RETAINED_SURFACE),
            "surface doc missing canonical retained surface string"
        );
        assert!(
            !doc.contains("`batch`"),
            "surface doc must not reintroduce legacy batch command"
        );
    }
}

#[test]
fn release_security_gate_identity_surface_is_locked() {
    let workspace_manifest = include_str!("../../../Cargo.toml");
    let apps_manifest = include_str!("../Cargo.toml");
    let core_manifest = include_str!("../../../crates/core/Cargo.toml");
    let adapters_manifest = include_str!("../../../crates/adapters/Cargo.toml");
    let readme = include_str!("../../../README.md");
    let runbook = include_str!("../../../docs/RUNBOOK.md");
    let charter = include_str!("../../../docs/project-charter.md");
    let changelog = include_str!("../../../CHANGELOG.md");
    let nightly = include_str!("../../../scripts/nightly_dogfood.sh");

    for token in [
        "product_name = \"AxiomRunner\"",
        "binary_name = \"axiomrunner_apps\"",
        "env_prefix = \"AXIOMRUNNER\"",
    ] {
        assert!(
            workspace_manifest.contains(token),
            "workspace identity metadata missing token: {token}"
        );
    }

    for manifest in [apps_manifest, core_manifest, adapters_manifest] {
        assert!(
            manifest.contains("description = \"AxiomRunner"),
            "crate manifest missing AxiomRunner description"
        );
    }

    for doc in [readme, runbook, charter, changelog] {
        assert!(
            doc.contains("AxiomRunner"),
            "identity doc missing product name"
        );
        assert!(
            doc.contains("axiomrunner_apps") || doc.contains("AXIOMRUNNER_"),
            "identity doc missing binary or env prefix"
        );
    }

    assert!(
        nightly.contains("axiomrunner_apps") && nightly.contains("AXIOMRUNNER_"),
        "nightly script must keep the canonical binary and env prefix"
    );

    for legacy in ["AxonRunner", "AXONRUNNER_", "axonrunner_apps"] {
        for source in [readme, runbook, charter, changelog, nightly] {
            assert!(
                !source.contains(legacy),
                "identity surface must reject legacy token: {legacy}"
            );
        }
    }
}

#[test]
fn release_security_gate_bridge_docs_lock_autonomous_target_contract() {
    let readme = include_str!("../../../README.md");
    let docs_guide = include_str!("../../../docs/README.md");
    let bridge = include_str!("../../../docs/AUTONOMOUS_AGENT_BRIDGE.md");
    let workflow_pack = include_str!("../../../docs/WORKFLOW_PACK_CONTRACT.md");

    for token in ["run <goal>", "resume", "abort", "goal", "approval", "trace"] {
        assert!(
            bridge.contains(token),
            "bridge docs missing token: {token}"
        );
    }

    for token in [
        "README.md",
        "PROJECT_STRUCTURE.md",
        "bridge",
        "run <goal-file>",
    ] {
        assert!(
            docs_guide.contains(token),
            "docs guide missing token: {token}"
        );
    }

    for doc in [readme, bridge, docs_guide] {
        assert!(
            !doc.contains("docs/transition/README.md"),
            "root docs must not point at removed docs/transition/README.md"
        );
        assert!(
            !doc.contains("docs/roadmap/"),
            "root bridge references must not point at deleted docs/roadmap paths"
        );
    }

    for token in ["pack_id", "allowed_tools", "resume", "abort", "replay"] {
        assert!(
            workflow_pack.contains(token),
            "workflow pack contract missing token: {token}"
        );
    }
}

#[test]
fn release_security_gate_keeps_single_workflow_pack_contract_source() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve");
    let docs_guide = include_str!("../../../docs/README.md");
    let workflow_pack = repo_root.join("docs/WORKFLOW_PACK_CONTRACT.md");

    assert!(
        workflow_pack.is_file(),
        "workflow pack contract doc must exist"
    );
    assert!(
        docs_guide.contains("docs/WORKFLOW_PACK_CONTRACT.md"),
        "docs guide must point to the authoritative workflow pack contract"
    );
    assert!(
        docs_guide.contains("transition mirror는 두지 않는다."),
        "docs guide must state that transition mirror docs are not authoritative"
    );
    assert!(
        !repo_root.join("docs/transition").exists(),
        "legacy docs/transition tree must not reappear"
    );

    let mut contract_headers = Vec::new();
    let mut pack_schema_tokens = Vec::new();
    for path in markdown_files_under(&repo_root.join("docs")) {
        let contents = std::fs::read_to_string(&path).expect("doc should be readable");
        if contents.contains("# Workflow Pack Contract") {
            contract_headers.push(path.clone());
        }
        if contents.contains("recommended_verifier_flow[]") && contents.contains("verifier_rules[]")
        {
            pack_schema_tokens.push(path);
        }
    }

    assert_eq!(
        contract_headers,
        vec![workflow_pack.clone()],
        "workflow pack contract header must live in exactly one docs file"
    );
    assert_eq!(
        pack_schema_tokens,
        vec![workflow_pack],
        "workflow pack schema tokens must live in exactly one docs file"
    );
}

#[test]
fn release_security_gate_forbidden_legacy_tokens_reject_injected_fixture() {
    let fixture = "AxiomRunner replaced batch mode, but AxonRunner gateway docs remain with AXONRUNNER_ prefix";
    let hits = forbidden_legacy_tokens(fixture);

    assert!(hits.contains(&"AxonRunner"));
    assert!(hits.contains(&"AXONRUNNER_"));
}

#[test]
fn release_security_gate_naming_truth_rejects_injected_fixture() {
    let fixture = "product=AxiomRunner binary=axionrunner_apps env=AXIOMRUNNER_";
    let issues = naming_truth_issues(fixture);

    assert!(issues.iter().any(|issue| issue.contains("binary_name")));
}

#[test]
fn release_security_gate_placeholder_verifier_rejects_injected_fixture_and_examples_stay_clean() {
    let fixture = r#"{"command":{"program":"pwd","args":[]}}"#;
    assert!(contains_placeholder_verifier(fixture));

    for contents in [
        include_str!("../../../examples/rust_service/pack.json"),
        include_str!("../../../examples/node_api/pack.json"),
        include_str!("../../../examples/nextjs_app/pack.json"),
        include_str!("../../../examples/python_fastapi/pack.json"),
        include_str!("../../../docs/WORKFLOW_PACK_CONTRACT.md"),
    ] {
        assert!(
            !contains_placeholder_verifier(contents),
            "release surface must not keep placeholder verifier commands"
        );
    }
}

#[test]
fn release_security_gate_false_success_heuristic_rejects_injected_summary() {
    let summary = "ok fixture=x run_rc=0 replay_rc=0 doctor_rc=0 failed_intents=0 false_success_intents=1 false_done_intents=0 weak_verifications=0 unresolved_verifications=0 pack_required_verifications=0";
    assert!(nightly_summary_has_untrusted_green(summary));
}

#[test]
fn release_security_gate_autonomy_evidence_bundle_is_locked() {
    let readme = include_str!("../../../README.md");
    let capability_matrix = include_str!("../../../docs/CAPABILITY_MATRIX.md");
    let runbook = include_str!("../../../docs/RUNBOOK.md");
    let versioning = include_str!("../../../docs/VERSIONING.md");
    let workflow_pack = include_str!("../../../docs/WORKFLOW_PACK_CONTRACT.md");

    for token in [
        "autonomous_eval_corpus",
        "fault_path_suite",
        "nightly_dogfood_contract",
        "release_security_gate",
    ] {
        assert!(
            readme.contains(token) || capability_matrix.contains(token) || runbook.contains(token),
            "autonomy evidence docs missing token: {token}"
        );
    }

    for token in [
        "false_success_intents",
        "false_done_intents",
        "nightly dogfood",
        "fault path suite",
        "verification_weak",
        "verification_unresolved",
        "pack_required",
        "rollback metadata",
    ] {
        assert!(
            capability_matrix.contains(token)
                || runbook.contains(token)
                || versioning.contains(token)
                || workflow_pack.contains(token),
            "autonomy evidence docs missing detail: {token}"
        );
    }
}

#[test]
fn release_security_gate_relative_doc_and_example_paths_exist() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve");
    let docs = [
        ("README.md", include_str!("../../../README.md")),
        ("docs/README.md", include_str!("../../../docs/README.md")),
        ("docs/RUNBOOK.md", include_str!("../../../docs/RUNBOOK.md")),
        (
            "docs/AUTONOMOUS_AGENT_BRIDGE.md",
            include_str!("../../../docs/AUTONOMOUS_AGENT_BRIDGE.md"),
        ),
        (
            "docs/WORKFLOW_PACK_CONTRACT.md",
            include_str!("../../../docs/WORKFLOW_PACK_CONTRACT.md"),
        ),
        (
            "examples/README.md",
            include_str!("../../../examples/README.md"),
        ),
    ];

    for (doc_name, contents) in docs {
        for path in relative_markdown_links(contents) {
            assert!(
                repo_relative_path_exists(&repo_root, &path),
                "{doc_name} references missing markdown link path: {path}"
            );
        }
        for path in repo_path_code_spans(contents) {
            assert!(
                repo_relative_path_exists(&repo_root, &path),
                "{doc_name} references missing repo path: {path}"
            );
        }
    }
}

#[test]
fn release_security_gate_pack_required_goals_block_instead_of_claiming_success() {
    let workspace = unique_path("pack-required-workspace", "dir");
    let goal_file = unique_path("pack-required-goal", "json");
    std::fs::create_dir_all(&workspace).expect("workspace should exist");
    std::fs::write(
        &goal_file,
        r#"{
  "summary": "Need a domain workflow pack",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report_artifact_exists" }
  ],
  "verification_checks": [
    { "label": "domain verification", "detail": "representative domain path" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}"#,
    )
    .expect("goal file should be written");

    let run = run_cli_with_env(
        &["run", goal_file.to_str().expect("utf8 path")],
        &[(
            "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
            workspace.to_str().expect("utf8 path"),
        )],
    );
    let replay = run_cli_with_env(
        &["replay", "run-1"],
        &[(
            "AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE",
            workspace.to_str().expect("utf8 path"),
        )],
    );

    assert!(run.status.success(), "stderr:\n{}", stderr_of(&run));
    assert!(replay.status.success(), "stderr:\n{}", stderr_of(&replay));
    assert!(stdout_of(&run).contains("phase=blocked outcome=blocked"));
    assert!(stdout_of(&run).contains("reason=pack_required:domain verification"));
    assert!(stdout_of(&replay).contains("replay verification status=pack_required"));

    let _ = std::fs::remove_dir_all(workspace);
    let _ = std::fs::remove_file(goal_file);
}

#[test]
fn release_security_gate_nightly_summary_keeps_quality_metrics() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = repo_root.join("../../scripts/nightly_dogfood.sh");
    let log_root = unique_path("nightly-logs", "dir");
    let timestamp = "20260314T010000Z";

    let output = std::process::Command::new("sh")
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

    assert!(
        output.status.success(),
        "stdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let summary =
        std::fs::read_to_string(log_root.join(timestamp).join("summary.txt")).expect("summary");
    assert!(summary.contains("failed_intents=0"));
    assert!(summary.contains("false_success_intents=0"));
    assert!(summary.contains("false_done_intents=0"));
    assert!(summary.contains("weak_verifications=0"));
    assert!(summary.contains("unresolved_verifications=0"));
    assert!(summary.contains("pack_required_verifications=0"));

    let _ = std::fs::remove_dir_all(log_root);
}

#[test]
fn release_security_gate_representative_and_rollback_evidence_bundle_stays_locked() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve");
    let e2e = include_str!("e2e_cli.rs");
    let runbook = include_str!("../../../docs/RUNBOOK.md");

    for path in [
        "crates/apps/tests/fixtures/goals/rust_service.json",
        "crates/apps/tests/fixtures/goals/node_api.json",
        "crates/apps/tests/fixtures/goals/nextjs_app.json",
        "crates/apps/tests/fixtures/goals/python_fastapi.json",
        "crates/apps/tests/fixtures/packs/rust_service.json",
        "crates/apps/tests/fixtures/packs/node_api.json",
        "crates/apps/tests/fixtures/packs/nextjs_app.json",
        "crates/apps/tests/fixtures/packs/python_fastapi.json",
        "examples/rust_service/goal.json",
        "examples/node_api/goal.json",
        "examples/nextjs_app/goal.json",
        "examples/python_fastapi/goal.json",
    ] {
        assert!(
            repo_relative_path_exists(&repo_root, path),
            "representative evidence path missing: {path}"
        );
    }

    for token in [
        "rollback metadata should exist",
        "replay rollback metadata=",
        "cli-1.rollback.json",
    ] {
        assert!(
            e2e.contains(token),
            "rollback evidence contract missing token: {token}"
        );
    }
    assert!(
        runbook.contains("rollback.json"),
        "runbook must keep rollback recovery instructions"
    );
}

fn relative_markdown_links(contents: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let bytes = contents.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] != b']' || index + 1 >= bytes.len() || bytes[index + 1] != b'(' {
            index += 1;
            continue;
        }
        let start = index + 2;
        let Some(end_rel) = contents[start..].find(')') else {
            break;
        };
        let raw = contents[start..start + end_rel].trim();
        if is_repo_relative_path(raw) {
            paths.push(raw.to_owned());
        }
        index = start + end_rel + 1;
    }

    paths
}

fn repo_path_code_spans(contents: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut in_span = false;
    let mut current = String::new();

    for ch in contents.chars() {
        if ch == '`' {
            if in_span {
                let value = current.trim();
                if is_repo_relative_path(value) {
                    paths.push(value.to_owned());
                }
                current.clear();
            }
            in_span = !in_span;
            continue;
        }
        if in_span {
            current.push(ch);
        }
    }

    paths
}

fn is_repo_relative_path(value: &str) -> bool {
    if value.is_empty()
        || value.starts_with('#')
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("mailto:")
        || value.contains(' ')
        || value.contains('<')
        || value.contains('>')
    {
        return false;
    }

    matches!(
        value,
        "README.md"
            | "CHANGELOG.md"
            | "Cargo.toml"
            | "Cargo.lock"
            | _ if value.starts_with("docs/")
                || value.starts_with("examples/")
                || value.starts_with("scripts/")
                || value.starts_with("crates/")
    )
}

fn repo_relative_path_exists(repo_root: &Path, relative: &str) -> bool {
    repo_root.join(relative).exists()
}

fn markdown_files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !root.exists() {
        return files;
    }

    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let entries = std::fs::read_dir(&path).expect("directory should be readable");
        for entry in entries {
            let entry = entry.expect("directory entry should be readable");
            let entry_path = entry.path();
            if entry_path.is_dir() {
                pending.push(entry_path);
                continue;
            }
            if entry_path.extension().and_then(|value| value.to_str()) == Some("md") {
                files.push(entry_path);
            }
        }
    }

    files.sort();
    files
}

fn forbidden_legacy_tokens(contents: &str) -> Vec<&'static str> {
    let tokens = ["AxonRunner", "AXONRUNNER_", "`batch`", "docs/transition/"];
    tokens
        .into_iter()
        .filter(|token| contents.contains(token))
        .collect()
}

fn naming_truth_issues(contents: &str) -> Vec<String> {
    let mut issues = Vec::new();
    if contents.contains("AxiomRunner") && !contents.contains("axiomrunner_apps") {
        issues.push(String::from("binary_name missing axiomrunner_apps"));
    }
    if contents.contains("AxiomRunner") && !contents.contains("AXIOMRUNNER_") {
        issues.push(String::from("env_prefix missing AXIOMRUNNER_"));
    }
    if contents.contains("axionrunner_apps") {
        issues.push(String::from("binary_name typo axionrunner_apps"));
    }
    issues
}

fn contains_placeholder_verifier(contents: &str) -> bool {
    contents.contains("\"program\":\"pwd\"")
        || contents.contains("\"program\": \"pwd\"")
        || contents.contains("program = \"pwd\"")
}

fn nightly_summary_has_untrusted_green(contents: &str) -> bool {
    summary_metric(contents, "false_success_intents").is_some_and(|value| value > 0)
        || summary_metric(contents, "false_done_intents").is_some_and(|value| value > 0)
}

fn summary_metric(contents: &str, key: &str) -> Option<u64> {
    let prefix = format!("{key}=");
    contents
        .split_whitespace()
        .find_map(|part| part.strip_prefix(&prefix))
        .and_then(|raw| raw.parse::<u64>().ok())
}
