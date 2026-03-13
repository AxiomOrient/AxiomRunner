use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use rusqlite::{Connection, ToSql, TransactionBehavior, params, params_from_iter};

use crate::error::{AdapterError, AdapterResult, RetryClass};
use crate::memory::{
    MemoryError, MemoryRecord, MemoryResult, create_parent_dir, hex_decode, hex_encode, now_millis,
    record_terms, tokenize_terms,
};

const HEALTH_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct HealthCache {
    result: crate::contracts::AdapterHealth,
    checked_at: Instant,
}

#[derive(Debug, Clone)]
pub struct SqliteMemoryAdapter {
    connection: Arc<Mutex<Connection>>,
    health_cache: Arc<Mutex<Option<HealthCache>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SqliteIndexedRecord {
    key_hex: String,
    value_hex: String,
    updated_at: u64,
    terms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SqliteRecallPlan {
    include_all: bool,
    terms: Vec<String>,
    limit: usize,
}

impl SqliteIndexedRecord {
    fn from_input(key: &str, value: &str, updated_at: u64) -> Self {
        Self {
            key_hex: hex_encode(key.as_bytes()),
            value_hex: hex_encode(value.as_bytes()),
            updated_at,
            terms: record_terms(key, value),
        }
    }
}

impl SqliteRecallPlan {
    fn from_query(query: &str, limit: usize) -> Self {
        Self {
            include_all: query.trim().is_empty(),
            terms: tokenize_terms(query),
            limit,
        }
    }
}

impl SqliteMemoryAdapter {
    pub fn new(path: impl Into<std::path::PathBuf>) -> AdapterResult<Self> {
        let db_path = path.into();
        create_parent_dir(&db_path).map_err(|e| map_memory_error("memory.sqlite.new", e))?;

        let conn = Connection::open(&db_path)
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.new", e))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.new", e))?;

        ensure_schema(&conn).map_err(|e| map_memory_error("memory.sqlite.new", e))?;

        Ok(Self {
            connection: Arc::new(Mutex::new(conn)),
            health_cache: Arc::new(Mutex::new(None)),
        })
    }

    pub fn store_at(&self, key: &str, value: &str, updated_at: u64) -> AdapterResult<()> {
        let record = SqliteIndexedRecord::from_input(key, value, updated_at);
        let mut connection = lock_connection(&self.connection);
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.store_at", e))?;

        transaction
            .execute(
                "INSERT INTO memory(key_hex, value_hex, updated_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(key_hex) DO UPDATE SET \
                 value_hex = excluded.value_hex, \
                 updated_at = excluded.updated_at;",
                params![
                    record.key_hex.as_str(),
                    record.value_hex.as_str(),
                    timestamp_to_i64(record.updated_at)
                        .map_err(|e| map_memory_error("memory.sqlite.store_at", e))?
                ],
            )
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.store_at", e))?;

        transaction
            .execute(
                "DELETE FROM memory_term_index WHERE key_hex = ?1;",
                params![record.key_hex.as_str()],
            )
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.store_at", e))?;

        for term in &record.terms {
            transaction
                .execute(
                    "INSERT INTO memory_term_index(term, key_hex) VALUES (?1, ?2);",
                    params![term.as_str(), record.key_hex.as_str()],
                )
                .map_err(map_sqlite_error)
                .map_err(|e| map_memory_error("memory.sqlite.store_at", e))?;
        }

        transaction
            .commit()
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.store_at", e))?;
        Ok(())
    }

    pub fn prune_before(&self, cutoff_ms: u64) -> AdapterResult<usize> {
        let mut connection = lock_connection(&self.connection);
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.prune_before", e))?;
        let cutoff = timestamp_to_i64(cutoff_ms)
            .map_err(|e| map_memory_error("memory.sqlite.prune_before", e))?;

        transaction
            .execute(
                "DELETE FROM memory_term_index \
                 WHERE key_hex IN (SELECT key_hex FROM memory WHERE updated_at < ?1);",
                params![cutoff],
            )
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.prune_before", e))?;
        let removed = transaction
            .execute("DELETE FROM memory WHERE updated_at < ?1;", params![cutoff])
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.prune_before", e))?;

        transaction
            .commit()
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.prune_before", e))?;
        Ok(removed)
    }

    fn fetch_records_no_params(&self, sql: &str) -> MemoryResult<Vec<MemoryRecord>> {
        let connection = lock_connection(&self.connection);
        let mut statement = connection.prepare(sql).map_err(map_sqlite_error)?;
        let mut rows = statement.query([]).map_err(map_sqlite_error)?;
        let mut records = Vec::new();

        while let Some(row) = rows.next().map_err(map_sqlite_error)? {
            records.push(decode_row(row)?);
        }

        Ok(records)
    }

