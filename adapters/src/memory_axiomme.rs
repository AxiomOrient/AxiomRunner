use std::io::Write as IoWrite;
use std::path::PathBuf;

use axiomme_core::{AxiomError, AxiomMe};

use crate::contracts::{AdapterHealth, MemoryAdapter, MemoryEntry};
use crate::error::{AdapterError, AdapterResult, RetryClass};
use crate::memory::now_millis;

/// MemoryAdapter backed by AxiomMe — stores key/value pairs as flat markdown
/// files under `axiom://agent/memory/<key>` in the AxiomMe context root.
pub struct AxiommeMemoryAdapter {
    client: AxiomMe,
}

impl AxiommeMemoryAdapter {
    const MEMORY_URI: &'static str = "axiom://agent/memory";

    /// Initialize (or open) an AxiomMe context rooted at `root_dir`.
    ///
    /// `root_dir` will be created if it does not exist.
    pub fn new(root_dir: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root_dir.into();
        let client = AxiomMe::new(&root)
            .map_err(|e| format!("AxiomMe::new failed: {e}"))?;
        client.initialize()
            .map_err(|e| format!("AxiomMe::initialize failed: {e}"))?;
        Ok(Self { client })
    }

    fn uri_for(key: &str) -> String {
        format!("{}/{}.md", Self::MEMORY_URI, key)
    }

    fn key_from_uri(uri: &str) -> Option<String> {
        let segment = uri.split('/').next_back()?;
        // Strip the `.md` suffix that uri_for adds.
        if let Some(stem) = segment.strip_suffix(".md")
            && !stem.is_empty() {
                return Some(stem.to_string());
            }
        // Fall back to the bare segment (directories, legacy entries).
        if !segment.is_empty() {
            Some(segment.to_string())
        } else {
            None
        }
    }
}

impl MemoryAdapter for AxiommeMemoryAdapter {
    fn id(&self) -> &str {
        "axiomme"
    }

    fn health(&self) -> AdapterHealth {
        match self.client.ls(Self::MEMORY_URI, false, true) {
            Ok(_) => AdapterHealth::Healthy,
            Err(AxiomError::NotFound(_)) => AdapterHealth::Healthy,
            Err(_) => AdapterHealth::Degraded,
        }
    }

