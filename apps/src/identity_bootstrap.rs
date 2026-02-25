use std::fs;
use std::path::Path;

pub const DEFAULT_BOOTSTRAP_MARKDOWN_FILES: &[&str] = &[
    "AGENTS.md",
    "SOUL.md",
    "IDENTITY.md",
    "USER.md",
    "HEARTBEAT.md",
    "TOOLS.md",
    "BOOTSTRAP.md",
    "MEMORY.md",
];

pub const DEFAULT_AIEOS_FILE: &str = "AIEOS.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapLoadConfig {
    pub max_total_bytes: usize,
    pub markdown_files: &'static [&'static str],
    pub aieos_file: &'static str,
}

impl Default for BootstrapLoadConfig {
    fn default() -> Self {
        Self {
            max_total_bytes: 32 * 1024,
            markdown_files: DEFAULT_BOOTSTRAP_MARKDOWN_FILES,
            aieos_file: DEFAULT_AIEOS_FILE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapSectionSource {
    MarkdownFile { file_name: String },
    AieosField { field: String },
}

impl BootstrapSectionSource {
    fn label(&self) -> String {
        match self {
            BootstrapSectionSource::MarkdownFile { file_name } => file_name.clone(),
            BootstrapSectionSource::AieosField { field } => format!("AIEOS.{field}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapSection {
    pub source: BootstrapSectionSource,
    pub content: String,
    pub content_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapContext {
    pub sections: Vec<BootstrapSection>,
    pub total_content_bytes: usize,
    pub rendered: String,
}

pub fn load_bootstrap_context(
    root: &Path,
    config: &BootstrapLoadConfig,
) -> Option<BootstrapContext> {
    if config.max_total_bytes == 0 {
        return None;
    }

    let mut sections = Vec::new();
    let mut remaining = config.max_total_bytes;

    for file_name in config.markdown_files {
        if remaining == 0 {
            break;
        }

        let path = root.join(file_name);
        let Ok(contents) = fs::read_to_string(path) else {
            continue;
        };
        let trimmed = contents.trim();
        if trimmed.is_empty() {
            continue;
        }

        let content = truncate_to_bytes(trimmed, remaining);
        if content.is_empty() {
            break;
        }

        remaining = remaining.saturating_sub(content.len());
        sections.push(BootstrapSection {
            source: BootstrapSectionSource::MarkdownFile {
                file_name: (*file_name).to_string(),
            },
            content_bytes: content.len(),
            content,
        });
    }

    if remaining > 0 {
        let path = root.join(config.aieos_file);
        if let Ok(contents) = fs::read_to_string(path)
            && let Some(value) = parse_json_document(&contents)
        {
            for (field, summary) in project_aieos_fields(&value) {
                if remaining == 0 {
                    break;
                }

                let content = truncate_to_bytes(&summary, remaining);
                if content.is_empty() {
                    break;
                }

                remaining = remaining.saturating_sub(content.len());
                sections.push(BootstrapSection {
                    source: BootstrapSectionSource::AieosField { field },
                    content_bytes: content.len(),
                    content,
                });
            }
        }
    }

    if sections.is_empty() {
        return None;
    }

    let total_content_bytes = sections.iter().map(|section| section.content_bytes).sum();
    let rendered = render_bootstrap_sections(&sections);

    Some(BootstrapContext {
        sections,
        total_content_bytes,
        rendered,
    })
}

fn render_bootstrap_sections(sections: &[BootstrapSection]) -> String {
    let mut rendered = String::new();
    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            rendered.push_str("\n\n");
        }

        rendered.push('[');
        rendered.push_str(&section.source.label());
        rendered.push_str("]\n");
        rendered.push_str(&section.content);
    }

    rendered
}

fn parse_json_document(input: &str) -> Option<serde_json::Value> {
    serde_json::from_str(input).ok()
}

fn project_aieos_fields(value: &serde_json::Value) -> Vec<(String, String)> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };

    let mut fields = Vec::with_capacity(object.len());
    for (key, raw) in object {
        let Some(summary) = summarize_json_value(raw, 0) else {
            continue;
        };

        let summary = summary.trim();
        if summary.is_empty() {
            continue;
        }

        fields.push((key.to_owned(), summary.to_string()));
    }

    fields
}

fn summarize_json_value(value: &serde_json::Value, depth: usize) -> Option<String> {
    if depth > 2 {
        return None;
    }

    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(v) => Some(v.to_string()),
        serde_json::Value::Number(v) => Some(v.to_string()),
        serde_json::Value::String(v) => {
            let v = v.trim();
            if v.is_empty() {
                None
            } else {
                Some(v.to_string())
            }
        }
        serde_json::Value::Array(values) => {
            let mut parts = Vec::new();
            for item in values {
                if let Some(summary) = summarize_json_value(item, depth + 1) {
                    let summary = summary.trim();
                    if !summary.is_empty() {
                        parts.push(summary.to_string());
                    }
                }
            }

            if parts.is_empty() {
                None
            } else {
                Some(parts.join(", "))
            }
        }
        serde_json::Value::Object(map) => {
            let mut parts = Vec::new();
            for (key, value) in map {
                let Some(summary) = summarize_json_value(value, depth + 1) else {
                    continue;
                };
                let summary = summary.trim();
                if summary.is_empty() {
                    continue;
                }
                parts.push(format!("{key}={summary}"));
            }

            if parts.is_empty() {
                None
            } else {
                Some(parts.join("; "))
            }
        }
    }
}

fn truncate_to_bytes(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_owned();
    }

    let mut end = 0usize;
    for (index, ch) in input.char_indices() {
        let next = index + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }

    if end == 0 {
        String::new()
    } else {
        input[..end].to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{BootstrapLoadConfig, load_bootstrap_context};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_dir(label: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiom-bootstrap-{label}-{}-{tick}",
            std::process::id()
        ))
    }

