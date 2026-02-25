use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::migrate_types::{
    DEFAULT_EXPECTED_SCHEMA, MigrationCounts, MigrationReport, SchemaCompatibility, SourcePaths,
};

impl MigrationReport {
    pub fn new(source_paths: SourcePaths, expected_schema: String, dry_run: bool) -> Self {
        Self {
            source_paths,
            schema: SchemaCompatibility {
                legacy_spec: None,
                legacy_version: None,
                expected_schema,
                status: "unknown",
                compatible: false,
            },
            counts: MigrationCounts {
                markdown_records: 0,
                sqlite_records: 0,
                merged_records: 0,
                imported_records: 0,
            },
            errors: Vec::new(),
            dry_run,
            fatal: false,
        }
    }

    pub fn argument_failure(error: String) -> Self {
        Self {
            source_paths: SourcePaths {
                legacy_root: PathBuf::new(),
                target_root: PathBuf::new(),
                config: PathBuf::new(),
                workspace: PathBuf::new(),
                markdown: Vec::new(),
                sqlite: PathBuf::new(),
                report: None,
            },
            schema: SchemaCompatibility {
                legacy_spec: None,
                legacy_version: None,
                expected_schema: DEFAULT_EXPECTED_SCHEMA.to_string(),
                status: "unknown",
                compatible: false,
            },
            counts: MigrationCounts {
                markdown_records: 0,
                sqlite_records: 0,
                merged_records: 0,
                imported_records: 0,
            },
            errors: vec![error],
            dry_run: false,
            fatal: true,
        }
    }

    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push('{');

        out.push_str("\"source_paths\":{");
        out.push_str("\"legacy_root\":");
        write_json_path(&mut out, &self.source_paths.legacy_root);
        out.push_str(",\"target_root\":");
        write_json_path(&mut out, &self.source_paths.target_root);
        out.push_str(",\"config\":");
        write_json_path(&mut out, &self.source_paths.config);
        out.push_str(",\"workspace\":");
        write_json_path(&mut out, &self.source_paths.workspace);
        out.push_str(",\"markdown\":[");
        for (index, path) in self.source_paths.markdown.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            write_json_path(&mut out, path);
        }
        out.push(']');
        out.push_str(",\"sqlite\":");
        write_json_path(&mut out, &self.source_paths.sqlite);
        out.push_str(",\"report\":");
        match &self.source_paths.report {
            Some(path) => write_json_path(&mut out, path),
            None => out.push_str("null"),
        }
        out.push('}');

        out.push_str(",\"schema\":{");
        out.push_str("\"legacy_spec\":");
        write_json_optional_string(&mut out, self.schema.legacy_spec.as_deref());
        out.push_str(",\"legacy_version\":");
        write_json_optional_string(&mut out, self.schema.legacy_version.as_deref());
        out.push_str(",\"expected_schema\":");
        write_json_string(&mut out, &self.schema.expected_schema);
        out.push_str(",\"status\":");
        write_json_string(&mut out, self.schema.status);
        out.push_str(",\"compatible\":");
        out.push_str(if self.schema.compatible {
            "true"
        } else {
            "false"
        });
        out.push('}');

        out.push_str(",\"counts\":{");
        out.push_str("\"markdown_records\":");
        out.push_str(&self.counts.markdown_records.to_string());
        out.push_str(",\"sqlite_records\":");
        out.push_str(&self.counts.sqlite_records.to_string());
        out.push_str(",\"merged_records\":");
        out.push_str(&self.counts.merged_records.to_string());
        out.push_str(",\"imported_records\":");
        out.push_str(&self.counts.imported_records.to_string());
        out.push('}');

        out.push_str(",\"errors\":[");
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            write_json_string(&mut out, error);
        }
        out.push(']');

        out.push_str(",\"dry_run\":");
        out.push_str(if self.dry_run { "true" } else { "false" });
        out.push_str(",\"fatal\":");
        out.push_str(if self.fatal { "true" } else { "false" });

        out.push('}');
        out
    }
}

fn write_json_optional_string(out: &mut String, value: Option<&str>) {
    match value {
        Some(value) => write_json_string(out, value),
        None => out.push_str("null"),
    }
}

fn write_json_path(out: &mut String, value: &Path) {
    write_json_string(out, &value.to_string_lossy());
}

fn write_json_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ if character.is_control() => {
                let _ = write!(out, "\\u{:04x}", character as u32);
            }
            _ => out.push(character),
        }
    }
    out.push('"');
}