    fn fetch_records_with_terms(&self, plan: &SqliteRecallPlan) -> MemoryResult<Vec<MemoryRecord>> {
        let placeholders = vec!["?"; plan.terms.len()].join(", ");
        let limit_clause = if plan.limit == 0 {
            String::new()
        } else {
            format!(" LIMIT {}", plan.limit)
        };

        let sql = format!(
            "WITH matched AS (\
             SELECT key_hex FROM memory_term_index \
             WHERE term IN ({placeholders}) \
             GROUP BY key_hex \
             HAVING COUNT(DISTINCT term) = {}\
             ) \
             SELECT memory.key_hex, memory.value_hex, memory.updated_at FROM memory \
             INNER JOIN matched ON matched.key_hex = memory.key_hex \
             ORDER BY memory.updated_at DESC, memory.key_hex ASC{limit_clause};",
            plan.terms.len(),
        );

        let params: Vec<&dyn ToSql> = plan.terms.iter().map(|term| term as &dyn ToSql).collect();
        let connection = lock_connection(&self.connection);
        let mut statement = connection.prepare(&sql).map_err(map_sqlite_error)?;
        let mut rows = statement
            .query(params_from_iter(params))
            .map_err(map_sqlite_error)?;
        let mut records = Vec::new();

        while let Some(row) = rows.next().map_err(map_sqlite_error)? {
            records.push(decode_row(row)?);
        }

        Ok(records)
    }

