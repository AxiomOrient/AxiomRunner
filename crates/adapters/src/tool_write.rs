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

pub(crate) fn write_patch_artifact(
    workspace_root: &Path,
    target_path: &Path,
    operation: &str,
    before_digest: Option<&str>,
    after_digest: Option<&str>,
    bytes_written: Option<usize>,
    before_excerpt: Option<&str>,
    after_excerpt: Option<&str>,
    unified_diff: Option<&str>,
) -> Result<PathBuf, io::Error> {
    let patch_dir = workspace_root.join(".axonrunner").join("patches");
    fs::create_dir_all(&patch_dir)?;
    let artifact_path = patch_dir.join(unique_patch_filename(target_path));
    let relative_target = target_path
        .strip_prefix(workspace_root)
        .unwrap_or(target_path)
        .display()
        .to_string();
    let payload = json!({
        "schema": "axonrunner.patch.v2",
        "timestamp_ms": now_millis(),
        "operation": operation,
        "target_path": relative_target,
        "before_digest": before_digest,
        "after_digest": after_digest,
        "bytes_written": bytes_written,
        "before_excerpt": before_excerpt,
        "after_excerpt": after_excerpt,
        "unified_diff": unified_diff,
    });
    let rendered =
        serde_json::to_vec_pretty(&payload).map_err(|error| io::Error::other(error.to_string()))?;
    fs::write(&artifact_path, rendered)?;
    Ok(artifact_path)
}

pub(crate) fn write_command_artifact(
    workspace_root: &Path,
    program: &str,
    args: &[String],
    profile: &str,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
    stdout_truncated: bool,
    stderr_truncated: bool,
) -> Result<PathBuf, io::Error> {
    let command_dir = workspace_root.join(".axonrunner").join("commands");
    fs::create_dir_all(&command_dir)?;
    let artifact_path = command_dir.join(format!(
        "{}-{}-{}.json",
        program.replace('/', "_"),
        std::process::id(),
        now_millis()
    ));
    let payload = json!({
        "schema": "axonrunner.command.v1",
        "timestamp_ms": now_millis(),
        "program": program,
        "args": args,
        "profile": profile,
        "exit_code": exit_code,
        "stdout": bounded_excerpt(stdout, 240),
        "stderr": bounded_excerpt(stderr, 240),
        "stdout_truncated": stdout_truncated,
        "stderr_truncated": stderr_truncated,
    });
    let rendered =
        serde_json::to_vec_pretty(&payload).map_err(|error| io::Error::other(error.to_string()))?;
    fs::write(&artifact_path, rendered)?;
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
        detect_line_ending, normalize_line_endings, prepare_contents_for_existing_file,
    };
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_path(label: &str) -> std::path::PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axonrunner-tool-write-test-{label}-{}-{tick}",
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
}
