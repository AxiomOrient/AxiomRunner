use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod memory_markdown;
mod memory_sqlite;

pub use memory_markdown::MarkdownMemoryAdapter;
pub use memory_sqlite::SqliteMemoryAdapter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTier {
    Working,
    Recall,
}

impl MemoryTier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Recall => "recall",
        }
    }
}

pub fn tiered_memory_key(tier: MemoryTier, key: &str) -> String {
    format!("{}:{key}", tier.as_str())
}

pub fn detect_memory_tier(key: &str) -> MemoryTier {
    if key.starts_with("recall:") {
        MemoryTier::Recall
    } else {
        MemoryTier::Working
    }
}

pub(crate) type MemoryResult<T> = Result<T, MemoryError>;

pub fn build_contract_memory(
    backend: &str,
    path: impl Into<PathBuf>,
) -> Result<Box<dyn crate::contracts::MemoryAdapter>, String> {
    let backend = backend.trim().to_ascii_lowercase();
    let path = path.into();

    match backend.as_str() {
        "markdown" | "md" => MarkdownMemoryAdapter::new(path)
            .map(|adapter| Box::new(adapter) as Box<dyn crate::contracts::MemoryAdapter>)
            .map_err(|error| format!("failed to initialize markdown memory adapter: {error}")),
        "sqlite" | "sqlite3" => SqliteMemoryAdapter::new(path)
            .map(|adapter| Box::new(adapter) as Box<dyn crate::contracts::MemoryAdapter>)
            .map_err(|error| format!("failed to initialize sqlite memory adapter: {error}")),
        _ => Err(format!(
            "unsupported memory backend '{backend}'. supported backends: markdown, sqlite"
        )),
    }
}

#[derive(Debug)]
pub(crate) enum MemoryError {
    Io(io::Error),
    InvalidData(String),
    Backend(String),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::InvalidData(message) => write!(f, "invalid data: {message}"),
            Self::Backend(message) => write!(f, "backend error: {message}"),
        }
    }
}

impl std::error::Error for MemoryError {}

impl From<io::Error> for MemoryError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemoryRecord {
    pub key: String,
    pub value: String,
    pub updated_at: u64,
}

pub(crate) fn create_parent_dir(path: &Path) -> MemoryResult<()> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            fs::create_dir_all(parent)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(crate) fn now_millis() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_millis(0));
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).ok())
        .collect()
}

pub(crate) fn tokenize_terms(input: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();

    for ch in input.chars() {
        if ch.is_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            terms.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        terms.push(current);
    }

    terms.sort_unstable();
    terms.dedup();
    terms
}

pub(crate) fn record_terms(key: &str, value: &str) -> Vec<String> {
    let mut terms = tokenize_terms(key);
    terms.extend(tokenize_terms(value));
    terms.sort_unstable();
    terms.dedup();
    terms
}

pub(crate) fn sort_records(records: &mut [MemoryRecord]) {
    records.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.key.cmp(&right.key))
    });
}
