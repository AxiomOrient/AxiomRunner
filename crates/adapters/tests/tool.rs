use axiomrunner_adapters::{
    AdapterHealth, SearchMode, ToolAdapter, ToolPolicy, ToolRequest, ToolResult, ToolRiskTier,
    WorkspaceTool, classify_tool_request_risk, validate_run_command_spec,
};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn unique_path(label: &str) -> PathBuf {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!(
        "axiomrunner-tool-test-{label}-{}-{tick}",
        std::process::id()
    ))
}

fn tool(root: &PathBuf) -> WorkspaceTool {
    tool_with_allowlist(root, vec![String::from("pwd"), String::from("cat")])
}

fn tool_with_allowlist(root: &PathBuf, command_allowlist: Vec<String>) -> WorkspaceTool {
    WorkspaceTool::new(
        root,
        root,
        ToolPolicy {
            max_file_write_bytes: 16 * 1024,
            max_file_read_bytes: 16 * 1024,
            max_search_results: 32,
            max_command_output_bytes: 256,
            command_timeout_ms: 50,
            command_allowlist,
        },
    )
    .expect("workspace tool should initialize")
}

#[test]
fn workspace_tool_supports_list_read_search_replace_remove_and_run_command() {
    let root = unique_path("surface");
    fs::create_dir_all(&root).expect("workspace should be created");
    let tool = tool(&root);

    assert_eq!(tool.health(), AdapterHealth::Healthy);

    let write = tool
        .execute(ToolRequest::FileWrite {
            path: String::from("notes.txt"),
            contents: String::from("alpha\nbeta\nalpha\n"),
            append: false,
        })
        .expect("file write should succeed");
    let ToolResult::FileWrite(write) = write else {
        panic!("expected file write result");
    };
    assert!(write.evidence.before_digest.is_none());
    assert_eq!(write.evidence.operation, "overwrite");
    assert!(
        !write
            .evidence
            .after_digest
            .as_deref()
            .unwrap_or("")
            .is_empty()
    );
    assert!(write.evidence.after_excerpt.is_some());
    assert!(write.evidence.artifact_path.exists());

    let list = tool
        .execute(ToolRequest::ListFiles {
            path: String::from("."),
        })
        .expect("list files should succeed");
    let ToolResult::ListFiles(list) = list else {
        panic!("expected list files result");
    };
    assert!(list.paths.iter().any(|path| path.ends_with("notes.txt")));

    let read = tool
        .execute(ToolRequest::ReadFile {
            path: String::from("notes.txt"),
        })
        .expect("read file should succeed");
    let ToolResult::ReadFile(read) = read else {
        panic!("expected read file result");
    };
    assert!(read.contents.contains("beta"));

    let search = tool
        .execute(ToolRequest::SearchFiles {
            path: String::from("."),
            needle: String::from("alpha"),
            mode: SearchMode::Substring,
        })
        .expect("search should succeed");
    let ToolResult::SearchFiles(search) = search else {
        panic!("expected search files result");
    };
    assert_eq!(search.matches.len(), 2);
    assert_eq!(search.scanned_files, 1);
    assert_eq!(search.skipped_files, 0);

    let replace = tool
        .execute(ToolRequest::ReplaceInFile {
            path: String::from("notes.txt"),
            needle: String::from("beta"),
            replacement: String::from("gamma"),
            expected_replacements: None,
        })
        .expect("replace should succeed");
    let ToolResult::ReplaceInFile(replace) = replace else {
        panic!("expected replace result");
    };
    assert_eq!(replace.replacements, 1);
    assert!(replace.evidence.before_digest.is_some());
    assert!(replace.evidence.unified_diff.is_some());
    assert!(replace.evidence.artifact_path.exists());

    let run = tool
        .execute(ToolRequest::RunCommand {
            program: String::from("pwd"),
            args: Vec::new(),
        })
        .expect("run command should succeed");
    let ToolResult::RunCommand(run) = run else {
        panic!("expected run command result");
    };
    assert_eq!(run.exit_code, 0);
    assert_eq!(run.profile.as_str(), "generic");
    assert!(!run.stdout.trim().is_empty());
    assert!(!run.stdout_truncated);
    assert!(!run.stderr_truncated);
    assert!(run.artifact_path.exists());

    let remove = tool
        .execute(ToolRequest::RemovePath {
            path: String::from("notes.txt"),
        })
        .expect("remove path should succeed");
    let ToolResult::RemovePath(remove) = remove else {
        panic!("expected remove result");
    };
    assert!(remove.removed);
    assert_eq!(
        remove
            .evidence
            .as_ref()
            .map(|evidence| evidence.operation.as_str()),
        Some("remove")
    );
    assert!(!root.join("notes.txt").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_overwrite_is_atomic_and_leaves_no_temp_file() {
    let root = unique_path("atomic");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(root.join("notes.txt"), "before\n").expect("fixture should be written");

    let tool = tool(&root);
    tool.execute(ToolRequest::FileWrite {
        path: String::from("notes.txt"),
        contents: String::from("after\n"),
        append: false,
    })
    .expect("overwrite should succeed");

    let contents = fs::read_to_string(root.join("notes.txt")).expect("file should be readable");
    assert_eq!(contents, "after\n");

    let entries = fs::read_dir(&root)
        .expect("workspace should be readable")
        .map(|entry| entry.expect("entry should exist").file_name())
        .collect::<Vec<_>>();
    assert!(entries.iter().any(|entry| entry == "notes.txt"));
    assert!(
        entries
            .iter()
            .all(|entry| !entry.to_string_lossy().contains(".tmp-"))
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_writes_patch_artifact_with_before_and_after_digests() {
    let root = unique_path("patch-artifact");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(root.join("notes.txt"), "before\n").expect("fixture should be written");

    let tool = tool(&root);
    let write = tool
        .execute(ToolRequest::FileWrite {
            path: String::from("notes.txt"),
            contents: String::from("after\n"),
            append: false,
        })
        .expect("overwrite should succeed");
    let ToolResult::FileWrite(write) = write else {
        panic!("expected file write result");
    };

    assert!(write.evidence.before_digest.is_some());
    assert!(
        !write
            .evidence
            .after_digest
            .as_deref()
            .unwrap_or("")
            .is_empty()
    );
    let artifact = fs::read_to_string(&write.evidence.artifact_path)
        .expect("patch artifact should be readable");
    assert!(artifact.contains("\"schema\": \"axiomrunner.patch.v2\""));
    assert!(artifact.contains("\"operation\": \"overwrite\""));
    assert!(artifact.contains("\"target_path\": \"notes.txt\""));
    assert!(artifact.contains("\"after_excerpt\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_can_write_artifacts_outside_execution_workspace() {
    let root = unique_path("artifact-separation-workspace");
    let artifact_root = unique_path("artifact-separation-artifacts");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::create_dir_all(&artifact_root).expect("artifact workspace should be created");
    let canonical_artifact_root = fs::canonicalize(&artifact_root).unwrap_or(artifact_root.clone());
    fs::write(root.join("notes.txt"), "before\n").expect("fixture should be written");

    let tool = WorkspaceTool::new(
        &root,
        &artifact_root,
        ToolPolicy {
            max_file_write_bytes: 16 * 1024,
            max_file_read_bytes: 16 * 1024,
            max_search_results: 32,
            max_command_output_bytes: 256,
            command_timeout_ms: 50,
            command_allowlist: vec![String::from("pwd")],
        },
    )
    .expect("workspace tool should initialize");

    let write = tool
        .execute(ToolRequest::FileWrite {
            path: String::from("notes.txt"),
            contents: String::from("after\n"),
            append: false,
        })
        .expect("overwrite should succeed");
    let ToolResult::FileWrite(write) = write else {
        panic!("expected file write result");
    };

    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).expect("target file should be readable"),
        "after\n"
    );
    assert!(
        write
            .evidence
            .artifact_path
            .starts_with(&canonical_artifact_root)
    );
    assert!(!write.evidence.artifact_path.starts_with(&root));

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(artifact_root);
}

#[test]
fn workspace_tool_blocks_workspace_escape_and_denied_command() {
    let root = unique_path("guard");
    fs::create_dir_all(&root).expect("workspace should be created");
    let tool = tool(&root);

    let escape = tool.execute(ToolRequest::ReadFile {
        path: String::from("../escape.txt"),
    });
    assert!(escape.is_err());

    let denied = tool.execute(ToolRequest::RunCommand {
        program: String::from("git"),
        args: vec![String::from("status")],
    });
    assert!(denied.is_err());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_rejects_interpreter_inline_command_even_when_program_is_allowlisted() {
    let root = unique_path("deny-interpreter-inline");
    fs::create_dir_all(&root).expect("workspace should be created");
    let tool = tool_with_allowlist(&root, vec![String::from("python3")]);

    let denied = tool.execute(ToolRequest::RunCommand {
        program: String::from("python3"),
        args: vec![String::from("-c"), String::from("print(1)")],
    });
    assert!(denied.is_err());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn command_contract_allows_safe_python_module_command() {
    let allowed = validate_run_command_spec(
        "python3",
        &[
            String::from("-m"),
            String::from("py_compile"),
            String::from("script.py"),
        ],
        &[String::from("python3")],
    );
    assert!(allowed.is_ok());
}

#[test]
fn golden_workspace_boundary_blocks_escape_reads_and_writes() {
    let root = unique_path("golden-boundary");
    fs::create_dir_all(&root).expect("workspace should be created");
    let tool = tool(&root);

    let read_escape = tool.execute(ToolRequest::ReadFile {
        path: String::from("../escape.txt"),
    });
    let write_escape = tool.execute(ToolRequest::FileWrite {
        path: String::from("../escape.txt"),
        contents: String::from("alpha\n"),
        append: false,
    });

    assert!(read_escape.is_err());
    assert!(write_escape.is_err());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_list_and_search_respect_gitignore() {
    let root = unique_path("gitignore");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(root.join(".gitignore"), "ignored.txt\n").expect("gitignore should be written");
    fs::write(root.join("visible.txt"), "alpha visible\n").expect("visible file should exist");
    fs::write(root.join("ignored.txt"), "alpha ignored\n").expect("ignored file should exist");

    let tool = tool(&root);

    let list = tool
        .execute(ToolRequest::ListFiles {
            path: String::from("."),
        })
        .expect("list should succeed");
    let ToolResult::ListFiles(list) = list else {
        panic!("expected list files result");
    };
    assert!(list.paths.iter().any(|path| path.ends_with("visible.txt")));
    assert!(!list.paths.iter().any(|path| path.ends_with("ignored.txt")));

    let search = tool
        .execute(ToolRequest::SearchFiles {
            path: String::from("."),
            needle: String::from("alpha"),
            mode: SearchMode::Substring,
        })
        .expect("search should succeed");
    let ToolResult::SearchFiles(search) = search else {
        panic!("expected search files result");
    };
    assert!(
        search
            .matches
            .iter()
            .any(|m| m.path.ends_with("visible.txt"))
    );
    assert_eq!(search.scanned_files, 2);
    assert_eq!(search.skipped_files, 0);
    assert!(
        !search
            .matches
            .iter()
            .any(|m| m.path.ends_with("ignored.txt"))
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_supports_regex_search_mode() {
    let root = unique_path("regex-search");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(root.join("notes.txt"), "alpha-01\nbeta-02\nalpha-03\n")
        .expect("fixture should be written");
    let tool = tool(&root);

    let search = tool
        .execute(ToolRequest::SearchFiles {
            path: String::from("."),
            needle: String::from(r"alpha-\d{2}"),
            mode: SearchMode::Regex,
        })
        .expect("regex search should succeed");
    let ToolResult::SearchFiles(search) = search else {
        panic!("expected search files result");
    };
    assert_eq!(search.matches.len(), 2);
    assert_eq!(search.scanned_files, 1);
    assert_eq!(search.skipped_files, 0);

    let invalid = tool.execute(ToolRequest::SearchFiles {
        path: String::from("."),
        needle: String::from("("),
        mode: SearchMode::Regex,
    });
    assert!(invalid.is_err());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_truncates_command_output() {
    let root = unique_path("truncate");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(
        root.join("large.txt"),
        "abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789\n",
    )
    .expect("fixture file should be written");

    let tool = WorkspaceTool::new(
        &root,
        &root,
        ToolPolicy {
            max_file_write_bytes: 16 * 1024,
            max_file_read_bytes: 16 * 1024,
            max_search_results: 32,
            max_command_output_bytes: 64,
            command_timeout_ms: 50,
            command_allowlist: vec![String::from("cat")],
        },
    )
    .expect("workspace tool should initialize");
    let run = tool
        .execute(ToolRequest::RunCommand {
            program: String::from("cat"),
            args: vec![String::from("large.txt")],
        })
        .expect("run command should succeed");
    let ToolResult::RunCommand(run) = run else {
        panic!("expected run command result");
    };
    assert_eq!(run.profile.as_str(), "generic");
    assert!(run.stdout_truncated);
    assert!(run.stdout.contains("[truncated]"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_replace_preserves_crlf_line_endings() {
    let root = unique_path("crlf");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(root.join("notes.txt"), "alpha\r\nbeta\r\n").expect("fixture should be written");

    let tool = tool(&root);
    tool.execute(ToolRequest::ReplaceInFile {
        path: String::from("notes.txt"),
        needle: String::from("beta"),
        replacement: String::from("gamma\nzeta"),
        expected_replacements: None,
    })
    .expect("replace should succeed");

    let contents = fs::read_to_string(root.join("notes.txt")).expect("file should be readable");
    assert!(contents.contains("alpha\r\n"));
    assert!(contents.contains("gamma\r\nzeta\r\n"));
    assert!(!contents.contains("\ngamma\n"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_times_out_long_running_command() {
    let root = unique_path("timeout");
    fs::create_dir_all(&root).expect("workspace should be created");

    let tool = WorkspaceTool::new(
        &root,
        &root,
        ToolPolicy {
            max_file_write_bytes: 16 * 1024,
            max_file_read_bytes: 16 * 1024,
            max_search_results: 32,
            max_command_output_bytes: 64,
            command_timeout_ms: 50,
            command_allowlist: vec![String::from("sleep")],
        },
    )
    .expect("workspace tool should initialize");

    let timed_out = tool.execute(ToolRequest::RunCommand {
        program: String::from("sleep"),
        args: vec![String::from("1")],
    });
    assert!(timed_out.is_err());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_times_out_even_when_stdout_is_flooded() {
    let root = unique_path("timeout-flood");
    fs::create_dir_all(&root).expect("workspace should be created");

    let tool = WorkspaceTool::new(
        &root,
        &root,
        ToolPolicy {
            max_file_write_bytes: 16 * 1024,
            max_file_read_bytes: 16 * 1024,
            max_search_results: 32,
            max_command_output_bytes: 64,
            command_timeout_ms: 100,
            command_allowlist: vec![String::from("yes")],
        },
    )
    .expect("workspace tool should initialize");

    let started = std::time::Instant::now();
    let timed_out = tool.execute(ToolRequest::RunCommand {
        program: String::from("yes"),
        args: Vec::new(),
    });
    assert!(timed_out.is_err());
    assert!(started.elapsed() < Duration::from_millis(800));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_rejects_shell_interpreters_even_if_allowlisted() {
    let root = unique_path("shell-deny");
    fs::create_dir_all(&root).expect("workspace should be created");

    let tool = WorkspaceTool::new(
        &root,
        &root,
        ToolPolicy {
            max_file_write_bytes: 16 * 1024,
            max_file_read_bytes: 16 * 1024,
            max_search_results: 32,
            max_command_output_bytes: 64,
            command_timeout_ms: 100,
            command_allowlist: vec![String::from("sh"), String::from("bash")],
        },
    )
    .expect("workspace tool should initialize");

    let sh = tool.execute(ToolRequest::RunCommand {
        program: String::from("sh"),
        args: vec![String::from("-c"), String::from("echo blocked")],
    });
    let bash = tool.execute(ToolRequest::RunCommand {
        program: String::from("bash"),
        args: vec![String::from("-lc"), String::from("echo blocked")],
    });

    assert!(sh.is_err());
    assert!(bash.is_err());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn golden_stdout_flood_timeout_remains_bounded() {
    let root = unique_path("golden-timeout");
    fs::create_dir_all(&root).expect("workspace should be created");

    let tool = WorkspaceTool::new(
        &root,
        &root,
        ToolPolicy {
            max_file_write_bytes: 16 * 1024,
            max_file_read_bytes: 16 * 1024,
            max_search_results: 32,
            max_command_output_bytes: 64,
            command_timeout_ms: 100,
            command_allowlist: vec![String::from("yes")],
        },
    )
    .expect("workspace tool should initialize");

    let started = std::time::Instant::now();
    let timed_out = tool.execute(ToolRequest::RunCommand {
        program: String::from("yes"),
        args: Vec::new(),
    });

    assert!(timed_out.is_err());
    assert!(started.elapsed() < Duration::from_millis(800));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_rejects_non_utf8_text_mutation() {
    let root = unique_path("encoding");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(root.join("notes.bin"), [0xff_u8, 0xfe_u8, 0xfd_u8])
        .expect("fixture should be written");

    let tool = tool(&root);

    let overwrite = tool.execute(ToolRequest::FileWrite {
        path: String::from("notes.bin"),
        contents: String::from("alpha\n"),
        append: false,
    });
    assert!(overwrite.is_err());

    let replace = tool.execute(ToolRequest::ReplaceInFile {
        path: String::from("notes.bin"),
        needle: String::from("alpha"),
        replacement: String::from("beta"),
        expected_replacements: None,
    });
    assert!(replace.is_err());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_rejects_ambiguous_replace_targets() {
    let root = unique_path("replace-ambiguous");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(root.join("notes.txt"), "alpha\nalpha\n").expect("fixture should be written");

    let tool = tool(&root);
    let replace = tool.execute(ToolRequest::ReplaceInFile {
        path: String::from("notes.txt"),
        needle: String::from("alpha"),
        replacement: String::from("beta"),
        expected_replacements: None,
    });
    let error = replace.expect_err("replace should be rejected");
    assert_eq!(error.retry_class().as_str(), "non_retryable");
    assert!(error.to_string().contains("tool.ambiguous_replace"));

    let contents = fs::read_to_string(root.join("notes.txt")).expect("file should be readable");
    assert_eq!(contents, "alpha\nalpha\n");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_replace_allows_expected_multi_match_replace() {
    let root = unique_path("replace-expected");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(root.join("notes.txt"), "alpha\nalpha\n").expect("fixture should be written");

    let tool = tool(&root);
    let replace = tool
        .execute(ToolRequest::ReplaceInFile {
            path: String::from("notes.txt"),
            needle: String::from("alpha"),
            replacement: String::from("beta"),
            expected_replacements: Some(2),
        })
        .expect("replace should succeed");
    let ToolResult::ReplaceInFile(replace) = replace else {
        panic!("expected replace result");
    };
    assert_eq!(replace.replacements, 2);
    let contents = fs::read_to_string(root.join("notes.txt")).expect("file should be readable");
    assert_eq!(contents, "beta\nbeta\n");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_tool_replace_rejects_expected_count_mismatch() {
    let root = unique_path("replace-count-mismatch");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(root.join("notes.txt"), "alpha\nalpha\n").expect("fixture should be written");

    let tool = tool(&root);
    let replace = tool.execute(ToolRequest::ReplaceInFile {
        path: String::from("notes.txt"),
        needle: String::from("alpha"),
        replacement: String::from("beta"),
        expected_replacements: Some(1),
    });
    let error = replace.expect_err("replace should be rejected");
    assert_eq!(error.retry_class().as_str(), "non_retryable");
    assert!(error.to_string().contains("tool.replace_count_mismatch"));

    let contents = fs::read_to_string(root.join("notes.txt")).expect("file should be readable");
    assert_eq!(contents, "alpha\nalpha\n");

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn workspace_tool_search_reports_unreadable_files_as_skipped() {
    use std::os::unix::fs::PermissionsExt;

    let root = unique_path("search-skipped");
    fs::create_dir_all(&root).expect("workspace should be created");
    fs::write(root.join("visible.txt"), "alpha visible\n").expect("visible file should exist");
    fs::write(root.join("hidden.txt"), "alpha hidden\n").expect("hidden file should exist");
    fs::set_permissions(root.join("hidden.txt"), fs::Permissions::from_mode(0o0))
        .expect("permissions should change");

    let tool = tool(&root);
    let search = tool
        .execute(ToolRequest::SearchFiles {
            path: String::from("."),
            needle: String::from("alpha"),
            mode: SearchMode::Substring,
        })
        .expect("search should succeed");
    let ToolResult::SearchFiles(search) = search else {
        panic!("expected search files result");
    };
    assert_eq!(search.matches.len(), 1);
    assert_eq!(search.scanned_files, 1);
    assert_eq!(search.skipped_files, 1);

    fs::set_permissions(root.join("hidden.txt"), fs::Permissions::from_mode(0o644))
        .expect("permissions should restore");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn tool_risk_tiers_lock_high_risk_operations() {
    assert_eq!(
        classify_tool_request_risk(&ToolRequest::ListFiles {
            path: String::from("."),
        }),
        ToolRiskTier::Low
    );
    assert_eq!(
        classify_tool_request_risk(&ToolRequest::FileWrite {
            path: String::from("notes.txt"),
            contents: String::from("alpha"),
            append: false,
        }),
        ToolRiskTier::Medium
    );
    assert_eq!(
        classify_tool_request_risk(&ToolRequest::RemovePath {
            path: String::from("notes.txt"),
        }),
        ToolRiskTier::High
    );
    assert_eq!(
        classify_tool_request_risk(&ToolRequest::RunCommand {
            program: String::from("git"),
            args: vec![String::from("status")],
        }),
        ToolRiskTier::High
    );
}
