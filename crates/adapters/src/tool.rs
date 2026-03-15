use crate::contracts::{
    AdapterHealth, FileMutationEvidence, FileWriteOutput, ListFilesOutput, ReadFileOutput,
    RemovePathOutput, ReplaceInFileOutput, RunCommandOutput, RunCommandProfile, SearchFilesOutput,
    SearchMatch, SearchMode, ToolAdapter, ToolPolicy, ToolRequest, ToolResult,
};
use crate::error::{AdapterError, AdapterResult, RetryClass};
use crate::tool_workspace::{
    WorkspacePathError, canonicalize_workspace_root, collect_files_respecting_gitignore,
    resolve_workspace_path,
};
use crate::tool_write::{
    CommandArtifact, PatchArtifact, WritePreparationError, atomic_overwrite, bounded_excerpt,
    bounded_unified_diff, digest_path, existing_digest, existing_utf8_contents,
    prepare_contents_for_existing_file, write_command_artifact, write_patch_artifact,
};
use regex::Regex;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRiskTier {
    Low,
    Medium,
    High,
}

impl ToolRiskTier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunCommandClass {
    WorkspaceLocal,
    Destructive,
    External,
}

pub fn classify_run_command_class(program: &str) -> RunCommandClass {
    if matches!(program, "rm" | "mv") {
        return RunCommandClass::Destructive;
    }

    if matches!(
        program,
        "pwd"
            | "git"
            | "cargo"
            | "npm"
            | "node"
            | "python"
            | "python3"
            | "pytest"
            | "rg"
            | "ls"
            | "cat"
            | "sh"
            | "bash"
            | "pnpm"
            | "yarn"
            | "uv"
            | "make"
    ) || program.starts_with("./")
        || program.starts_with("../")
    {
        RunCommandClass::WorkspaceLocal
    } else {
        RunCommandClass::External
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceTool {
    workspace_root: PathBuf,
    artifact_root: PathBuf,
    policy: ToolPolicy,
}

pub fn classify_tool_request_risk(request: &ToolRequest) -> ToolRiskTier {
    match request {
        ToolRequest::ListFiles { .. }
        | ToolRequest::ReadFile { .. }
        | ToolRequest::SearchFiles { .. } => ToolRiskTier::Low,
        ToolRequest::FileWrite { contents, .. } => {
            if contents.len() > 4 * 1024 {
                ToolRiskTier::High
            } else {
                ToolRiskTier::Medium
            }
        }
        ToolRequest::ReplaceInFile {
            needle,
            replacement,
            ..
        } => {
            if needle.len() > 256 || replacement.len() > 4 * 1024 {
                ToolRiskTier::High
            } else {
                ToolRiskTier::Medium
            }
        }
        ToolRequest::RemovePath { .. } => ToolRiskTier::High,
        ToolRequest::RunCommand { program, .. } => {
            if matches!(program.as_str(), "git" | "rm" | "mv") {
                ToolRiskTier::High
            } else {
                ToolRiskTier::Medium
            }
        }
    }
}

impl WorkspaceTool {
    pub fn new(
        workspace_root: impl Into<PathBuf>,
        artifact_root: impl Into<PathBuf>,
        policy: ToolPolicy,
    ) -> Result<Self, ToolError> {
        let canonical_root =
            canonicalize_workspace_root(workspace_root).map_err(map_workspace_path_error)?;
        let canonical_artifact_root =
            canonicalize_workspace_root(artifact_root).map_err(map_workspace_path_error)?;

        Ok(Self {
            workspace_root: canonical_root,
            artifact_root: canonical_artifact_root,
            policy,
        })
    }

    fn list_files(&self, path: &str) -> Result<ToolResult, ToolError> {
        let base = self.resolve_workspace_path(path)?;
        if !base.exists() {
            return Err(ToolError::NotFound {
                path: path.to_owned(),
            });
        }

        let paths = collect_files_respecting_gitignore(&base).map_err(|error| ToolError::Io {
            operation: "list_files",
            source: error,
        })?;

        Ok(ToolResult::ListFiles(ListFilesOutput { base, paths }))
    }

    fn read_file(&self, path: &str) -> Result<ToolResult, ToolError> {
        let resolved_path = self.resolve_workspace_path(path)?;
        let metadata = fs::metadata(&resolved_path).map_err(|error| ToolError::Io {
            operation: "stat_file",
            source: error,
        })?;
        if !metadata.is_file() {
            return Err(ToolError::InvalidInput("read path must be a file"));
        }
        if metadata.len() as usize > self.policy.max_file_read_bytes {
            return Err(ToolError::InputTooLarge {
                limit: self.policy.max_file_read_bytes,
                actual: metadata.len() as usize,
            });
        }

        let mut contents = String::new();
        fs::File::open(&resolved_path)
            .and_then(|mut file| file.read_to_string(&mut contents))
            .map_err(|error| ToolError::Io {
                operation: "read_file",
                source: error,
            })?;

        Ok(ToolResult::ReadFile(ReadFileOutput {
            path: resolved_path,
            contents,
        }))
    }

    fn search_files(
        &self,
        path: &str,
        needle: &str,
        mode: SearchMode,
    ) -> Result<ToolResult, ToolError> {
        if needle.trim().is_empty() {
            return Err(ToolError::InvalidInput("search needle"));
        }
        let regex = match mode {
            SearchMode::Substring => None,
            SearchMode::Regex => Some(
                Regex::new(needle).map_err(|error| ToolError::InvalidPattern(error.to_string()))?,
            ),
        };

        let base = self.resolve_workspace_path(path)?;
        if !base.exists() {
            return Err(ToolError::NotFound {
                path: path.to_owned(),
            });
        }

        let files = collect_files_respecting_gitignore(&base).map_err(|error| ToolError::Io {
            operation: "search_files",
            source: error,
        })?;

        let mut matches = Vec::new();
        let mut scanned_files = 0;
        let mut skipped_files = 0;
        for file in files {
            if matches.len() >= self.policy.max_search_results {
                break;
            }
            match fs::read_to_string(&file) {
                Ok(contents) => {
                    scanned_files += 1;
                    for (index, line) in contents.lines().enumerate() {
                        let matched = match &regex {
                            Some(regex) => regex.is_match(line),
                            None => line.contains(needle),
                        };
                        if matched {
                            matches.push(SearchMatch {
                                path: file.clone(),
                                line_number: index + 1,
                                line: line.to_owned(),
                            });
                            if matches.len() >= self.policy.max_search_results {
                                break;
                            }
                        }
                    }
                }
                Err(_) => skipped_files += 1,
            }
        }

        Ok(ToolResult::SearchFiles(SearchFilesOutput {
            base,
            matches,
            scanned_files,
            skipped_files,
        }))
    }

    fn file_write(
        &self,
        path: &str,
        contents: &str,
        append: bool,
    ) -> Result<ToolResult, ToolError> {
        if path.trim().is_empty() {
            return Err(ToolError::InvalidInput("file path"));
        }
        if path.contains('\0') || contents.contains('\0') {
            return Err(ToolError::InvalidInput("nul byte is not allowed"));
        }
        if contents.len() > self.policy.max_file_write_bytes {
            return Err(ToolError::InputTooLarge {
                limit: self.policy.max_file_write_bytes,
                actual: contents.len(),
            });
        }

        let resolved_path = self.resolve_workspace_path(path)?;
        if let Some(parent) = resolved_path.parent() {
            fs::create_dir_all(parent).map_err(|error| ToolError::Io {
                operation: "create_parent_directories",
                source: error,
            })?;
        }

        let before_contents =
            existing_utf8_contents(&resolved_path).map_err(map_write_preparation_error)?;
        let before_digest = existing_digest(&resolved_path).map_err(map_write_preparation_error)?;

        let bytes_written = if append {
            let contents = prepare_contents_for_existing_file(&resolved_path, contents)
                .map_err(map_write_preparation_error)?;
            let bytes_written = contents.len();
            let mut options = OpenOptions::new();
            options.create(true).append(true);
            let mut file = options
                .open(&resolved_path)
                .map_err(|error| ToolError::Io {
                    operation: "open_file_for_write",
                    source: error,
                })?;
            file.write_all(contents.as_bytes())
                .map_err(|error| ToolError::Io {
                    operation: "write_file",
                    source: error,
                })?;
            bytes_written
        } else {
            let contents = prepare_contents_for_existing_file(&resolved_path, contents)
                .map_err(map_write_preparation_error)?;
            let bytes_written = contents.len();
            atomic_overwrite(&resolved_path, &contents).map_err(|error| ToolError::Io {
                operation: "atomic_overwrite",
                source: error,
            })?;
            bytes_written
        };
        let after_digest = digest_path(&resolved_path).map_err(|error| ToolError::Io {
            operation: "digest_file_after_write",
            source: error,
        })?;
        let after_contents =
            existing_utf8_contents(&resolved_path).map_err(map_write_preparation_error)?;
        let operation = if append { "append" } else { "overwrite" };
        let before_excerpt = before_contents
            .as_deref()
            .and_then(|contents| bounded_excerpt(contents, 240));
        let after_excerpt = after_contents
            .as_deref()
            .and_then(|contents| bounded_excerpt(contents, 240));
        let unified_diff = before_contents
            .as_deref()
            .zip(after_contents.as_deref())
            .and_then(|(before, after)| bounded_unified_diff(before, after, 2048));
        let artifact_path = write_patch_artifact(PatchArtifact {
            workspace_root: &self.artifact_root,
            target_path: &resolved_path,
            operation,
            before_digest: before_digest.as_deref(),
            after_digest: Some(&after_digest),
            bytes_written: Some(bytes_written),
            before_excerpt: before_excerpt.as_deref(),
            after_excerpt: after_excerpt.as_deref(),
            unified_diff: unified_diff.as_deref(),
        })
        .map_err(|error| ToolError::Io {
            operation: "write_patch_artifact",
            source: error,
        })?;

        Ok(ToolResult::FileWrite(FileWriteOutput {
            path: resolved_path,
            bytes_written,
            evidence: FileMutationEvidence {
                operation: operation.to_owned(),
                artifact_path,
                before_digest,
                after_digest: Some(after_digest),
                before_excerpt,
                after_excerpt,
                unified_diff,
            },
        }))
    }

    fn replace_in_file(
        &self,
        path: &str,
        needle: &str,
        replacement: &str,
        expected_replacements: Option<usize>,
    ) -> Result<ToolResult, ToolError> {
        if needle.is_empty() {
            return Err(ToolError::InvalidInput("replace needle"));
        }
        if matches!(expected_replacements, Some(0)) {
            return Err(ToolError::InvalidInput(
                "expected replacements must be positive",
            ));
        }
        let resolved_path = self.resolve_workspace_path(path)?;
        let contents = match fs::read(&resolved_path).map_err(|error| ToolError::Io {
            operation: "read_file_for_replace",
            source: error,
        }) {
            Ok(bytes) => String::from_utf8(bytes).map_err(|_| ToolError::UnsupportedEncoding {
                path: resolved_path.clone(),
            })?,
            Err(error) => return Err(error),
        };
        let replacements = contents.matches(needle).count();
        if replacements == 0 {
            return Err(ToolError::NotFound {
                path: format!("{path}::{needle}"),
            });
        }
        if let Some(expected) = expected_replacements {
            if replacements != expected {
                return Err(ToolError::ReplacementCountMismatch {
                    path: path.to_owned(),
                    expected,
                    actual: replacements,
                });
            }
        } else if replacements > 1 {
            return Err(ToolError::AmbiguousReplace {
                path: path.to_owned(),
                occurrences: replacements,
            });
        }
        let updated = contents.replace(needle, replacement);
        let write_result = self.file_write(path, &updated, false)?;
        let ToolResult::FileWrite(write_output) = write_result else {
            return Err(ToolError::Io {
                operation: "replace_in_file_write_result",
                source: io::Error::other("unexpected tool result"),
            });
        };
        Ok(ToolResult::ReplaceInFile(ReplaceInFileOutput {
            path: resolved_path,
            replacements,
            evidence: write_output.evidence,
        }))
    }

    fn remove_path(&self, path: &str) -> Result<ToolResult, ToolError> {
        let resolved_path = self.resolve_workspace_path(path)?;
        if !resolved_path.exists() {
            return Ok(ToolResult::RemovePath(RemovePathOutput {
                path: resolved_path,
                removed: false,
                evidence: None,
            }));
        }

        let before_contents =
            existing_utf8_contents(&resolved_path).map_err(map_write_preparation_error)?;
        let before_digest = existing_digest(&resolved_path).map_err(map_write_preparation_error)?;
        let metadata = fs::metadata(&resolved_path).map_err(|error| ToolError::Io {
            operation: "stat_remove_path",
            source: error,
        })?;
        if metadata.is_dir() {
            fs::remove_dir_all(&resolved_path).map_err(|error| ToolError::Io {
                operation: "remove_dir_all",
                source: error,
            })?;
        } else {
            fs::remove_file(&resolved_path).map_err(|error| ToolError::Io {
                operation: "remove_file",
                source: error,
            })?;
        }

        let before_excerpt = before_contents
            .as_deref()
            .and_then(|contents| bounded_excerpt(contents, 240));
        let artifact_path = write_patch_artifact(PatchArtifact {
            workspace_root: &self.artifact_root,
            target_path: &resolved_path,
            operation: "remove",
            before_digest: before_digest.as_deref(),
            after_digest: None,
            bytes_written: None,
            before_excerpt: before_excerpt.as_deref(),
            after_excerpt: None,
            unified_diff: None,
        })
        .map_err(|error| ToolError::Io {
            operation: "write_patch_artifact",
            source: error,
        })?;

        Ok(ToolResult::RemovePath(RemovePathOutput {
            path: resolved_path,
            removed: true,
            evidence: Some(FileMutationEvidence {
                operation: String::from("remove"),
                artifact_path,
                before_digest,
                after_digest: None,
                before_excerpt,
                after_excerpt: None,
                unified_diff: None,
            }),
        }))
    }

    fn run_command(&self, program: &str, args: &[String]) -> Result<ToolResult, ToolError> {
        if program.trim().is_empty() {
            return Err(ToolError::InvalidInput("run command program"));
        }
        if is_forbidden_shell_program(program) {
            return Err(ToolError::CommandDenied {
                program: program.to_owned(),
            });
        }
        if !self
            .policy
            .command_allowlist
            .iter()
            .any(|allowed| allowed == program)
        {
            return Err(ToolError::CommandDenied {
                program: program.to_owned(),
            });
        }

        let profile = classify_run_command_profile(program, args);
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(&self.workspace_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_command(&mut command);
        let mut child = command.spawn().map_err(|error| ToolError::Io {
            operation: "run_command",
            source: error,
        })?;

        let stdout_worker = spawn_output_worker(
            child.stdout.take().ok_or_else(|| ToolError::Io {
                operation: "capture_command_stdout",
                source: io::Error::other("stdout pipe not available"),
            })?,
            self.policy.max_command_output_bytes,
        );
        let stderr_worker = spawn_output_worker(
            child.stderr.take().ok_or_else(|| ToolError::Io {
                operation: "capture_command_stderr",
                source: io::Error::other("stderr pipe not available"),
            })?,
            self.policy.max_command_output_bytes,
        );

        let timeout = Duration::from_millis(self.policy.command_timeout_ms.max(1));
        let started_at = Instant::now();
        let exit_code = loop {
            match child.try_wait().map_err(|error| ToolError::Io {
                operation: "run_command_try_wait",
                source: error,
            })? {
                Some(status) => break status.code().unwrap_or(-1),
                None if started_at.elapsed() >= timeout => {
                    let _ = terminate_child(&mut child);
                    let _ = child.wait();
                    let _ = join_output_worker(stdout_worker, "read_command_stdout");
                    let _ = join_output_worker(stderr_worker, "read_command_stderr");
                    return Err(ToolError::Timeout {
                        program: program.to_owned(),
                        timeout_ms: timeout.as_millis() as u64,
                    });
                }
                None => thread::sleep(Duration::from_millis(10)),
            }
        };

        let stdout = join_output_worker(stdout_worker, "read_command_stdout")?;
        let stderr = join_output_worker(stderr_worker, "read_command_stderr")?;
        let rendered_stdout = finalize_command_output(stdout.bytes, stdout.truncated);
        let rendered_stderr = finalize_command_output(stderr.bytes, stderr.truncated);
        let artifact_path = write_command_artifact(CommandArtifact {
            workspace_root: &self.artifact_root,
            program,
            args,
            profile: profile.as_str(),
            exit_code,
            stdout: &rendered_stdout,
            stderr: &rendered_stderr,
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
        })
        .map_err(|error| ToolError::Io {
            operation: "write_command_artifact",
            source: error,
        })?;

        Ok(ToolResult::RunCommand(RunCommandOutput {
            program: program.to_owned(),
            args: args.to_vec(),
            profile,
            exit_code,
            stdout: rendered_stdout,
            stderr: rendered_stderr,
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
            artifact_path,
        }))
    }

    fn resolve_workspace_path(&self, requested_path: &str) -> Result<PathBuf, ToolError> {
        resolve_workspace_path(&self.workspace_root, requested_path)
            .map_err(map_workspace_path_error)
    }
}

fn classify_run_command_profile(program: &str, args: &[String]) -> RunCommandProfile {
    match (
        program,
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        ("cargo", Some("build"), _) => RunCommandProfile::Build,
        ("cargo", Some("test"), _) => RunCommandProfile::Test,
        ("cargo", Some("clippy"), _) => RunCommandProfile::Lint,
        ("cargo", Some("fmt"), Some("--check")) => RunCommandProfile::Lint,
        ("cargo", Some("fmt"), _) => RunCommandProfile::Lint,
        ("npm", Some("test"), _) | ("pnpm", Some("test"), _) | ("yarn", Some("test"), _) => {
            RunCommandProfile::Test
        }
        ("npm", Some("run"), Some("lint"))
        | ("pnpm", Some("run"), Some("lint"))
        | ("yarn", Some("lint"), _) => RunCommandProfile::Lint,
        ("npm", Some("run"), Some("build"))
        | ("pnpm", Some("run"), Some("build"))
        | ("yarn", Some("build"), _) => RunCommandProfile::Build,
        _ => RunCommandProfile::Generic,
    }
}

impl ToolAdapter for WorkspaceTool {
    fn id(&self) -> &str {
        "tool.workspace"
    }

    fn health(&self) -> AdapterHealth {
        if self.workspace_root.is_dir() {
            AdapterHealth::Healthy
        } else {
            AdapterHealth::Unavailable
        }
    }

    fn execute(&self, request: ToolRequest) -> AdapterResult<ToolResult> {
        let result = match request {
            ToolRequest::ListFiles { path } => self.list_files(&path),
            ToolRequest::ReadFile { path } => self.read_file(&path),
            ToolRequest::SearchFiles { path, needle, mode } => {
                self.search_files(&path, &needle, mode)
            }
            ToolRequest::FileWrite {
                path,
                contents,
                append,
            } => self.file_write(&path, &contents, append),
            ToolRequest::ReplaceInFile {
                path,
                needle,
                replacement,
                expected_replacements,
            } => self.replace_in_file(&path, &needle, &replacement, expected_replacements),
            ToolRequest::RemovePath { path } => self.remove_path(&path),
            ToolRequest::RunCommand { program, args } => self.run_command(&program, &args),
        };

        result.map_err(map_tool_error)
    }
}

#[derive(Debug)]
pub enum ToolError {
    InvalidInput(&'static str),
    NotFound {
        path: String,
    },
    InputTooLarge {
        limit: usize,
        actual: usize,
    },
    WorkspaceEscape {
        requested: String,
    },
    InvalidPattern(String),
    CommandDenied {
        program: String,
    },
    UnsupportedEncoding {
        path: PathBuf,
    },
    AmbiguousReplace {
        path: String,
        occurrences: usize,
    },
    ReplacementCountMismatch {
        path: String,
        expected: usize,
        actual: usize,
    },
    Timeout {
        program: String,
        timeout_ms: u64,
    },
    Io {
        operation: &'static str,
        source: io::Error,
    },
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(reason) => write!(f, "invalid tool input: {reason}"),
            Self::NotFound { path } => write!(f, "tool path not found: {path}"),
            Self::InputTooLarge { limit, actual } => {
                write!(f, "tool input exceeds limit ({actual} > {limit})")
            }
            Self::WorkspaceEscape { requested } => {
                write!(f, "path escapes workspace boundary: {requested}")
            }
            Self::InvalidPattern(pattern) => write!(f, "invalid search pattern: {pattern}"),
            Self::CommandDenied { program } => write!(f, "command not allowed: {program}"),
            Self::UnsupportedEncoding { path } => {
                write!(f, "unsupported file encoding: {}", path.display())
            }
            Self::AmbiguousReplace { path, occurrences } => {
                write!(
                    f,
                    "replace target is ambiguous: {path} matched {occurrences} locations"
                )
            }
            Self::ReplacementCountMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "replace target count mismatch: {path} expected {expected} matches but found {actual}"
            ),
            Self::Timeout {
                program,
                timeout_ms,
            } => write!(f, "command timed out: {program} after {timeout_ms}ms"),
            Self::Io { operation, source } => write!(f, "{operation} failed: {source}"),
        }
    }
}

fn finalize_command_output(bytes: Vec<u8>, truncated: bool) -> String {
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if !truncated {
        return text;
    }

    let marker = "...[truncated]";
    let budget = bytes.len().saturating_sub(marker.len());
    let mut truncated_text = String::new();
    for ch in text.chars() {
        let len = ch.len_utf8();
        if truncated_text.len() + len > budget {
            break;
        }
        truncated_text.push(ch);
    }
    truncated_text.push_str(marker);
    truncated_text
}

fn spawn_output_worker<R>(
    mut reader: R,
    limit: usize,
) -> JoinHandle<Result<CollectedOutput, io::Error>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut collected = Vec::new();
        let mut buffer = [0_u8; 4096];
        let mut truncated = false;
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            let available = limit.saturating_sub(collected.len());
            if available > 0 {
                let kept = available.min(bytes_read);
                collected.extend_from_slice(&buffer[..kept]);
                if kept < bytes_read {
                    truncated = true;
                }
            } else {
                truncated = true;
            }
        }
        Ok(CollectedOutput {
            bytes: collected,
            truncated,
        })
    })
}

