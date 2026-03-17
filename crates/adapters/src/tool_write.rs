use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineEnding {
    Lf,
    Crlf,
    Cr,
}

#[derive(Debug)]
pub(crate) enum WritePreparationError {
    Io(io::Error),
    UnsupportedEncoding,
}

impl From<io::Error> for WritePreparationError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub(crate) fn detect_line_ending(contents: &str) -> Option<LineEnding> {
    let bytes = contents.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                if i > 0 && bytes[i - 1] == b'\r' {
                    return Some(LineEnding::Crlf);
                }
                return Some(LineEnding::Lf);
            }
            b'\r' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    return Some(LineEnding::Crlf);
                }
                return Some(LineEnding::Cr);
            }
            _ => {}
        }
        i += 1;
    }
    None
}

pub(crate) fn normalize_line_endings(contents: &str, ending: LineEnding) -> String {
    let target = match ending {
        LineEnding::Lf => "\n",
        LineEnding::Crlf => "\r\n",
        LineEnding::Cr => "\r",
    };

    let normalized = contents.replace("\r\n", "\n").replace('\r', "\n");
    if target == "\n" {
        normalized
    } else {
        normalized.replace('\n', target)
    }
}

pub(crate) fn prepare_contents_for_existing_file(
    path: &Path,
    contents: &str,
) -> Result<String, WritePreparationError> {
    if !path.exists() {
        return Ok(contents.to_owned());
    }

    let existing = read_existing_utf8(path)?;
    if !contents.contains('\n') && !contents.contains('\r') {
        return Ok(contents.to_owned());
    }

    let Some(ending) = detect_line_ending(&existing) else {
        return Ok(contents.to_owned());
    };

    Ok(normalize_line_endings(contents, ending))
}

pub(crate) fn atomic_overwrite(path: &Path, contents: &str) -> Result<(), io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = temp_path_for(path);
    {
        let mut temp = fs::File::create(&temp_path)?;
        temp.write_all(contents.as_bytes())?;
        temp.sync_all()?;
    }

    match fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(path)?;
            fs::rename(&temp_path, path)
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error)
        }
    }
}

pub(crate) fn existing_digest(path: &Path) -> Result<Option<String>, WritePreparationError> {
    if !path.exists() {
        return Ok(None);
    }
    digest_path(path)
        .map(Some)
        .map_err(WritePreparationError::Io)
}

pub(crate) fn existing_utf8_contents(path: &Path) -> Result<Option<String>, WritePreparationError> {
    if !path.exists() {
        return Ok(None);
    }
    read_existing_utf8(path).map(Some)
}

pub(crate) fn digest_path(path: &Path) -> Result<String, io::Error> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) struct PatchArtifact<'a> {
    pub artifact_root: &'a Path,
    pub workspace_root: &'a Path,
    pub target_path: &'a Path,
    pub operation: &'a str,
    pub before_digest: Option<&'a str>,
    pub after_digest: Option<&'a str>,
    pub bytes_written: Option<usize>,
    pub before_excerpt: Option<&'a str>,
    pub after_excerpt: Option<&'a str>,
    pub unified_diff: Option<&'a str>,
    pub restore_mode: Option<&'a str>,
    pub restore_artifact_path: Option<&'a Path>,
}

fn write_json_artifact(path: &Path, payload: serde_json::Value) -> Result<(), io::Error> {
    let rendered =
        serde_json::to_vec_pretty(&payload).map_err(|error| io::Error::other(error.to_string()))?;
    fs::write(path, rendered)
}

pub(crate) fn write_patch_artifact(artifact: PatchArtifact<'_>) -> Result<PathBuf, io::Error> {
    let patch_dir = artifact.artifact_root.join(".axiomrunner").join("patches");
    fs::create_dir_all(&patch_dir)?;
    let artifact_path = patch_dir.join(unique_patch_filename(artifact.target_path));
    let relative_target = artifact
        .target_path
        .strip_prefix(artifact.workspace_root)
        .unwrap_or(artifact.target_path)
        .display()
        .to_string();
    let payload = json!({
        "schema": "axiomrunner.patch.v2",
        "timestamp_ms": now_millis(),
        "operation": artifact.operation,
        "target_path": relative_target,
        "before_digest": artifact.before_digest,
        "after_digest": artifact.after_digest,
        "bytes_written": artifact.bytes_written,
        "before_excerpt": artifact.before_excerpt,
        "after_excerpt": artifact.after_excerpt,
        "unified_diff": artifact.unified_diff,
        "restore_mode": artifact.restore_mode,
        "restore_artifact_path": artifact
            .restore_artifact_path
            .map(|path| path.display().to_string()),
    });
    write_json_artifact(&artifact_path, payload)?;
    Ok(artifact_path)
}