    fn store(&mut self, key: &str, value: &str) -> AdapterResult<()> {
        // Write value to a temp file so add_resource can ingest it.
        // Timestamp (nanoseconds) + thread ID ensure uniqueness under concurrent calls.
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let tid = format!("{:?}", std::thread::current().id())
            .replace(['(', ')'], "");
        let tmp_path = std::env::temp_dir()
            .join(format!("axiomme_store_{}_{ts}_{tid}.md", key.replace('/', "_")));
        {
            let mut f = std::fs::File::create(&tmp_path).map_err(|e| {
                AdapterError::failed(
                    "axiomme.store.tmpfile",
                    e.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;
            f.write_all(value.as_bytes()).map_err(|e| {
                AdapterError::failed(
                    "axiomme.store.write",
                    e.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;
        }

        let uri = Self::uri_for(key);
        let tmp_str = tmp_path.to_string_lossy().into_owned();

        let result = self
            .client
            .add_resource(&tmp_str, Some(&uri), None, None, true, None)
            .map_err(|e| {
                AdapterError::failed("axiomme.store.add", e.to_string(), RetryClass::Retryable)
            });

        // Always clean up the temp file regardless of success/failure.
        let _ = std::fs::remove_file(&tmp_path);

        result?;
        Ok(())
    }

    fn get(&self, key: &str) -> AdapterResult<Option<MemoryEntry>> {
        let uri = Self::uri_for(key);
        match self.client.read(&uri) {
            Ok(content) => Ok(Some(MemoryEntry {
                key: key.to_string(),
                value: content,
                updated_at: now_millis(),
            })),
            Err(AxiomError::NotFound(_)) => Ok(None),
            Err(e) => Err(AdapterError::failed(
                "axiomme.get",
                e.to_string(),
                RetryClass::NonRetryable,
            )),
        }
    }

    fn list(&self) -> AdapterResult<Vec<MemoryEntry>> {
        let entries = match self.client.ls(Self::MEMORY_URI, true, true) {
            Ok(entries) => entries,
            Err(AxiomError::NotFound(_)) => return Ok(Vec::new()),
            Err(e) => {
                return Err(AdapterError::failed(
                    "axiomme.list",
                    e.to_string(),
                    RetryClass::NonRetryable,
                ));
            }
        };

        let memory_entries = entries
            .into_iter()
            .filter(|entry| !entry.is_dir)
            .filter_map(|entry| {
                let key = Self::key_from_uri(&entry.uri)?;
                let uri = Self::uri_for(&key);
                let value = self.client.read(&uri).unwrap_or_default();
                Some(MemoryEntry {
                    key,
                    value,
                    updated_at: 0,
                })
            })
            .collect();

        Ok(memory_entries)
    }

    fn recall(&self, query: &str, limit: usize) -> AdapterResult<Vec<MemoryEntry>> {
        let limit_opt = if limit == 0 { None } else { Some(limit) };
        let result = match self
            .client
            .find(query, Some(Self::MEMORY_URI), limit_opt, None, None)
        {
            Ok(r) => r,
            Err(AxiomError::NotFound(_)) => return Ok(Vec::new()),
            Err(e) => {
                return Err(AdapterError::failed(
                    "axiomme.recall",
                    e.to_string(),
                    RetryClass::NonRetryable,
                ));
            }
        };

        let entries = result
            .query_results
            .into_iter()
            .filter_map(|hit| {
                let key = Self::key_from_uri(&hit.uri)?;
                Some(MemoryEntry {
                    key,
                    value: hit.abstract_text,
                    updated_at: 0,
                })
            })
            .collect();

        Ok(entries)
    }

    fn delete(&mut self, key: &str) -> AdapterResult<bool> {
        let uri = Self::uri_for(key);
        match self.client.rm(&uri, false) {
            Ok(()) => Ok(true),
            Err(AxiomError::NotFound(_)) => Ok(false),
            Err(e) => Err(AdapterError::failed(
                "axiomme.delete",
                e.to_string(),
                RetryClass::NonRetryable,
            )),
        }
    }

    fn count(&self) -> AdapterResult<usize> {
        self.list().map(|v| v.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("axiomme_adapter_test_{suffix}"))
    }

    #[test]
    fn new_initializes_without_error() {
        let dir = test_dir("init");
        std::fs::create_dir_all(&dir).unwrap();
        let result = AxiommeMemoryAdapter::new(&dir);
        std::fs::remove_dir_all(&dir).ok();
        assert!(result.is_ok(), "init failed: {:?}", result.err());
    }

    #[test]
    fn id_returns_axiomme() {
        let dir = test_dir("id");
        std::fs::create_dir_all(&dir).unwrap();
        let adapter_result = AxiommeMemoryAdapter::new(&dir);
        std::fs::remove_dir_all(&dir).ok();
        if let Ok(adapter) = adapter_result {
            assert_eq!(adapter.id(), "axiomme");
        } else {
            panic!("adapter init failed");
        }
    }

    #[test]
    fn key_from_uri_strips_md_suffix() {
        assert_eq!(
            AxiommeMemoryAdapter::key_from_uri("axiom://agent/memory/hello.md"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn key_from_uri_returns_none_for_empty_segment() {
        assert_eq!(AxiommeMemoryAdapter::key_from_uri("axiom://agent/memory/"), None);
    }

    #[test]
    fn uri_for_produces_expected_path() {
        assert_eq!(
            AxiommeMemoryAdapter::uri_for("some_key"),
            "axiom://agent/memory/some_key.md"
        );
    }

    /// Full round-trip: store → get → delete.
    /// Requires a working AxiomMe setup; may be slow due to semantic processing.
    #[test]
    #[ignore]
    fn store_get_delete_roundtrip() {
        let dir = test_dir("roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let mut adapter = AxiommeMemoryAdapter::new(&dir).expect("adapter init");
        adapter.store("test_key", "hello world").expect("store");
        let entry = adapter.get("test_key").expect("get");
        assert!(entry.is_some(), "entry should exist after store");
        assert!(
            entry.unwrap().value.contains("hello world"),
            "value should contain stored text"
        );
        let deleted = adapter.delete("test_key").expect("delete");
        assert!(deleted, "delete should return true for existing key");
        let after = adapter.get("test_key").expect("get after delete");
        assert!(after.is_none(), "entry should be gone after delete");
        std::fs::remove_dir_all(&dir).ok();
    }
}