fn join_output_worker(
    worker: JoinHandle<Result<CollectedOutput, io::Error>>,
    operation: &'static str,
) -> Result<CollectedOutput, ToolError> {
    match worker.join() {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(source)) => Err(ToolError::Io { operation, source }),
        Err(_) => Err(ToolError::Io {
            operation,
            source: io::Error::other("command output worker panicked"),
        }),
    }
}

#[derive(Debug)]
struct CollectedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

fn configure_command(command: &mut Command) {
    #[cfg(unix)]
    {
        command.process_group(0);
    }
}

fn is_forbidden_shell_program(program: &str) -> bool {
    matches!(
        program.trim(),
        "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh"
    )
}

fn terminate_child(child: &mut Child) -> io::Result<()> {
    #[cfg(unix)]
    {
        let group_id = child.id() as i32;
        let status = Command::new("kill")
            .args(["-KILL", &format!("-{group_id}")])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            child.kill()
        }
    }

    #[cfg(not(unix))]
    {
        child.kill()
    }
}

impl std::error::Error for ToolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn map_tool_error(error: ToolError) -> AdapterError {
    match error {
        ToolError::InvalidInput(reason) => AdapterError::invalid_input("tool", reason),
        ToolError::NotFound { path } => AdapterError::not_found("tool_path", path),
        ToolError::InputTooLarge { limit, actual } => AdapterError::failed(
            "tool.input_too_large",
            format!("{actual}>{limit}"),
            RetryClass::NonRetryable,
        ),
        ToolError::WorkspaceEscape { requested } => {
            AdapterError::failed("tool.workspace_escape", requested, RetryClass::PolicyDenied)
        }
        ToolError::InvalidPattern(pattern) => AdapterError::failed(
            "tool.invalid_search_pattern",
            pattern,
            RetryClass::NonRetryable,
        ),
        ToolError::CommandDenied { program } => {
            AdapterError::failed("tool.command_denied", program, RetryClass::PolicyDenied)
        }
        ToolError::UnsupportedEncoding { path } => AdapterError::failed(
            "tool.unsupported_encoding",
            path.display().to_string(),
            RetryClass::NonRetryable,
        ),
        ToolError::AmbiguousReplace { path, occurrences } => AdapterError::failed(
            "tool.ambiguous_replace",
            format!("{path}@{occurrences}"),
            RetryClass::NonRetryable,
        ),
        ToolError::ReplacementCountMismatch {
            path,
            expected,
            actual,
        } => AdapterError::failed(
            "tool.replace_count_mismatch",
            format!("{path}@expected={expected}@actual={actual}"),
            RetryClass::NonRetryable,
        ),
        ToolError::Timeout {
            program,
            timeout_ms,
        } => AdapterError::failed(
            "tool.command_timeout",
            format!("{program}@{timeout_ms}ms"),
            RetryClass::Retryable,
        ),
        ToolError::Io { operation, source } => {
            AdapterError::failed(operation, source.to_string(), RetryClass::Retryable)
        }
    }
}

fn map_workspace_path_error(error: WorkspacePathError) -> ToolError {
    match error {
        WorkspacePathError::InvalidInput(reason) => ToolError::InvalidInput(reason),
        WorkspacePathError::WorkspaceEscape { requested } => {
            ToolError::WorkspaceEscape { requested }
        }
        WorkspacePathError::Io { operation, source } => ToolError::Io { operation, source },
    }
}

fn map_write_preparation_error(error: WritePreparationError) -> ToolError {
    match error {
        WritePreparationError::Io(source) => ToolError::Io {
            operation: "prepare_file_contents",
            source,
        },
        WritePreparationError::UnsupportedEncoding => {
            ToolError::InvalidInput("text write requires an existing UTF-8 file")
        }
    }
}