    #[test]
    fn loads_markdown_sections_with_stable_rendered_order() {
        let root = unique_dir("markdown");
        fs::create_dir_all(&root).expect("root directory should be created");
        fs::write(root.join("AGENTS.md"), "Agent rules").expect("AGENTS.md should be writable");
        fs::write(root.join("IDENTITY.md"), "Identity profile")
            .expect("IDENTITY.md should be writable");

        let context = load_bootstrap_context(&root, &BootstrapLoadConfig::default())
            .expect("context should load");

        assert_eq!(context.sections.len(), 2);
        assert!(context.rendered.contains("[AGENTS.md]\nAgent rules"));
        assert!(context.rendered.contains("[IDENTITY.md]\nIdentity profile"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn loads_aieos_json_fields_when_markdown_missing() {
        let root = unique_dir("aieos");
        fs::create_dir_all(&root).expect("root directory should be created");
        fs::write(
            root.join("AIEOS.json"),
            "{\"persona\":\"calm\",\"capabilities\":[\"plan\",\"code\"],\"limits\":{\"max_tokens\":2048}}",
        )
        .expect("AIEOS.json should be writable");

        let context = load_bootstrap_context(&root, &BootstrapLoadConfig::default())
            .expect("context should load from AIEOS");

        assert!(!context.sections.is_empty());
        assert!(context.rendered.contains("[AIEOS.capabilities]"));
        assert!(context.rendered.contains("plan, code"));
        assert!(context.rendered.contains("[AIEOS.persona]"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn enforces_total_content_byte_budget() {
        let root = unique_dir("budget");
        fs::create_dir_all(&root).expect("root directory should be created");
        fs::write(root.join("AGENTS.md"), "0123456789abcdef")
            .expect("AGENTS.md should be writable");

        let context = load_bootstrap_context(
            &root,
            &BootstrapLoadConfig {
                max_total_bytes: 8,
                ..BootstrapLoadConfig::default()
            },
        )
        .expect("context should load");

        assert_eq!(context.total_content_bytes, 8);
        assert!(context.rendered.contains("[AGENTS.md]\n01234567"));

        let _ = fs::remove_dir_all(root);
    }
}
