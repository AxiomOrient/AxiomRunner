use std::io::Write as IoWrite;
use std::path::PathBuf;

use axiomme_core::{AxiomError, AxiomMe as AxiomSync};

use crate::contracts::{AdapterHealth, MemoryAdapter, MemoryEntry};
use crate::error::{AdapterError, AdapterResult, RetryClass};
use crate::memory::now_millis;

/// MemoryAdapter backed by AxiomSync — stores key/value pairs as flat markdown
/// files under `axonrunner://agent/memory/<key>` in the AxiomSync context root.
pub struct AxiomsyncMemoryAdapter {
    client: AxiomSync,
}

pub type AxiommeMemoryAdapter = AxiomsyncMemoryAdapter;

struct TempPathCleanup {
    path: PathBuf,
}

impl TempPathCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempPathCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl AxiomsyncMemoryAdapter {
    const MEMORY_URI: &'static str = "axonrunner://agent/memory";
    const LEGACY_MEMORY_URI: &'static str = "axiom://agent/memory";

    /// Initialize (or open) an AxiomSync context rooted at `root_dir`.
    ///
    /// `root_dir` will be created if it does not exist.
    pub fn new(root_dir: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root_dir.into();
        let client = AxiomSync::new(&root).map_err(|e| format!("AxiomSync::new failed: {e}"))?;
        client
            .initialize()
            .map_err(|e| format!("AxiomSync::initialize failed: {e}"))?;
        Ok(Self { client })
    }

    fn uri_for(key: &str) -> String {
        format!("{}/{}.md", Self::MEMORY_URI, key)
    }

    fn legacy_uri_for(key: &str) -> String {
        format!("{}/{}.md", Self::LEGACY_MEMORY_URI, key)
    }

    fn key_from_uri(uri: &str) -> Option<String> {
        let segment = uri.split('/').next_back()?;
        // Strip the `.md` suffix that uri_for adds.
        if let Some(stem) = segment.strip_suffix(".md")
            && !stem.is_empty()
        {
            return Some(stem.to_string());
        }
        // Fall back to the bare segment (directories, legacy entries).
        if !segment.is_empty() {
            Some(segment.to_string())
        } else {
            None
        }
    }

