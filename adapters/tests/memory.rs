use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axiom_adapters::MemoryAdapter;
use axiom_adapters::contracts::AdapterHealth;
use axiom_adapters::memory::{MarkdownMemoryAdapter, SqliteMemoryAdapter};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn sqlite_memory_api_full_path() {
    let db_path = temp_path("sqlite", "db");
    cleanup_sqlite_sidecars(&db_path);

    let mut adapter = SqliteMemoryAdapter::new(&db_path).expect("sqlite adapter init must succeed");

    let initial_health = adapter.health();
    assert_eq!(initial_health, AdapterHealth::Healthy);
    assert_eq!(adapter.count().expect("sqlite count must succeed"), 0);

    adapter
        .store("alpha", "first value")
        .expect("sqlite store alpha must succeed");
    adapter
        .store("beta", "second value")
        .expect("sqlite store beta must succeed");
    adapter
        .store("gamma", "docs and memory")
        .expect("sqlite store gamma must succeed");

    assert_eq!(adapter.count().expect("sqlite count must succeed"), 3);

    let beta = adapter
        .get("beta")
        .expect("sqlite get beta must succeed")
        .expect("beta must exist");
    assert_eq!(beta.value, "second value");

    let recalled = adapter
        .recall("memory", 8)
        .expect("sqlite recall must succeed");
    assert_eq!(recalled.len(), 1);
    assert_eq!(recalled[0].key, "gamma");

    let listed = adapter.list().expect("sqlite list must succeed");
    assert_eq!(listed.len(), 3);

    assert!(
        adapter
            .delete("beta")
            .expect("sqlite delete beta must succeed")
    );
    assert!(
        !adapter
            .delete("beta")
            .expect("sqlite second delete must succeed")
    );
    assert_eq!(adapter.count().expect("sqlite count must succeed"), 2);

    drop(adapter);

    let reopened = SqliteMemoryAdapter::new(&db_path).expect("sqlite reopen must succeed");
    assert_eq!(reopened.count().expect("sqlite count must succeed"), 2);
    assert!(
        reopened
            .get("alpha")
            .expect("sqlite get alpha must succeed")
            .is_some()
    );

    let final_health = reopened.health();
    assert_eq!(final_health, AdapterHealth::Healthy);

    cleanup_sqlite_sidecars(&db_path);
}

#[test]
fn markdown_memory_api_full_path() {
    let file_path = temp_path("markdown", "md");
    let temp_path = file_path.with_extension("tmp");
    cleanup_file(&file_path);
    cleanup_file(&temp_path);

    let mut adapter =
        MarkdownMemoryAdapter::new(&file_path).expect("markdown adapter init must succeed");

    let initial_health = adapter.health();
    assert_eq!(initial_health, AdapterHealth::Healthy);
    assert_eq!(adapter.count().expect("markdown count must succeed"), 0);

    adapter
        .store("alpha", "first note")
        .expect("markdown store alpha must succeed");
    adapter
        .store("beta", "second note")
        .expect("markdown store beta must succeed");
    adapter
        .store("gamma", "memory index")
        .expect("markdown store gamma must succeed");

    assert_eq!(adapter.count().expect("markdown count must succeed"), 3);

    let alpha = adapter
        .get("alpha")
        .expect("markdown get alpha must succeed")
        .expect("alpha must exist");
    assert_eq!(alpha.value, "first note");

    let recalled = adapter
        .recall("index", 4)
        .expect("markdown recall must succeed");
    assert_eq!(recalled.len(), 1);
    assert_eq!(recalled[0].key, "gamma");

    let listed = adapter.list().expect("markdown list must succeed");
    assert_eq!(listed.len(), 3);

    assert!(
        adapter
            .delete("beta")
            .expect("markdown delete beta must succeed")
    );
    assert!(
        !adapter
            .delete("beta")
            .expect("markdown second delete must succeed")
    );
    assert_eq!(adapter.count().expect("markdown count must succeed"), 2);

    drop(adapter);

    let reopened = MarkdownMemoryAdapter::new(&file_path).expect("markdown reopen must succeed");
    assert_eq!(reopened.count().expect("markdown count must succeed"), 2);
    assert!(
        reopened
            .get("alpha")
            .expect("markdown get alpha must succeed")
            .is_some()
    );

    let final_health = reopened.health();
    assert_eq!(final_health, AdapterHealth::Healthy);
    assert!(
        !temp_path.exists(),
        "temporary markdown persist file should not remain after writes"
    );

    cleanup_file(&file_path);
    cleanup_file(&temp_path);
}

#[test]
fn sqlite_memory_performance_smoke_small() {
    let db_path = temp_path("sqlite_perf", "db");
    cleanup_sqlite_sidecars(&db_path);

    let mut adapter = SqliteMemoryAdapter::new(&db_path).expect("sqlite adapter init must succeed");

    let start = Instant::now();

    for i in 0..80 {
        adapter
            .store(&format!("k{i}"), &format!("value-{i}"))
            .expect("sqlite performance store must succeed");
    }

    for i in 0..80 {
        let key = format!("k{i}");
        let value = adapter
            .get(&key)
            .expect("sqlite performance get must succeed")
            .expect("stored key must exist");
        assert_eq!(value.key, key);
    }

    let recall = adapter
        .recall("value", 20)
        .expect("sqlite performance recall must succeed");
    assert_eq!(recall.len(), 20);

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "sqlite performance regression: {:?}",
        elapsed
    );

    cleanup_sqlite_sidecars(&db_path);
}

#[test]
fn markdown_memory_performance_smoke_small() {
    let file_path = temp_path("markdown_perf", "md");
    cleanup_file(&file_path);

    let mut adapter =
        MarkdownMemoryAdapter::new(&file_path).expect("markdown adapter init must succeed");

    let start = Instant::now();

    for i in 0..120 {
        adapter
            .store(&format!("k{i}"), &format!("value-{i}"))
            .expect("markdown performance store must succeed");
    }

    for i in 0..120 {
        let key = format!("k{i}");
        let value = adapter
            .get(&key)
            .expect("markdown performance get must succeed")
            .expect("stored key must exist");
        assert_eq!(value.key, key);
    }

    let recall = adapter
        .recall("value", 30)
        .expect("markdown performance recall must succeed");
    assert_eq!(recall.len(), 30);

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(8),
        "markdown performance regression: {:?}",
        elapsed
    );

    cleanup_file(&file_path);
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
