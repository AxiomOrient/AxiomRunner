use std::io::Write as IoWrite;
use std::path::PathBuf;

use axiomme_core::{AxiomError, AxiomMe as AxiomSync};

use crate::contracts::{ContextAdapter, ContextDocument, ContextHit};
use crate::error::{AdapterError, AdapterResult, RetryClass};

/// ContextAdapter backed by AxiomSync — stores documents at arbitrary URIs and
/// provides semantic search over URI-scoped document collections.
///
/// Unlike AxiomsyncMemoryAdapter (key/value), this adapter exposes the full
/// AxiomSync URI model: any URI is valid, scores and snippets are preserved.
pub struct AxiomsyncContextAdapter {
    client: AxiomSync,
}

pub type AxiommeContextAdapter = AxiomsyncContextAdapter;

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

impl AxiomsyncContextAdapter {
    /// Initialize (or open) an AxiomSync context rooted at `root_dir`.
    ///
    /// `root_dir` will be created by AxiomSync if it does not exist.
    pub fn new(root_dir: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root_dir.into();
        let client = AxiomSync::new(&root).map_err(|e| format!("AxiomSync::new failed: {e}"))?;
        client
            .initialize()
            .map_err(|e| format!("AxiomSync::initialize failed: {e}"))?;
        Ok(Self { client })
    }
}

impl ContextAdapter for AxiomsyncContextAdapter {
    fn id(&self) -> &str {
        "axiomsync"
    }

    /// Store `content` at the given AxiomSync URI.
    ///
    /// Implementation: write to a temp file → add_resource → delete temp file.
    /// The URI is used directly (no `.md` suffix appended).
    fn store_document(&self, uri: &str, content: &str) -> AdapterResult<()> {
        // Derive a safe temp file name from the URI.
        // Timestamp (nanoseconds) + thread ID ensure uniqueness under concurrent calls.
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let tid = format!("{:?}", std::thread::current().id()).replace(['(', ')'], "");
        let safe_name = uri.replace(['/', ':'], "_");
        let tmp_path =
            std::env::temp_dir().join(format!("axiomsync_ctx_{safe_name}_{ts}_{tid}.md"));
        let tmp_path = TempPathCleanup::new(tmp_path);

        {
            let mut f = std::fs::File::create(tmp_path.path()).map_err(|e| {
                AdapterError::failed(
                    "axiomsync_ctx.store.tmpfile",
                    e.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;
            f.write_all(content.as_bytes()).map_err(|e| {
                AdapterError::failed(
                    "axiomsync_ctx.store.write",
                    e.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;
        }

        let tmp_str = tmp_path.path().to_string_lossy().into_owned();
        self.client
            .add_resource(&tmp_str, Some(uri), None, None, true, None)
            .map_err(|e| {
                AdapterError::failed(
                    "axiomsync_ctx.store.add",
                    e.to_string(),
                    RetryClass::Retryable,
                )
            })?;
        Ok(())
    }

    /// Semantic search over documents whose URI starts with `scope_uri`.
    ///
    /// Returns ranked hits with scores (f32) and snippets from AxiomSync.
    /// Allocates O(limit) ContextHit values.
    fn semantic_search(
        &self,
        query: &str,
        scope_uri: &str,
        limit: usize,
    ) -> AdapterResult<Vec<ContextHit>> {
        let limit_opt = if limit == 0 { None } else { Some(limit) };

        let result = match self
            .client
            .find(query, Some(scope_uri), limit_opt, None, None)
        {
            Ok(r) => r,
            Err(AxiomError::NotFound(_)) => return Ok(Vec::new()),
            Err(e) => {
                return Err(AdapterError::failed(
                    "axiomsync_ctx.search",
                    e.to_string(),
                    RetryClass::NonRetryable,
                ));
            }
        };

        let hits = result
            .query_results
            .into_iter()
            .map(|hit| ContextHit {
                uri: hit.uri,
                score: hit.score,
                snippet: hit.abstract_text,
                // Full content is not fetched during search — caller can use
                // get_document() if the full document is needed.
                content: String::new(),
            })
            .collect();

        Ok(hits)
    }

    /// Retrieve the full document stored at `uri`. Returns None if not found.
    fn get_document(&self, uri: &str) -> AdapterResult<Option<ContextDocument>> {
        match self.client.read(uri) {
            Ok(content) => Ok(Some(ContextDocument {
                uri: uri.to_string(),
                content,
            })),
            Err(AxiomError::NotFound(_)) => Ok(None),
            Err(e) => Err(AdapterError::failed(
                "axiomsync_ctx.get",
                e.to_string(),
                RetryClass::NonRetryable,
            )),
        }
    }

    /// Delete the document at `uri`. Returns true if it existed.
    fn remove_document(&self, uri: &str) -> AdapterResult<bool> {
        match self.client.rm(uri, false) {
            Ok(()) => Ok(true),
            Err(AxiomError::NotFound(_)) => Ok(false),
            Err(e) => Err(AdapterError::failed(
                "axiomsync_ctx.remove",
                e.to_string(),
                RetryClass::NonRetryable,
            )),
        }
    }

    /// Append a conversation message to a session scope.
    ///
    /// Stored at: `{session_prefix_uri}/{role}_{unix_millis}`
    /// This gives each message a unique, ordered URI within the session.
    fn store_session_message(
        &self,
        session_prefix_uri: &str,
        role: &str,
        content: &str,
    ) -> AdapterResult<()> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let uri = format!("{session_prefix_uri}/{role}_{ts}");
        self.store_document(&uri, content)
    }

    /// Semantic search within a session scope.
    ///
    /// Delegates to semantic_search with the session prefix as the scope URI.
    fn recall_with_session(
        &self,
        query: &str,
        session_prefix_uri: &str,
        limit: usize,
    ) -> AdapterResult<Vec<ContextHit>> {
        self.semantic_search(query, session_prefix_uri, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIVE_ENV: &str = "AXONRUNNER_RUN_AXIOMSYNC_LIVE";
    const LEGACY_LIVE_ENV: &str = "AXONRUNNER_RUN_AXIOMME_LIVE";

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn new_initializes_successfully() {
        let dir = tmp_dir("axiomsync_ctx_init");
        let result = AxiomsyncContextAdapter::new(&dir);
        assert!(result.is_ok(), "init failed: {:?}", result.err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn id_returns_axiomsync() {
        let dir = tmp_dir("axiomsync_ctx_id");
        if let Ok(a) = AxiomsyncContextAdapter::new(&dir) {
            assert_eq!(a.id(), "axiomsync");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[ignore]
    fn store_and_search_roundtrip() {
        if std::env::var_os(LIVE_ENV).is_none() && std::env::var_os(LEGACY_LIVE_ENV).is_none() {
            eprintln!("skipping store_and_search_roundtrip: set {LIVE_ENV}=1 to enable");
            return;
        }
        let dir = tmp_dir("axiomsync_ctx_roundtrip");
        let a = AxiomsyncContextAdapter::new(&dir).unwrap();
        a.store_document("axonrunner://agent/memory/test-doc", "semantic hello world")
            .unwrap();
        let hits = a
            .semantic_search("hello", "axonrunner://agent/memory", 5)
            .unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].score > 0.0);
        a.remove_document("axonrunner://agent/memory/test-doc")
            .unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }
}