    fn indexed_memory_values(&self) -> std::collections::HashMap<String, String> {
        self.client
            .state
            .list_search_documents()
            .map(|docs| {
                docs.into_iter()
                    .filter(|doc| {
                        doc.uri
                            .starts_with(format!("{}/", Self::MEMORY_URI).as_str())
                            || doc
                                .uri
                                .starts_with(format!("{}/", Self::LEGACY_MEMORY_URI).as_str())
                    })
                    .map(|doc| (doc.uri, doc.content))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn list_documents_at(&self, uri: &str) -> AdapterResult<Vec<(String, bool)>> {
        match self.client.ls(uri, true, true) {
            Ok(entries) => Ok(entries
                .into_iter()
                .map(|entry| (entry.uri, entry.is_dir))
                .collect()),
            Err(AxiomError::NotFound(_)) => Ok(Vec::new()),
            Err(error) => Err(AdapterError::failed(
                "axiomsync.list",
                error.to_string(),
                RetryClass::NonRetryable,
            )),
        }
    }

    fn recall_documents_at(
        &self,
        query: &str,
        scope_uri: &str,
        limit: usize,
    ) -> AdapterResult<Vec<String>> {
        let limit_opt = if limit == 0 { None } else { Some(limit) };
        match self
            .client
            .find(query, Some(scope_uri), limit_opt, None, None)
        {
            Ok(result) => Ok(result
                .query_results
                .into_iter()
                .map(|hit| hit.uri)
                .collect()),
            Err(AxiomError::NotFound(_)) => Ok(Vec::new()),
            Err(error) => Err(AdapterError::failed(
                "axiomsync.recall",
                error.to_string(),
                RetryClass::NonRetryable,
            )),
        }
    }

    fn value_for_uri(
        &self,
        uri: &str,
        indexed_values: &std::collections::HashMap<String, String>,
    ) -> String {
        indexed_values
            .get(uri)
            .cloned()
            .unwrap_or_else(|| self.client.read(uri).unwrap_or_default())
    }
}

impl MemoryAdapter for AxiomsyncMemoryAdapter {
    fn id(&self) -> &str {
        "axiomsync"
    }

    fn health(&self) -> AdapterHealth {
        for scope in [Self::MEMORY_URI, Self::LEGACY_MEMORY_URI] {
            match self.client.ls(scope, false, true) {
                Ok(_) | Err(AxiomError::NotFound(_)) => return AdapterHealth::Healthy,
                Err(_) => continue,
            }
        }
        AdapterHealth::Degraded
    }

    fn store(&self, key: &str, value: &str) -> AdapterResult<()> {
        // Write value to a temp file so add_resource can ingest it.
        // Timestamp (nanoseconds) + thread ID ensure uniqueness under concurrent calls.
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let tid = format!("{:?}", std::thread::current().id()).replace(['(', ')'], "");
        let tmp_path = std::env::temp_dir().join(format!(
            "axiomsync_store_{}_{ts}_{tid}.md",
            key.replace('/', "_")
        ));
        let tmp_path = TempPathCleanup::new(tmp_path);
        {
            let mut f = std::fs::File::create(tmp_path.path()).map_err(|e| {
                AdapterError::failed(
                    "axiomsync.store.tmpfile",
                    e.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;
            f.write_all(value.as_bytes()).map_err(|e| {
                AdapterError::failed(
                    "axiomsync.store.write",
                    e.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;
        }

        let uri = Self::uri_for(key);
        let tmp_str = tmp_path.path().to_string_lossy().into_owned();
        self.client
            .add_resource(&tmp_str, Some(&uri), None, None, true, None)
            .map_err(|e| {
                AdapterError::failed("axiomsync.store.add", e.to_string(), RetryClass::Retryable)
            })?;
        Ok(())
    }

    fn get(&self, key: &str) -> AdapterResult<Option<MemoryEntry>> {
        for uri in [Self::uri_for(key), Self::legacy_uri_for(key)] {
            match self.client.read(&uri) {
                Ok(content) => {
                    return Ok(Some(MemoryEntry {
                        key: key.to_string(),
                        value: content,
                        updated_at: now_millis(),
                    }));
                }
                Err(AxiomError::NotFound(_)) => continue,
                Err(error) => {
                    return Err(AdapterError::failed(
                        "axiomsync.get",
                        error.to_string(),
                        RetryClass::NonRetryable,
                    ));
                }
            }
        }
        Ok(None)
    }

    fn list(&self) -> AdapterResult<Vec<MemoryEntry>> {
        let indexed_values = self.indexed_memory_values();
        let mut deduped = std::collections::BTreeMap::new();
        for entry in self
            .list_documents_at(Self::MEMORY_URI)?
            .into_iter()
            .chain(self.list_documents_at(Self::LEGACY_MEMORY_URI)?)
        {
            let (uri, is_dir) = entry;
            if is_dir {
                continue;
            }
            let Some(key) = Self::key_from_uri(&uri) else {
                continue;
            };
            deduped.entry(key.clone()).or_insert_with(|| MemoryEntry {
                key,
                value: self.value_for_uri(&uri, &indexed_values),
                updated_at: 0,
            });
        }

        Ok(deduped.into_values().collect())
    }

    fn recall(&self, query: &str, limit: usize) -> AdapterResult<Vec<MemoryEntry>> {
        let indexed_values = self.indexed_memory_values();
        let mut hits = self.recall_documents_at(query, Self::MEMORY_URI, limit)?;
        if limit == 0 || hits.len() < limit {
            let legacy_limit = if limit == 0 {
                0
            } else {
                limit.saturating_sub(hits.len())
            };
            hits.extend(self.recall_documents_at(query, Self::LEGACY_MEMORY_URI, legacy_limit)?);
        }

        let mut deduped = std::collections::BTreeMap::new();
        for uri in hits {
            let Some(key) = Self::key_from_uri(&uri) else {
                continue;
            };
            deduped.entry(key.clone()).or_insert_with(|| MemoryEntry {
                key,
                value: self.value_for_uri(&uri, &indexed_values),
                updated_at: 0,
            });
            if limit != 0 && deduped.len() >= limit {
                break;
            }
        }

        Ok(deduped.into_values().collect())
    }

    fn delete(&self, key: &str) -> AdapterResult<bool> {
        let mut deleted = false;
        for uri in [Self::uri_for(key), Self::legacy_uri_for(key)] {
            match self.client.rm(&uri, false) {
                Ok(()) => deleted = true,
                Err(AxiomError::NotFound(_)) => {}
                Err(error) => {
                    return Err(AdapterError::failed(
                        "axiomsync.delete",
                        error.to_string(),
                        RetryClass::NonRetryable,
                    ));
                }
            }
        }
        Ok(deleted)
    }

    fn count(&self) -> AdapterResult<usize> {
        Ok(self.list()?.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIVE_ENV: &str = "AXONRUNNER_RUN_AXIOMSYNC_LIVE";
    const LEGACY_LIVE_ENV: &str = "AXONRUNNER_RUN_AXIOMME_LIVE";

    fn test_dir(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("axiomsync_adapter_test_{suffix}"))
    }

    #[test]
    fn new_initializes_without_error() {
        let dir = test_dir("init");
        std::fs::create_dir_all(&dir).unwrap();
        let result = AxiomsyncMemoryAdapter::new(&dir);
        std::fs::remove_dir_all(&dir).ok();
        assert!(result.is_ok(), "init failed: {:?}", result.err());
    }

    #[test]
    fn id_returns_axiomsync() {
        let dir = test_dir("id");
        std::fs::create_dir_all(&dir).unwrap();
        let adapter_result = AxiomsyncMemoryAdapter::new(&dir);
        std::fs::remove_dir_all(&dir).ok();
        if let Ok(adapter) = adapter_result {
            assert_eq!(adapter.id(), "axiomsync");
        } else {
            panic!("adapter init failed");
        }
    }

    #[test]
    fn key_from_uri_strips_md_suffix() {
        assert_eq!(
            AxiomsyncMemoryAdapter::key_from_uri("axonrunner://agent/memory/hello.md"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn key_from_uri_returns_none_for_empty_segment() {
        assert_eq!(
            AxiomsyncMemoryAdapter::key_from_uri("axonrunner://agent/memory/"),
            None
        );
    }

    #[test]
    fn uri_for_produces_expected_path() {
        assert_eq!(
            AxiomsyncMemoryAdapter::uri_for("some_key"),
            "axonrunner://agent/memory/some_key.md"
        );
    }

    /// Full round-trip: store → get → delete.
    /// Requires a working AxiomSync setup; may be slow due to semantic processing.
    #[test]
    #[ignore]
    fn store_get_delete_roundtrip() {
        if std::env::var_os(LIVE_ENV).is_none() && std::env::var_os(LEGACY_LIVE_ENV).is_none() {
            eprintln!("skipping store_get_delete_roundtrip: set {LIVE_ENV}=1 to enable");
            return;
        }
        let dir = test_dir("roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let adapter = AxiomsyncMemoryAdapter::new(&dir).expect("adapter init");
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