    fn run_health_check(&self) -> MemoryResult<bool> {
        let connection = lock_connection(&self.connection);
        let mut statement = connection
            .prepare("PRAGMA quick_check;")
            .map_err(map_sqlite_error)?;
        let mut rows = statement.query([]).map_err(map_sqlite_error)?;
        while let Some(row) = rows.next().map_err(map_sqlite_error)? {
            let line: String = row.get(0).map_err(map_sqlite_error)?;
            if line.trim() == "ok" {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl crate::contracts::MemoryAdapter for SqliteMemoryAdapter {
    fn id(&self) -> &str {
        "memory.sqlite"
    }

    fn health(&self) -> crate::contracts::AdapterHealth {
        // TTL 이내이면 캐시 반환
        {
            let cache = lock_health_cache(&self.health_cache);
            if let Some(ref cached) = *cache
                && cached.checked_at.elapsed() < HEALTH_CACHE_TTL
            {
                return cached.result;
            }
        }

        // TTL 초과: 실제 헬스 체크 수행 후 캐시 갱신
        let result = match self.run_health_check() {
            Ok(is_ok) if is_ok => crate::contracts::AdapterHealth::Healthy,
            Ok(_) => crate::contracts::AdapterHealth::Degraded,
            Err(_) => crate::contracts::AdapterHealth::Unavailable,
        };

        let mut cache = lock_health_cache(&self.health_cache);
        *cache = Some(HealthCache {
            result,
            checked_at: Instant::now(),
        });

        result
    }

    fn store(&self, key: &str, value: &str) -> AdapterResult<()> {
        self.store_at(key, value, now_millis())
    }

    fn recall(
        &self,
        query: &str,
        limit: usize,
    ) -> AdapterResult<Vec<crate::contracts::MemoryEntry>> {
        let plan = SqliteRecallPlan::from_query(query, limit);
        let records = if plan.include_all {
            self.fetch_records_no_params(&select_records_sql(limit))
        } else if plan.terms.is_empty() {
            Ok(Vec::new())
        } else {
            self.fetch_records_with_terms(&plan)
        }
        .map_err(|e| map_memory_error("memory.sqlite.recall", e))?;
        Ok(records.into_iter().map(record_to_entry).collect())
    }

    fn get(&self, key: &str) -> AdapterResult<Option<crate::contracts::MemoryEntry>> {
        let key_hex = hex_encode(key.as_bytes());
        let sql = "SELECT key_hex, value_hex, updated_at FROM memory WHERE key_hex = ?1 LIMIT 1;";
        let connection = lock_connection(&self.connection);
        let mut statement = connection
            .prepare(sql)
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.get", e))?;
        let mut rows = statement
            .query(rusqlite::params![key_hex.as_str()])
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.get", e))?;
        match rows
            .next()
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.get", e))?
        {
            Some(row) => Ok(Some(record_to_entry(
                decode_row(row).map_err(|e| map_memory_error("memory.sqlite.get", e))?,
            ))),
            None => Ok(None),
        }
    }

    fn list(&self) -> AdapterResult<Vec<crate::contracts::MemoryEntry>> {
        let records = self
            .fetch_records_no_params(&select_records_sql(0))
            .map_err(|e| map_memory_error("memory.sqlite.list", e))?;
        Ok(records.into_iter().map(record_to_entry).collect())
    }

    fn delete(&self, key: &str) -> AdapterResult<bool> {
        let key_hex = hex_encode(key.as_bytes());
        let mut connection = lock_connection(&self.connection);
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.delete", e))?;
        transaction
            .execute(
                "DELETE FROM memory_term_index WHERE key_hex = ?1;",
                rusqlite::params![key_hex.as_str()],
            )
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.delete", e))?;
        let changed = transaction
            .execute(
                "DELETE FROM memory WHERE key_hex = ?1;",
                rusqlite::params![key_hex.as_str()],
            )
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.delete", e))?;
        transaction
            .commit()
            .map_err(map_sqlite_error)
            .map_err(|e| map_memory_error("memory.sqlite.delete", e))?;
        Ok(changed > 0)
    }

    fn count(&self) -> AdapterResult<usize> {
        let connection = lock_connection(&self.connection);
        count_from_connection(&connection).map_err(|e| map_memory_error("memory.sqlite.count", e))
    }
}

fn lock_connection(connection: &Arc<Mutex<Connection>>) -> MutexGuard<'_, Connection> {
    connection
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_health_cache(
    health_cache: &Arc<Mutex<Option<HealthCache>>>,
) -> MutexGuard<'_, Option<HealthCache>> {
    health_cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn record_to_entry(record: MemoryRecord) -> crate::contracts::MemoryEntry {
    crate::contracts::MemoryEntry {
        key: record.key,
        value: record.value,
        updated_at: record.updated_at,
    }
}

fn select_records_sql(limit: usize) -> String {
    if limit == 0 {
        String::from(
            "SELECT key_hex, value_hex, updated_at FROM memory ORDER BY updated_at DESC, key_hex ASC;",
        )
    } else {
        format!(
            "SELECT key_hex, value_hex, updated_at FROM memory ORDER BY updated_at DESC, key_hex ASC LIMIT {limit};",
        )
    }
}

fn ensure_schema(connection: &Connection) -> MemoryResult<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS memory (\
             key_hex TEXT PRIMARY KEY NOT NULL,\
             value_hex TEXT NOT NULL,\
             updated_at INTEGER NOT NULL\
             );\
             CREATE TABLE IF NOT EXISTS memory_term_index (\
             term TEXT NOT NULL,\
             key_hex TEXT NOT NULL,\
             PRIMARY KEY(term, key_hex)\
             );\
             CREATE INDEX IF NOT EXISTS idx_memory_updated_key ON memory(updated_at DESC, key_hex ASC);\
             CREATE INDEX IF NOT EXISTS idx_memory_term_key ON memory_term_index(key_hex);",
        )
        .map_err(map_sqlite_error)?;
    Ok(())
}

fn decode_row(row: &rusqlite::Row<'_>) -> MemoryResult<MemoryRecord> {
    let key_hex: String = row.get(0).map_err(map_sqlite_error)?;
    let value_hex: String = row.get(1).map_err(map_sqlite_error)?;
    let updated_at_raw: i64 = row.get(2).map_err(map_sqlite_error)?;
    let updated_at = u64::try_from(updated_at_raw).map_err(|_| {
        MemoryError::InvalidData(format!(
            "invalid sqlite timestamp `{updated_at_raw}`: must be non-negative"
        ))
    })?;

    let key_bytes = hex_decode(&key_hex).ok_or_else(|| {
        MemoryError::InvalidData(format!("invalid hex key in sqlite row: `{key_hex}`"))
    })?;
    let key = String::from_utf8(key_bytes).map_err(|error| {
        MemoryError::InvalidData(format!(
            "invalid utf8 key in sqlite row `{key_hex}`: {error}"
        ))
    })?;
    let value_bytes = hex_decode(&value_hex).ok_or_else(|| {
        MemoryError::InvalidData(format!("invalid hex value in sqlite row: `{value_hex}`"))
    })?;
    let value = String::from_utf8(value_bytes).map_err(|error| {
        MemoryError::InvalidData(format!(
            "invalid utf8 value in sqlite row `{value_hex}`: {error}"
        ))
    })?;

    Ok(MemoryRecord {
        key,
        value,
        updated_at,
    })
}

fn timestamp_to_i64(timestamp: u64) -> MemoryResult<i64> {
    i64::try_from(timestamp).map_err(|_| {
        MemoryError::InvalidData(format!("timestamp overflow for sqlite: {timestamp}"))
    })
}

fn count_from_connection(connection: &Connection) -> MemoryResult<usize> {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM memory;", [], |row| row.get(0))
        .map_err(map_sqlite_error)?;

    usize::try_from(count)
        .map_err(|_| MemoryError::InvalidData(format!("invalid sqlite count `{count}`")))
}

fn map_sqlite_error(error: rusqlite::Error) -> MemoryError {
    MemoryError::Backend(error.to_string())
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
