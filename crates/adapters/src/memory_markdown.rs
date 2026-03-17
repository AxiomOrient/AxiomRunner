use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{AdapterError, AdapterResult, RetryClass};
use crate::memory::{
    MemoryError, MemoryRecord, MemoryResult, create_parent_dir, hex_decode, hex_encode, now_millis,
    record_terms, sort_records, tokenize_terms,
};

#[derive(Debug)]
pub struct MarkdownMemoryAdapter {
    file_path: PathBuf,
    state: Mutex<MarkdownMemoryState>,
}

#[derive(Debug, Clone)]
struct MarkdownMemoryState {
    records: BTreeMap<String, MemoryRecord>,
    index: MarkdownIndexState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MarkdownIndexState {
    term_to_keys: BTreeMap<String, BTreeSet<String>>,
    key_to_terms: BTreeMap<String, Vec<String>>,
}

impl MarkdownIndexState {
    fn from_records(records: &BTreeMap<String, MemoryRecord>) -> Self {
        let mut state = Self::default();
        for record in records.values() {
            state.upsert_record(record);
        }
        state
    }

    fn upsert_record(&mut self, record: &MemoryRecord) {
        self.remove_key(&record.key);

        let terms = record_terms(&record.key, &record.value);
        for term in &terms {
            self.term_to_keys
                .entry(term.clone())
                .or_default()
                .insert(record.key.clone());
        }

        self.key_to_terms.insert(record.key.clone(), terms);
    }

    fn remove_key(&mut self, key: &str) {
        let Some(terms) = self.key_to_terms.remove(key) else {
            return;
        };

        for term in terms {
            let should_remove = match self.term_to_keys.get_mut(&term) {
                Some(keys) => {
                    keys.remove(key);
                    keys.is_empty()
                }
                None => false,
            };

            if should_remove {
                self.term_to_keys.remove(&term);
            }
        }
    }

    fn matched_keys(&self, query: &str) -> Vec<String> {
        let terms = tokenize_terms(query);
        if terms.is_empty() {
            return Vec::new();
        }

        let mut candidates: Option<BTreeSet<String>> = None;
        for term in terms {
            let Some(keys) = self.term_to_keys.get(&term) else {
                return Vec::new();
            };

            candidates = Some(match candidates {
                Some(existing) => existing.intersection(keys).cloned().collect(),
                None => keys.clone(),
            });

            if candidates.as_ref().is_some_and(BTreeSet::is_empty) {
                return Vec::new();
            }
        }

        candidates
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<String>>()
    }
}

impl MarkdownMemoryAdapter {
    pub fn new(path: impl Into<PathBuf>) -> AdapterResult<Self> {
        let file_path = path.into();
        create_parent_dir(&file_path).map_err(|e| map_memory_error("memory.markdown.new", e))?;

        let records = if file_path.exists() {
            load_markdown_records(&file_path)
                .map_err(|e| map_memory_error("memory.markdown.new", e))?
        } else {
            BTreeMap::new()
        };

        let adapter = Self {
            file_path,
            state: Mutex::new(MarkdownMemoryState {
                index: MarkdownIndexState::from_records(&records),
                records,
            }),
        };
        {
            let state = adapter.state.lock().map_err(|_| {
                map_memory_error(
                    "memory.markdown.new",
                    MemoryError::Backend(String::from("state lock poisoned")),
                )
            })?;
            adapter
                .persist_records(&state.records)
                .map_err(|e| map_memory_error("memory.markdown.new", e))?;
        }
        Ok(adapter)
    }

    fn persist_records(&self, records: &BTreeMap<String, MemoryRecord>) -> MemoryResult<()> {
        let mut content = String::from(
            "# AxiomRunner Markdown Memory\n\n<!-- format: axiomrunner-memory-markdown-v1 -->\n",
        );

        for record in records.values() {
            let line = format!(
                "- key_hex={};updated_at={};value_hex={}\n",
                hex_encode(record.key.as_bytes()),
                record.updated_at,
                hex_encode(record.value.as_bytes())
            );
            content.push_str(&line);
        }

        let temp_path = self.file_path.with_extension("tmp");
        fs::write(&temp_path, &content)?;
        match fs::rename(&temp_path, &self.file_path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                fs::remove_file(&self.file_path)?;
                fs::rename(&temp_path, &self.file_path)?;
            }
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                return Err(error.into());
            }
        }
        Ok(())
    }
}

impl crate::contracts::MemoryAdapter for MarkdownMemoryAdapter {
    fn id(&self) -> &str {
        "memory.markdown"
    }

    fn health(&self) -> crate::contracts::AdapterHealth {
        let write_check = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .map(|_| true)
            .unwrap_or(false);
        if write_check {
            crate::contracts::AdapterHealth::Healthy
        } else {
            crate::contracts::AdapterHealth::Degraded
        }
    }

    fn store(&self, key: &str, value: &str) -> AdapterResult<()> {
        let mut state = self.state.lock().map_err(|_| {
            AdapterError::failed(
                "memory.markdown.store",
                "state lock poisoned",
                RetryClass::NonRetryable,
            )
        })?;
        if state
            .records
            .get(key)
            .is_some_and(|record| record.value == value)
        {
            return Ok(());
        }
        let record = MemoryRecord {
            key: key.to_string(),
            value: value.to_string(),
            updated_at: now_millis(),
        };
        let mut next_records = state.records.clone();
        let mut next_index = state.index.clone();
        next_records.insert(key.to_string(), record.clone());
        next_index.upsert_record(&record);

        self.persist_records(&next_records)
            .map_err(|e| map_memory_error("memory.markdown.store", e))?;

        state.records = next_records;
        state.index = next_index;
        Ok(())
    }

