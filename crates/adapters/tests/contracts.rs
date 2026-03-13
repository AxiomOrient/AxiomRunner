use axonrunner_adapters::{
    FileMutationEvidence, FileWriteOutput, ListFilesOutput, ReadFileOutput, RemovePathOutput,
    ReplaceInFileOutput, RunCommandOutput, RunCommandProfile, SearchFilesOutput, SearchMatch,
    SearchMode, ToolRequest, ToolResult, WorkflowPackAllowedTool, WorkflowPackContract,
    WorkflowPackRiskPolicy, WorkflowPackVerifierRule,
};
use std::path::PathBuf;

#[test]
fn tool_contract_docs_lock_request_output_and_evidence_schema() {
    let docs = include_str!("../../../docs/CAPABILITY_MATRIX.md");

    for token in [
        "list_files",
        "read_file",
        "search_files",
        "file_write",
        "replace_in_file",
        "remove_path",
        "run_command",
        "workspace boundary",
        "allowlist",
        "timeout",
        "artifact_path",
        "before_digest",
        "after_digest",
        "unified_diff",
    ] {
        assert!(
            docs.contains(token),
            "tool contract docs missing token: {token}"
        );
    }
}

#[test]
fn tool_contract_request_and_result_shapes_stay_explicit() {
    let request = ToolRequest::SearchFiles {
        path: String::from("."),
        needle: String::from("alpha"),
        mode: SearchMode::Regex,
    };
    let result = ToolResult::SearchFiles(SearchFilesOutput {
        base: PathBuf::from("/workspace"),
        matches: vec![SearchMatch {
            path: PathBuf::from("/workspace/notes.txt"),
            line_number: 3,
            line: String::from("alpha-03"),
        }],
        scanned_files: 1,
        skipped_files: 0,
    });

    match request {
        ToolRequest::SearchFiles { path, needle, mode } => {
            assert_eq!(path, ".");
            assert_eq!(needle, "alpha");
            assert_eq!(mode, SearchMode::Regex);
        }
        _ => panic!("expected search request"),
    }

    match result {
        ToolResult::SearchFiles(output) => {
            assert_eq!(output.base, PathBuf::from("/workspace"));
            assert_eq!(output.matches.len(), 1);
            assert_eq!(output.matches[0].line_number, 3);
            assert_eq!(output.scanned_files, 1);
            assert_eq!(output.skipped_files, 0);
        }
        _ => panic!("expected search result"),
    }
}

#[test]
fn tool_contract_mutation_outputs_keep_evidence_fields() {
    let evidence = FileMutationEvidence {
        operation: String::from("overwrite"),
        artifact_path: PathBuf::from(".axonrunner/patches/notes.json"),
        before_digest: Some(String::from("before")),
        after_digest: Some(String::from("after")),
        before_excerpt: Some(String::from("old")),
        after_excerpt: Some(String::from("new")),
        unified_diff: Some(String::from("--- before\\n+++ after")),
    };

    let write = FileWriteOutput {
        path: PathBuf::from("/workspace/notes.txt"),
        bytes_written: 12,
        evidence: evidence.clone(),
    };
    let replace = ReplaceInFileOutput {
        path: PathBuf::from("/workspace/notes.txt"),
        replacements: 1,
        evidence: evidence.clone(),
    };
    let remove = RemovePathOutput {
        path: PathBuf::from("/workspace/notes.txt"),
        removed: true,
        evidence: Some(evidence.clone()),
    };
    let read = ReadFileOutput {
        path: PathBuf::from("/workspace/notes.txt"),
        contents: String::from("alpha"),
    };
    let list = ListFilesOutput {
        base: PathBuf::from("/workspace"),
        paths: vec![PathBuf::from("/workspace/notes.txt")],
    };
    let run = RunCommandOutput {
        program: String::from("cargo"),
        args: vec![String::from("test")],
        profile: RunCommandProfile::Test,
        exit_code: 0,
        stdout: String::from("ok"),
        stderr: String::new(),
        stdout_truncated: false,
        stderr_truncated: false,
        artifact_path: PathBuf::from(".axonrunner/commands/cargo-test.json"),
    };

    assert_eq!(write.evidence.operation, "overwrite");
    assert_eq!(
        replace.evidence.unified_diff.as_deref(),
        Some("--- before\\n+++ after")
    );
    assert_eq!(
        remove
            .evidence
            .as_ref()
            .and_then(|entry| entry.after_digest.as_deref()),
        Some("after")
    );
    assert_eq!(read.contents, "alpha");
    assert_eq!(list.paths.len(), 1);
    assert_eq!(run.program, "cargo");
    assert_eq!(run.profile, RunCommandProfile::Test);
    assert!(!run.stdout_truncated);
}

#[test]
fn tool_contract_replace_request_shape_stays_explicit() {
    let request = ToolRequest::ReplaceInFile {
        path: String::from("notes.txt"),
        needle: String::from("alpha"),
        replacement: String::from("beta"),
        expected_replacements: Some(2),
    };

    match request {
        ToolRequest::ReplaceInFile {
            path,
            needle,
            replacement,
            expected_replacements,
        } => {
            assert_eq!(path, "notes.txt");
            assert_eq!(needle, "alpha");
            assert_eq!(replacement, "beta");
            assert_eq!(expected_replacements, Some(2));
        }
        _ => panic!("expected replace request"),
    }
}

#[test]
fn workflow_pack_contract_docs_lock_manifest_and_ownership_rules() {
    let docs = include_str!("../../../docs/transition/WORKFLOW_PACK_CONTRACT.md");

    for token in [
        "pack_id",
        "version",
        "entry_goal",
        "planner_hints",
        "allowed_tools",
        "verifier_rules",
        "risk_policy",
        "resume",
        "abort",
        "status",
        "replay",
    ] {
        assert!(
            docs.contains(token),
            "workflow pack docs missing token: {token}"
        );
    }
}

#[test]
fn workflow_pack_contract_shape_stays_explicit() {
    let contract = WorkflowPackContract {
        pack_id: String::from("rust-service-basic"),
        version: String::from("1"),
        description: String::from("bounded Rust service workflow"),
        entry_goal: String::from("implement one bounded Rust service task"),
        planner_hints: vec![String::from("prefer cargo-first verification")],
        allowed_tools: vec![
            WorkflowPackAllowedTool {
                operation: String::from("read_file"),
                scope: String::from("workspace"),
            },
            WorkflowPackAllowedTool {
                operation: String::from("run_command"),
                scope: String::from("workspace"),
            },
        ],
        verifier_rules: vec![WorkflowPackVerifierRule {
            label: String::from("cargo test"),
            profile: RunCommandProfile::Test,
            command_example: String::from("cargo test"),
            artifact_expectation: String::from("test artifact exists"),
            required: true,
        }],
        risk_policy: WorkflowPackRiskPolicy {
            approval_mode: String::from("on-risk"),
            max_mutating_steps: 8,
        },
    };

    assert_eq!(contract.validate(), Ok(()));
    assert_eq!(contract.allowed_tools[0].operation, "read_file");
    assert_eq!(contract.verifier_rules[0].profile, RunCommandProfile::Test);
    assert_eq!(contract.risk_policy.approval_mode, "on-risk");
}
