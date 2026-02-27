use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axiom_adapters::MemoryAdapter;
use axiom_adapters::memory::{
    HybridRecallConfig, RetentionPolicy, SqliteMemoryAdapter, benchmark_hybrid_recall,
    hybrid_recall, run_sqlite_retention_job,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn sqlite_hybrid_recall_benchmark_smoke() {
    let db_path = temp_path("sqlite_hybrid_bench", "db");
    cleanup_sqlite_sidecars(&db_path);

    let memory = SqliteMemoryAdapter::new(&db_path).expect("sqlite adapter init must succeed");

    for i in 0..400 {
        let key = format!("memory-key-{i:04}");
        let value = if i % 11 == 0 {
            format!("policy engine hybrid note {i}")
        } else {
            format!("baseline note {i}")
        };
        memory
            .store(&key, &value)
            .expect("seed store should succeed");
    }

    let config = HybridRecallConfig {
        scan_limit: 256,
        result_limit: 16,
        ..HybridRecallConfig::default()
    };

    let benchmark = benchmark_hybrid_recall(&memory, "policy engine", config, 20)
        .expect("benchmark should succeed");

    assert!(benchmark.result_count > 0);
    assert!(
        benchmark.avg_ns_per_iteration < 30_000_000,
        "hybrid recall benchmark regression avg_ns_per_iteration={}",
        benchmark.avg_ns_per_iteration
    );

    let results =
        hybrid_recall(&memory, "policy engine", config).expect("hybrid recall should succeed");
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|entry| entry.value.contains("policy engine")),
        "results should contain keyword-matching entries"
    );

    cleanup_sqlite_sidecars(&db_path);
}

#[test]
fn sqlite_retention_job_prunes_by_age_and_capacity() {
    let db_path = temp_path("sqlite_retention", "db");
    cleanup_sqlite_sidecars(&db_path);

    let memory = SqliteMemoryAdapter::new(&db_path).expect("sqlite adapter init must succeed");

    let now_ms = 50_000;
    memory
        .store_at("old-1", "legacy", now_ms - 10_000)
        .expect("store old-1 should succeed");
    memory
        .store_at("old-2", "legacy", now_ms - 9_000)
        .expect("store old-2 should succeed");
    memory
        .store_at("keep-1", "fresh", now_ms - 100)
        .expect("store keep-1 should succeed");
    memory
        .store_at("keep-2", "fresh", now_ms - 80)
        .expect("store keep-2 should succeed");
    memory
        .store_at("keep-3", "fresh", now_ms - 60)
        .expect("store keep-3 should succeed");

    let report = run_sqlite_retention_job(
        &memory,
        RetentionPolicy {
            max_age_ms: Some(500),
            max_records: 2,
        },
        now_ms,
    )
    .expect("retention job should succeed");

    assert_eq!(report.removed_by_age, 2);
    assert_eq!(report.removed_by_capacity, 1);
    assert_eq!(report.remaining, 2);

    let remaining = memory.list().expect("list should succeed");
    let keys: Vec<&str> = remaining.iter().map(|entry| entry.key.as_str()).collect();

    assert!(keys.contains(&"keep-2"));
    assert!(keys.contains(&"keep-3"));
    assert!(!keys.contains(&"old-1"));
    assert!(!keys.contains(&"old-2"));
    assert!(!keys.contains(&"keep-1"));

    cleanup_sqlite_sidecars(&db_path);
}

fn temp_path(prefix: &str, ext: &str) -> PathBuf {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

    env::temp_dir().join(format!(
        "axiom_adapters_{prefix}_{}_{}_{}.{}",
        std::process::id(),
        seed,
        id,
        ext
    ))
}

fn cleanup_file(path: &Path) {
    let _ = fs::remove_file(path);
}

fn cleanup_sqlite_sidecars(path: &Path) {
    cleanup_file(path);

    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    cleanup_file(Path::new(&wal));

    let mut shm = path.as_os_str().to_os_string();
    shm.push("-shm");
    cleanup_file(Path::new(&shm));

    let mut journal = path.as_os_str().to_os_string();
    journal.push("-journal");
    cleanup_file(Path::new(&journal));
}
