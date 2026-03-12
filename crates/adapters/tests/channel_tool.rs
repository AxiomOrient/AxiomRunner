use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use axonrunner_adapters::{channel, tool};

struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    fn new(label: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "axonrunner_adapters_channel_tool_{label}_{}_{}",
            std::process::id(),
            timestamp
        ));
        fs::create_dir_all(&path).expect("temp workspace should be creatable");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn allow_all_policy(limit: usize) -> tool::ToolPolicy {
    tool::ToolPolicy {
        allow_shell: true,
        allow_file_read: true,
        allow_file_write: true,
        max_shell_command_bytes: limit,
        max_file_read_bytes: limit,
        max_file_write_bytes: limit,
    }
}

#[cfg(unix)]
fn create_directory_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(not(any(unix, windows)))]
fn create_directory_symlink(_target: &Path, _link: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "directory symlink is unsupported on this platform",
    ))
}

#[test]
fn channel_tool_cli_validation_success_and_failure_paths() {
    let cli = channel::CliChannel::new(12);
    let accepted = cli
        .accept("run:deploy")
        .expect("valid cli input should be accepted");

    assert_eq!(accepted.kind, channel::ChannelKind::Cli);
    assert_eq!(accepted.route, "cli");
    assert_eq!(accepted.payload, "run:deploy");

    assert!(matches!(
        cli.accept(""),
        Err(channel::ChannelValidationError::EmptyPayload)
    ));
    assert!(matches!(
        cli.accept("0123456789abc"),
        Err(channel::ChannelValidationError::PayloadTooLarge {
            limit: 12,
            actual: 13
        })
    ));
}

#[test]
fn channel_tool_webhook_validation_success_and_failure_paths() {
    let webhook = channel::WebhookChannel::new(true, 64);
    let accepted = webhook
        .accept(channel::WebhookInput {
            method: "post",
            path: "/hooks/deploy",
            body: "{\"run\":true}",
            signature: Some("sig-v1"),
        })
        .expect("valid webhook should be accepted");

    assert_eq!(accepted.kind, channel::ChannelKind::Webhook);
    assert_eq!(accepted.route, "/hooks/deploy");
    assert_eq!(accepted.payload, "{\"run\":true}");

    assert!(matches!(
        webhook.accept(channel::WebhookInput {
            method: "POST",
            path: "/hooks/deploy",
            body: "{\"run\":true}",
            signature: None,
        }),
        Err(channel::ChannelValidationError::MissingSignature)
    ));
    assert!(matches!(
        webhook.accept(channel::WebhookInput {
            method: "GET",
            path: "/hooks/deploy",
            body: "{\"run\":true}",
            signature: Some("sig-v1"),
        }),
        Err(channel::ChannelValidationError::InvalidMethod)
    ));
    assert!(matches!(
        webhook.accept(channel::WebhookInput {
            method: "POST",
            path: "/hooks/../deploy",
            body: "{\"run\":true}",
            signature: Some("sig-v1"),
        }),
        Err(channel::ChannelValidationError::InvalidPath)
    ));
}

#[test]
fn channel_tool_tool_policy_allow_path_shell_read_write() {
    let workspace = TempWorkspace::new("allow");
    let adapter = tool::WorkspaceTool::new(workspace.path().to_path_buf(), allow_all_policy(1024))
        .expect("adapter should initialize");

    let canonical_root = adapter.workspace_root().to_path_buf();

    let write_result = adapter
        .execute(tool::ToolRequest::FileWrite {
            path: "notes/output.txt",
            contents: "hello",
            append: false,
        })
        .expect("file write should be allowed");
    match write_result {
        tool::ToolResult::FileWrite(output) => {
            assert_eq!(output.bytes_written, 5);
            assert!(output.path.starts_with(&canonical_root));
        }
        _ => panic!("expected file write output"),
    }

    let read_result = adapter
        .execute(tool::ToolRequest::FileRead {
            path: "notes/output.txt",
        })
        .expect("file read should be allowed");
    match read_result {
        tool::ToolResult::FileRead(output) => {
            assert_eq!(output.contents, "hello");
            assert!(output.path.starts_with(&canonical_root));
        }
        _ => panic!("expected file read output"),
    }

    let shell_result = adapter
        .execute(tool::ToolRequest::Shell {
            command: "printf ok",
        })
        .expect("shell should be allowed");
    match shell_result {
        tool::ToolResult::Shell(output) => {
            assert_eq!(output.status_code, 0);
            assert_eq!(output.stdout, "ok");
        }
        _ => panic!("expected shell output"),
    }
}

#[test]
fn channel_tool_tool_policy_deny_path_for_each_capability() {
    let workspace = TempWorkspace::new("deny");
    let adapter =
        tool::WorkspaceTool::new(workspace.path().to_path_buf(), tool::ToolPolicy::deny_all())
            .expect("adapter should initialize");

    assert!(matches!(
        adapter.execute(tool::ToolRequest::Shell { command: "echo hi" }),
        Err(tool::ToolError::PolicyDenied(tool::ToolCapability::Shell))
    ));
    assert!(matches!(
        adapter.execute(tool::ToolRequest::FileRead { path: "a.txt" }),
        Err(tool::ToolError::PolicyDenied(
            tool::ToolCapability::FileRead
        ))
    ));
    assert!(matches!(
        adapter.execute(tool::ToolRequest::FileWrite {
            path: "a.txt",
            contents: "x",
            append: false,
        }),
        Err(tool::ToolError::PolicyDenied(
            tool::ToolCapability::FileWrite
        ))
    ));
}

