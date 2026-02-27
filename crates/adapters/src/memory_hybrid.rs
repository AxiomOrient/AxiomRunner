use crate::contracts::{MemoryAdapter, MemoryEntry};
use crate::error::AdapterResult;
use crate::memory::{SqliteMemoryAdapter, record_terms, sort_entries, tokenize_terms};
use std::cmp::Ordering;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HybridRecallWeights {
    pub keyword: u32,
    pub lexical: u32,
    pub recency: u32,
}

impl Default for HybridRecallWeights {
    fn default() -> Self {
        Self {
            keyword: 5,
            lexical: 3,
            recency: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HybridRecallConfig {
    pub scan_limit: usize,
    pub result_limit: usize,
    pub weights: HybridRecallWeights,
}

impl Default for HybridRecallConfig {
    fn default() -> Self {
        Self {
            scan_limit: 512,
            result_limit: 20,
            weights: HybridRecallWeights::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridRecallHit {
    pub entry: MemoryEntry,
    pub keyword_score: u32,
    pub lexical_score: u32,
    pub recency_score: u32,
    pub total_score: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub max_age_ms: Option<u64>,
    pub max_records: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age_ms: Some(7 * 24 * 60 * 60 * 1000),
            max_records: 10_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionReport {
    pub removed_by_age: usize,
    pub removed_by_capacity: usize,
    pub remaining: usize,
    pub oldest_kept_updated_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridRecallBenchmark {
    pub iterations: usize,
    pub elapsed_ns: u128,
    pub avg_ns_per_iteration: u128,
    pub result_count: usize,
}

pub fn hybrid_recall(
    memory: &dyn MemoryAdapter,
    query: &str,
    config: HybridRecallConfig,
) -> AdapterResult<Vec<MemoryEntry>> {
    let ranked = rank_hybrid_recall(memory, query, config)?;
    Ok(ranked.into_iter().map(|hit| hit.entry).collect())
}

pub fn rank_hybrid_recall(
    memory: &dyn MemoryAdapter,
    query: &str,
    config: HybridRecallConfig,
) -> AdapterResult<Vec<HybridRecallHit>> {
    if config.scan_limit == 0 || config.result_limit == 0 {
        return Ok(Vec::new());
    }

    let mut entries = memory.list()?;
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    sort_entries(&mut entries);
    if entries.len() > config.scan_limit {
        entries.truncate(config.scan_limit);
    }

    let query_terms = tokenize_terms(query);
    let query_is_empty = query.trim().is_empty();

    let newest = entries.first().map(|entry| entry.updated_at).unwrap_or(0);
    let oldest = entries
        .last()
        .map(|entry| entry.updated_at)
        .unwrap_or(newest);

    let mut hits = Vec::with_capacity(entries.len());
    for entry in entries {
        let keyword_score = keyword_score(&query_terms, &entry);
        let lexical_score = lexical_score(query, &query_terms, &entry);

        if !query_is_empty && keyword_score == 0 && lexical_score == 0 {
            continue;
        }

        let recency_score = recency_score(entry.updated_at, newest, oldest);
        let total_score = u64::from(config.weights.keyword) * u64::from(keyword_score)
            + u64::from(config.weights.lexical) * u64::from(lexical_score)
            + u64::from(config.weights.recency) * u64::from(recency_score);

        hits.push(HybridRecallHit {
            entry,
            keyword_score,
            lexical_score,
            recency_score,
            total_score,
        });
    }

    hits.sort_by(compare_hits);
    if hits.len() > config.result_limit {
        hits.truncate(config.result_limit);
    }

    Ok(hits)
}

pub fn benchmark_hybrid_recall(
    memory: &dyn MemoryAdapter,
    query: &str,
    config: HybridRecallConfig,
    iterations: usize,
) -> AdapterResult<HybridRecallBenchmark> {
    if iterations == 0 {
        return Ok(HybridRecallBenchmark {
            iterations: 0,
            elapsed_ns: 0,
            avg_ns_per_iteration: 0,
            result_count: 0,
        });
    }

    let start = Instant::now();
    let mut result_count = 0;
    for _ in 0..iterations {
        let results = hybrid_recall(memory, query, config)?;
        result_count = results.len();
    }
    let elapsed_ns = start.elapsed().as_nanos();

    Ok(HybridRecallBenchmark {
        iterations,
        elapsed_ns,
        avg_ns_per_iteration: elapsed_ns / iterations as u128,
        result_count,
    })
}

pub fn run_sqlite_retention_job(
    memory: &SqliteMemoryAdapter,
    policy: RetentionPolicy,
    now_ms: u64,
) -> AdapterResult<RetentionReport> {
    let removed_by_age = match policy.max_age_ms {
        Some(max_age_ms) => {
            let cutoff = now_ms.saturating_sub(max_age_ms);
            memory.prune_before(cutoff)?
        }
        None => 0,
    };

    let mut entries = memory.list()?;
    sort_entries(&mut entries);

    let mut removed_by_capacity = 0;
    if policy.max_records > 0 && entries.len() > policy.max_records {
        for entry in entries.iter().skip(policy.max_records) {
            if memory.delete(&entry.key)? {
                removed_by_capacity += 1;
            }
        }
        entries = memory.list()?;
        sort_entries(&mut entries);
    }

    Ok(RetentionReport {
        removed_by_age,
        removed_by_capacity,
        remaining: entries.len(),
        oldest_kept_updated_at: entries.iter().map(|entry| entry.updated_at).min(),
    })
}

fn compare_hits(left: &HybridRecallHit, right: &HybridRecallHit) -> Ordering {
    right
        .total_score
        .cmp(&left.total_score)
        .then_with(|| right.keyword_score.cmp(&left.keyword_score))
        .then_with(|| right.lexical_score.cmp(&left.lexical_score))
        .then_with(|| right.recency_score.cmp(&left.recency_score))
        .then_with(|| right.entry.updated_at.cmp(&left.entry.updated_at))
        .then_with(|| left.entry.key.cmp(&right.entry.key))
}

fn keyword_score(query_terms: &[String], entry: &MemoryEntry) -> u32 {
    if query_terms.is_empty() {
        return 0;
    }

    let entry_terms = record_terms(&entry.key, &entry.value);
    let matched = query_terms
        .iter()
        .filter(|term| entry_terms.iter().any(|entry_term| entry_term == *term))
        .count();

    scale_ratio(matched, query_terms.len())
}

fn lexical_score(query: &str, query_terms: &[String], entry: &MemoryEntry) -> u32 {
    if query.trim().is_empty() {
        return 0;
    }

    let query_lower = query.to_ascii_lowercase();
    let key_lower = entry.key.to_ascii_lowercase();
    let value_lower = entry.value.to_ascii_lowercase();

    if key_lower.contains(&query_lower) || value_lower.contains(&query_lower) {
        return 1000;
    }

    if query_terms.is_empty() {
        return 0;
    }

    let entry_terms = record_terms(&entry.key, &entry.value);
    let matched = query_terms
        .iter()
        .filter(|term| {
            entry_terms.iter().any(|entry_term| {
                entry_term.starts_with(term.as_str()) || term.starts_with(entry_term.as_str())
            })
        })
        .count();

    scale_ratio(matched, query_terms.len())
}

fn recency_score(updated_at: u64, newest: u64, oldest: u64) -> u32 {
    if newest <= oldest {
        return 1000;
    }

    let numerator = updated_at.saturating_sub(oldest);
    let denominator = newest.saturating_sub(oldest);
    u32::try_from((u128::from(numerator) * 1000) / u128::from(denominator)).unwrap_or(1000)
}

fn scale_ratio(matched: usize, total: usize) -> u32 {
    if total == 0 || matched == 0 {
        return 0;
    }

    let scaled = (u128::from(matched as u64) * 1000) / u128::from(total as u64);
    u32::try_from(scaled).unwrap_or(1000)
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::contracts::MemoryEntry;

    use super::{
        HybridRecallConfig, HybridRecallWeights, compare_hits, keyword_score, lexical_score,
        recency_score, scale_ratio,
    };

    fn record(key: &str, value: &str, updated_at: u64) -> MemoryEntry {
        MemoryEntry {
            key: key.to_string(),
            value: value.to_string(),
            updated_at,
        }
    }

    #[test]
    fn scale_ratio_handles_boundaries() {
        assert_eq!(scale_ratio(0, 3), 0);
        assert_eq!(scale_ratio(3, 3), 1000);
        assert_eq!(scale_ratio(1, 2), 500);
    }

    #[test]
    fn recency_score_is_monotonic() {
        let low = recency_score(10, 100, 0);
        let mid = recency_score(50, 100, 0);
        let high = recency_score(100, 100, 0);
        assert!(low < mid);
        assert!(mid < high);
        assert_eq!(high, 1000);
    }

    #[test]
    fn keyword_and_lexical_scoring_work_as_expected() {
        let query_terms = vec!["alpha".to_string(), "policy".to_string()];
        let target = record("alpha-key", "policy-engine", 100);

        assert!(keyword_score(&query_terms, &target) > 0);
        assert!(lexical_score("policy eng", &query_terms, &target) > 0);
    }

    #[test]
    fn compare_hits_orders_by_total_then_recency() {
        let config = HybridRecallConfig {
            scan_limit: 8,
            result_limit: 4,
            weights: HybridRecallWeights {
                keyword: 5,
                lexical: 3,
                recency: 2,
            },
        };

        let a = super::HybridRecallHit {
            entry: record("a", "x", 10),
            keyword_score: 200,
            lexical_score: 200,
            recency_score: 200,
            total_score: u64::from(config.weights.keyword) * 200
                + u64::from(config.weights.lexical) * 200
                + u64::from(config.weights.recency) * 200,
        };
        let mut b = a.clone();
        b.entry = record("b", "x", 20);

        assert_eq!(compare_hits(&a, &b), Ordering::Greater);
    }
}