pub(crate) fn write_file_restore_artifact(
    artifact_root: &Path,
    target_path: &Path,
    contents: &[u8],
) -> Result<PathBuf, io::Error> {
    let restore_dir = artifact_root.join(".axiomrunner").join("restores");
    fs::create_dir_all(&restore_dir)?;
    let artifact_path = restore_dir.join(unique_patch_filename(target_path));
    let payload = json!({
        "schema": "axiomrunner.restore.file.v1",
        "target_path": target_path.display().to_string(),
        "contents_hex": hex_encode_bytes(contents),
    });
    write_json_artifact(&artifact_path, payload)?;
    Ok(artifact_path)
}

pub(crate) fn write_directory_restore_artifact(
    artifact_root: &Path,
    target_path: &Path,
) -> Result<PathBuf, io::Error> {
    let restore_dir = artifact_root.join(".axiomrunner").join("restores");
    fs::create_dir_all(&restore_dir)?;
    let artifact_path = restore_dir.join(unique_patch_filename(target_path));
    let mut entries = Vec::new();
    snapshot_directory_entries(target_path, target_path, &mut entries)?;
    let payload = json!({
        "schema": "axiomrunner.restore.dir.v1",
        "target_path": target_path.display().to_string(),
        "entries": entries,
    });
    write_json_artifact(&artifact_path, payload)?;
    Ok(artifact_path)
}

pub(crate) struct CommandArtifact<'a> {
    pub artifact_root: &'a Path,
    pub program: &'a str,
    pub args: &'a [String],
    pub profile: &'a str,
    pub exit_code: i32,
    pub stdout: &'a str,
    pub stderr: &'a str,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

pub(crate) fn write_command_artifact(artifact: CommandArtifact<'_>) -> Result<PathBuf, io::Error> {
    let command_dir = artifact.artifact_root.join(".axiomrunner").join("commands");
    fs::create_dir_all(&command_dir)?;
    let artifact_path = command_dir.join(format!(
        "{}-{}-{}.json",
        artifact.program.replace('/', "_"),
        std::process::id(),
        now_millis()
    ));
    let payload = json!({
        "schema": "axiomrunner.command.v1",
        "timestamp_ms": now_millis(),
        "program": artifact.program,
        "args": artifact.args,
        "profile": artifact.profile,
        "exit_code": artifact.exit_code,
        "stdout": bounded_excerpt(artifact.stdout, 240),
        "stderr": bounded_excerpt(artifact.stderr, 240),
        "stdout_truncated": artifact.stdout_truncated,
        "stderr_truncated": artifact.stderr_truncated,
    });
    write_json_artifact(&artifact_path, payload)?;
    Ok(artifact_path)
}

fn temp_path_for(path: &Path) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    let extension = format!("tmp-{}-{nonce}", std::process::id());
    path.with_extension(extension)
}

fn unique_patch_filename(path: &Path) -> String {
    let stem = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file")
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect::<String>();
    format!("{stem}-{}-{}.json", std::process::id(), now_millis())
}

fn now_millis() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

fn read_existing_utf8(path: &Path) -> Result<String, WritePreparationError> {
    let bytes = fs::read(path)?;
    String::from_utf8(bytes).map_err(|_| WritePreparationError::UnsupportedEncoding)
}

fn snapshot_directory_entries(
    root: &Path,
    path: &Path,
    entries: &mut Vec<serde_json::Value>,
) -> Result<(), io::Error> {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let rel = if relative.as_os_str().is_empty() {
        String::from(".")
    } else {
        relative.display().to_string()
    };
    entries.push(json!({
        "path": rel,
        "kind": "dir",
    }));

    let mut children = fs::read_dir(path)?.collect::<Result<Vec<_>, io::Error>>()?;
    children.sort_by_key(|entry| entry.path());

    for child in children {
        let child_path = child.path();
        let metadata = child.metadata()?;
        if metadata.is_dir() {
            snapshot_directory_entries(root, &child_path, entries)?;
        } else if metadata.is_file() {
            let relative = child_path.strip_prefix(root).unwrap_or(&child_path);
            entries.push(json!({
                "path": relative.display().to_string(),
                "kind": "file",
                "contents_hex": hex_encode_bytes(&fs::read(&child_path)?),
            }));
        }
    }

    Ok(())
}

