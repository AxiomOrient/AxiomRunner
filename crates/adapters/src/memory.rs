use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod memory_hybrid;
mod memory_markdown;
mod memory_sqlite;

pub use memory_hybrid::{
    HybridRecallBenchmark, HybridRecallConfig, HybridRecallHit, HybridRecallWeights,
    RetentionPolicy, RetentionReport, benchmark_hybrid_recall, hybrid_recall, rank_hybrid_recall,
    run_sqlite_retention_job,
};
pub use memory_markdown::MarkdownMemoryAdapter;
pub use memory_sqlite::SqliteMemoryAdapter;

use crate::memory_axiomme::AxiommeMemoryAdapter;

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
        "axiomme" | "axiomme-core" => AxiommeMemoryAdapter::new(path)
            .map(|adapter| Box::new(adapter) as Box<dyn crate::contracts::MemoryAdapter>)
            .map_err(|error| format!("axiomme init: {error}")),
        _ => Err(format!(
            "unsupported memory backend '{backend}'. supported backends: markdown, sqlite, axiomme"
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
            MemoryError::Io(error) => write!(f, "io error: {error}"),
            MemoryError::InvalidData(message) => write!(f, "invalid data: {message}"),
            MemoryError::Backend(message) => write!(f, "backend error: {message}"),
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
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

pub(crate) fn tokenize_terms(input: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();

    for ch in input.chars() {
        if ch.is_alphanumeric() {
            // Keys are ASCII identifiers (e.g. "user.name"); values may contain
            // non-ASCII alphanumerics but to_ascii_lowercase is safe: non-ASCII
            // chars are pushed unchanged, and we only need case-folding for A-Z.
            // to_lowercase() would handle multi-char Unicode expansions (ß → "ss")
            // but that is unnecessary overhead for this indexing use-case.
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
    records.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.key.cmp(&b.key))
    });
}

pub(crate) fn sort_entries(entries: &mut [crate::contracts::MemoryEntry]) {
    entries.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.key.cmp(&b.key))
    });
}

/// Build a ContextAdapter by backend name.
///
/// O(1) construction; initialization cost depends on backend (AxiomMe opens
/// or creates the context root on disk).
pub fn build_contract_context(
    backend: &str,
    root: impl Into<std::path::PathBuf>,
) -> Result<Box<dyn crate::contracts::ContextAdapter>, String> {
    match backend.trim() {
        "axiomme" | "axiomme-core" => {
            let adapter = crate::context_axiomme::AxiommeContextAdapter::new(root)?;
            Ok(Box::new(adapter))
        }
        other => Err(format!("unknown context backend: '{other}'")),
    }
}