#[test]
fn channel_tool_tool_policy_boundary_limits_for_shell_and_file_io() {
    let workspace = TempWorkspace::new("boundary");
    let policy = allow_all_policy(4);
    let adapter = tool::WorkspaceTool::new(workspace.path().to_path_buf(), policy)
        .expect("adapter should initialize");

    let shell_ok = adapter
        .execute(tool::ToolRequest::Shell { command: "echo" })
        .expect("4-byte shell command should pass");
    assert!(matches!(shell_ok, tool::ToolResult::Shell(_)));
    assert!(matches!(
        adapter.execute(tool::ToolRequest::Shell { command: "echo " }),
        Err(tool::ToolError::InputTooLarge {
            field: "shell_command",
            limit: 4,
            actual: 5
        })
    ));

    let write_ok = adapter
        .execute(tool::ToolRequest::FileWrite {
            path: "ok.txt",
            contents: "data",
            append: false,
        })
        .expect("4-byte write should pass");
    assert!(matches!(write_ok, tool::ToolResult::FileWrite(_)));
    assert!(matches!(
        adapter.execute(tool::ToolRequest::FileWrite {
            path: "too_big.txt",
            contents: "datas",
            append: false,
        }),
        Err(tool::ToolError::InputTooLarge {
            field: "file_write",
            limit: 4,
            actual: 5
        })
    ));

    fs::write(workspace.path().join("read_ok.txt"), "data")
        .expect("read_ok test file should be created");
    fs::write(workspace.path().join("read_big.txt"), "datas")
        .expect("read_big test file should be created");
    let read_ok = adapter
        .execute(tool::ToolRequest::FileRead {
            path: "read_ok.txt",
        })
        .expect("4-byte read should pass");
    assert!(matches!(read_ok, tool::ToolResult::FileRead(_)));
    assert!(matches!(
        adapter.execute(tool::ToolRequest::FileRead {
            path: "read_big.txt"
        }),
        Err(tool::ToolError::InputTooLarge {
            field: "file_read",
            limit: 4,
            actual: 5
        })
    ));
}

#[test]
fn channel_tool_workspace_escape_is_blocked_for_read_and_write() {
    let workspace = TempWorkspace::new("escape");
    let adapter = tool::WorkspaceTool::new(workspace.path().to_path_buf(), allow_all_policy(1024))
        .expect("adapter should initialize");

    let outside_file = workspace
        .path()
        .parent()
        .expect("workspace should have parent")
        .join(format!(
            "axonrunner_adapters_channel_tool_outside_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after UNIX_EPOCH")
                .as_nanos()
        ));
    fs::write(&outside_file, "outside").expect("outside test file should be created");

    assert!(matches!(
        adapter.execute(tool::ToolRequest::FileRead {
            path: "../outside.txt"
        }),
        Err(tool::ToolError::WorkspaceEscape { .. })
    ));
    assert!(matches!(
        adapter.execute(tool::ToolRequest::FileRead {
            path: &outside_file.to_string_lossy(),
        }),
        Err(tool::ToolError::WorkspaceEscape { .. })
    ));
    assert!(matches!(
        adapter.execute(tool::ToolRequest::FileWrite {
            path: "../../escape.txt",
            contents: "blocked",
            append: false,
        }),
        Err(tool::ToolError::WorkspaceEscape { .. })
    ));

    let _ = fs::remove_file(outside_file);
}

#[test]
fn channel_tool_workspace_escape_is_blocked_for_symlinked_paths() {
    let workspace = TempWorkspace::new("symlink_escape");
    let outside = TempWorkspace::new("symlink_escape_outside");
    let adapter = tool::WorkspaceTool::new(workspace.path().to_path_buf(), allow_all_policy(1024))
        .expect("adapter should initialize");

    fs::write(outside.path().join("secret.txt"), "outside")
        .expect("outside target file should be created");

    let link_path = workspace.path().join("outside_link");
    match create_directory_symlink(outside.path(), &link_path) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::Unsupported => {
            eprintln!("skipping symlink boundary test: {err}");
            return;
        }
        Err(err) => panic!("directory symlink should be creatable or unsupported: {err}"),
    }

    assert!(matches!(
        adapter.execute(tool::ToolRequest::FileRead {
            path: "outside_link/secret.txt",
        }),
        Err(tool::ToolError::WorkspaceEscape { .. })
    ));
    assert!(matches!(
        adapter.execute(tool::ToolRequest::FileWrite {
            path: "outside_link/new.txt",
            contents: "blocked",
            append: false,
        }),
        Err(tool::ToolError::WorkspaceEscape { .. })
    ));
}
