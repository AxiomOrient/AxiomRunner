use std::path::PathBuf;

pub const DEFAULT_EXPECTED_SCHEMA: &str = "2.0.0";
pub const DEFAULT_PROFILE: &str = "prod";
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8080";
pub const SQLITE_QUERY: &str =
    "SELECT key_hex, value_hex, updated_at FROM memory ORDER BY updated_at DESC, key_hex ASC;";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub legacy_root: PathBuf,
    pub target_root: PathBuf,
    pub dry_run: bool,
    pub report_path: Option<PathBuf>,
    pub expected_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyConfig {
    pub profile: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRecord {
    pub key: String,
    pub value: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub source_paths: SourcePaths,
    pub schema: SchemaCompatibility,
    pub counts: MigrationCounts,
    pub errors: Vec<String>,
    pub dry_run: bool,
    pub fatal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePaths {
    pub legacy_root: PathBuf,
    pub target_root: PathBuf,
    pub config: PathBuf,
    pub workspace: PathBuf,
    pub markdown: Vec<PathBuf>,
    pub sqlite: PathBuf,
    pub report: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaCompatibility {
    pub legacy_spec: Option<String>,
    pub legacy_version: Option<String>,
    pub expected_schema: String,
    pub status: &'static str,
    pub compatible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationCounts {
    pub markdown_records: usize,
    pub sqlite_records: usize,
    pub merged_records: usize,
    pub imported_records: usize,
}