    fn recall(
        &self,
        query: &str,
        limit: usize,
    ) -> AdapterResult<Vec<crate::contracts::MemoryEntry>> {
        let state = self.state.lock().map_err(|_| {
            AdapterError::failed(
                "memory.markdown.recall",
                "state lock poisoned",
                RetryClass::NonRetryable,
            )
        })?;
        let mut records = if query.trim().is_empty() {
            state
                .records
                .values()
                .cloned()
                .collect::<Vec<MemoryRecord>>()
        } else {
            state
                .index
                .matched_keys(query)
                .into_iter()
                .filter_map(|key| state.records.get(&key).cloned())
                .collect::<Vec<MemoryRecord>>()
        };
        sort_records(&mut records);
        if limit > 0 {
            records.truncate(limit);
        }
        Ok(records.into_iter().map(record_to_entry).collect())
    }

    fn get(&self, key: &str) -> AdapterResult<Option<crate::contracts::MemoryEntry>> {
        let state = self.state.lock().map_err(|_| {
            AdapterError::failed(
                "memory.markdown.get",
                "state lock poisoned",
                RetryClass::NonRetryable,
            )
        })?;
        Ok(state.records.get(key).cloned().map(record_to_entry))
    }

    fn list(&self) -> AdapterResult<Vec<crate::contracts::MemoryEntry>> {
        let state = self.state.lock().map_err(|_| {
            AdapterError::failed(
                "memory.markdown.list",
                "state lock poisoned",
                RetryClass::NonRetryable,
            )
        })?;
        let mut records: Vec<MemoryRecord> = state.records.values().cloned().collect();
        sort_records(&mut records);
        Ok(records.into_iter().map(record_to_entry).collect())
    }

    fn delete(&self, key: &str) -> AdapterResult<bool> {
        let mut state = self.state.lock().map_err(|_| {
            AdapterError::failed(
                "memory.markdown.delete",
                "state lock poisoned",
                RetryClass::NonRetryable,
            )
        })?;
        let mut next_records = state.records.clone();
        let existed = next_records.remove(key).is_some();
        if existed {
            let mut next_index = state.index.clone();
            next_index.remove_key(key);

            self.persist_records(&next_records)
                .map_err(|e| map_memory_error("memory.markdown.delete", e))?;

            state.records = next_records;
            state.index = next_index;
        }
        Ok(existed)
    }

    fn count(&self) -> AdapterResult<usize> {
        let state = self.state.lock().map_err(|_| {
            AdapterError::failed(
                "memory.markdown.count",
                "state lock poisoned",
                RetryClass::NonRetryable,
            )
        })?;
        Ok(state.records.len())
    }
}

fn record_to_entry(record: MemoryRecord) -> crate::contracts::MemoryEntry {
    crate::contracts::MemoryEntry {
        key: record.key,
        value: record.value,
        updated_at: record.updated_at,
    }
}

fn load_markdown_records(path: &Path) -> MemoryResult<BTreeMap<String, MemoryRecord>> {
    let content = fs::read_to_string(path)?;
    let mut records = BTreeMap::new();

    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with("- key_hex=") {
            continue;
        }

        let payload = &line[2..];
        let mut key_hex = None;
        let mut value_hex = None;
        let mut updated_at = None;

        for pair in payload.split(';') {
            let mut split = pair.splitn(2, '=');
            let field = split.next().unwrap_or_default().trim();
            let value = split.next().unwrap_or_default().trim();

            match field {
                "key_hex" => key_hex = Some(value.to_string()),
                "value_hex" => value_hex = Some(value.to_string()),
                "updated_at" => {
                    let parsed = value.parse::<u64>().map_err(|error| {
                        MemoryError::InvalidData(format!(
                            "invalid markdown updated_at `{value}`: {error}"
                        ))
                    })?;
                    updated_at = Some(parsed);
                }
                _ => {}
            }
        }

        let key_hex = key_hex.ok_or_else(|| {
            MemoryError::InvalidData(format!("missing key_hex in markdown line `{line}`"))
        })?;
        let value_hex = value_hex.ok_or_else(|| {
            MemoryError::InvalidData(format!("missing value_hex in markdown line `{line}`"))
        })?;
        let updated_at = updated_at.ok_or_else(|| {
            MemoryError::InvalidData(format!("missing updated_at in markdown line `{line}`"))
        })?;

        let key_bytes = hex_decode(&key_hex).ok_or_else(|| {
            MemoryError::InvalidData(format!("invalid hex key in markdown line `{line}`"))
        })?;
        let key = String::from_utf8(key_bytes).map_err(|error| {
            MemoryError::InvalidData(format!(
                "invalid utf8 key in markdown line `{line}`: {error}"
            ))
        })?;
        let value_bytes = hex_decode(&value_hex).ok_or_else(|| {
            MemoryError::InvalidData(format!("invalid hex value in markdown line `{line}`"))
        })?;
        let value = String::from_utf8(value_bytes).map_err(|error| {
            MemoryError::InvalidData(format!(
                "invalid utf8 value in markdown line `{line}`: {error}"
            ))
        })?;

        records.insert(
            key.clone(),
            MemoryRecord {
                key,
                value,
                updated_at,
            },
        );
    }

    Ok(records)
}

fn map_memory_error(operation: &'static str, error: MemoryError) -> AdapterError {
    match error {
        MemoryError::Io(inner) => {
            AdapterError::failed(operation, inner.to_string(), RetryClass::Retryable)
        }
        MemoryError::Backend(reason) => {
            AdapterError::failed(operation, reason, RetryClass::Retryable)
        }
        MemoryError::InvalidData(reason) => {
            AdapterError::failed(operation, reason, RetryClass::NonRetryable)
        }
    }
}