fn hex_encode_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(s, "{byte:02x}").expect("writing to String is infallible");
    }
    s
}

pub(crate) fn bounded_excerpt(contents: &str, limit: usize) -> Option<String> {
    if contents.is_empty() {
        return None;
    }
    let normalized = contents.replace('\n', "\\n").replace('\r', "\\r");
    let excerpt = normalized.chars().take(limit).collect::<String>();
    if normalized.chars().count() > limit {
        Some(format!("{excerpt}...<truncated>"))
    } else {
        Some(excerpt)
    }
}

pub(crate) fn bounded_unified_diff(before: &str, after: &str, limit: usize) -> Option<String> {
    if before == after {
        return None;
    }
    let before_lines = before.lines().collect::<Vec<_>>();
    let after_lines = after.lines().collect::<Vec<_>>();
    if before_lines.len() + after_lines.len() > 80 {
        return None;
    }

    let mut diff = String::from("--- before\\n+++ after");
    for line in before_lines {
        diff.push_str("\\n-");
        diff.push_str(line);
    }
    for line in after_lines {
        diff.push_str("\\n+");
        diff.push_str(line);
    }

    if diff.chars().count() > limit {
        let truncated = diff.chars().take(limit).collect::<String>();
        Some(format!("{truncated}...<truncated>"))
    } else {
        Some(diff)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LineEnding, WritePreparationError, bounded_excerpt, bounded_unified_diff,
        detect_line_ending, hex_encode_bytes, normalize_line_endings,
        prepare_contents_for_existing_file, write_file_restore_artifact,
    };
    use serde_json::Value;
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_path(label: &str) -> std::path::PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiomrunner-tool-write-test-{label}-{}-{tick}",
            std::process::id()
        ))
    }

    #[test]
    fn detect_line_ending_finds_first_style() {
        assert_eq!(detect_line_ending("a\nb\n"), Some(LineEnding::Lf));
        assert_eq!(detect_line_ending("a\r\nb\r\n"), Some(LineEnding::Crlf));
        assert_eq!(detect_line_ending("a\rb\r"), Some(LineEnding::Cr));
        assert_eq!(detect_line_ending("plain"), None);
    }

    #[test]
    fn normalize_line_endings_rewrites_to_requested_style() {
        assert_eq!(
            normalize_line_endings("a\nb\r\nc\r", LineEnding::Lf),
            "a\nb\nc\n"
        );
        assert_eq!(
            normalize_line_endings("a\nb\r\nc\r", LineEnding::Crlf),
            "a\r\nb\r\nc\r\n"
        );
    }

    #[test]
    fn prepare_contents_rejects_non_utf8_existing_file() {
        let path = unique_path("encoding");
        fs::write(&path, [0xff_u8, 0xfe_u8, 0xfd_u8]).expect("fixture should be written");

        let err = prepare_contents_for_existing_file(&path, "alpha\nbeta\n")
            .expect_err("non-utf8 file should be rejected");
        assert!(matches!(err, WritePreparationError::UnsupportedEncoding));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn bounded_excerpt_marks_truncation() {
        assert_eq!(
            bounded_excerpt("alpha\nbeta\ngamma", 8),
            Some(String::from("alpha\\nb...<truncated>"))
        );
    }

    #[test]
    fn bounded_unified_diff_includes_before_and_after_lines() {
        let diff = bounded_unified_diff("alpha\nbeta\n", "alpha\ngamma\n", 256)
            .expect("diff should be present");
        assert!(diff.contains("--- before"));
        assert!(diff.contains("-beta"));
        assert!(diff.contains("+gamma"));
    }

    #[test]
    fn hex_encode_bytes_renders_dense_lowercase_hex() {
        assert_eq!(hex_encode_bytes(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }

    #[test]
    fn write_file_restore_artifact_persists_hex_payload() {
        let artifact_root = unique_path("restore-artifact-root");
        let target_path = unique_path("restore-artifact-target");
        fs::create_dir_all(&artifact_root).expect("artifact root should exist");

        let artifact_path = write_file_restore_artifact(&artifact_root, &target_path, b"\x00\xff")
            .expect("restore artifact should be written");
        let payload: Value = serde_json::from_slice(
            &fs::read(&artifact_path).expect("restore artifact should be readable"),
        )
        .expect("restore artifact should be valid json");

        assert_eq!(payload["schema"], "axiomrunner.restore.file.v1");
        assert_eq!(payload["target_path"], target_path.display().to_string());
        assert_eq!(payload["contents_hex"], "00ff");

        let _ = fs::remove_dir_all(artifact_root);
    }
}
