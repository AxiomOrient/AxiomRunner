use std::collections::VecDeque;
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axonrunner_apps::{daemon, gateway, metrics};

use daemon::{DaemonConfig, Sleeper, WorkError, WorkExecutor, WorkItem};

fn make_request(body: &str, source_ip: &str) -> gateway::HttpBoundaryRequest {
    gateway::HttpBoundaryRequest::new(
        gateway::GATEWAY_METHOD,
        gateway::GATEWAY_PATH,
        body,
        source_ip,
    )
}

struct SequenceExecutor {
    steps: VecDeque<Result<(), WorkError>>,
}

impl SequenceExecutor {
    fn new(steps: Vec<Result<(), WorkError>>) -> Self {
        Self {
            steps: steps.into(),
        }
    }
}

impl WorkExecutor for SequenceExecutor {
    fn execute(&mut self, _item: &WorkItem, _attempt: u32) -> Result<(), WorkError> {
        self.steps.pop_front().unwrap_or(Ok(()))
    }
}

#[derive(Default)]
struct RecordingSleeper;

impl Sleeper for RecordingSleeper {
    fn sleep(&mut self, _duration: Duration) {}
}

fn temp_health_path(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    std::env::temp_dir().join(format!(
        "axonrunner-metrics-dashboard-{label}-{}-{stamp}.health",
        std::process::id()
    ))
}

#[test]
fn gateway_runtime_collects_queue_lock_and_copy_metrics() {
    let mut runtime = gateway::GatewayRuntime::new();
    let ok = runtime.handle(make_request("write:alpha=1", "127.0.0.1"));
    assert!(ok.processed());

    let rejected = runtime.handle(make_request("write:beta=2", "8.8.8.8"));
    assert!(!rejected.processed());

    let snapshot = runtime.metrics_snapshot();
    assert_eq!(snapshot.queue.current_depth, 0);
    assert_eq!(snapshot.queue.peak_depth, 1);
    assert_eq!(snapshot.lock.wait_count, 2);
    assert_eq!(snapshot.lock.wait_ns_total, 0);
    assert_eq!(
        snapshot.copy.in_bytes,
        ("write:alpha=1".len() + "write:beta=2".len()) as u64
    );
    assert!(snapshot.copy.out_bytes > 0);
}

#[test]
fn daemon_loop_collects_queue_lock_and_copy_metrics() {
    let health = temp_health_path("daemon");
    let mut daemon = daemon::DaemonLoop::new(
        DaemonConfig::new(&health, vec![Duration::from_millis(25)]),
        vec![WorkItem::new("work-a")],
    );
    let mut executor = SequenceExecutor::new(vec![Err(WorkError::retryable("retry")), Ok(())]);
    let mut sleeper = RecordingSleeper;

    let report = daemon
        .run_until(&mut executor, &mut sleeper, |_| false)
        .expect("daemon loop should complete");
    assert_eq!(report.completed, 1);
    assert_eq!(report.failed, 0);

    let snapshot = daemon.metrics_snapshot();
    assert_eq!(snapshot.queue.current_depth, 0);
    assert_eq!(snapshot.queue.peak_depth, 1);
    assert_eq!(snapshot.lock.wait_count, 1);
    assert_eq!(snapshot.lock.wait_ns_total, 25_000_000);
    assert_eq!(snapshot.copy.in_bytes, "work-a".len() as u64);
    assert!(snapshot.copy.out_bytes > 0);

    let _ = fs::remove_file(health);
}

#[test]
fn dashboard_render_is_deterministic_after_snapshot_merge() {
    let merged = metrics::merge_snapshot_iter([
        metrics::MetricsSnapshot {
            queue: metrics::QueueMetrics {
                current_depth: 2,
                peak_depth: 5,
            },
            lock: metrics::LockMetrics {
                wait_count: 3,
                wait_ns_total: 30,
            },
            copy: metrics::CopyMetrics {
                in_bytes: 40,
                out_bytes: 50,
            },
        },
        metrics::MetricsSnapshot {
            queue: metrics::QueueMetrics {
                current_depth: 1,
                peak_depth: 4,
            },
            lock: metrics::LockMetrics {
                wait_count: 7,
                wait_ns_total: 70,
            },
            copy: metrics::CopyMetrics {
                in_bytes: 80,
                out_bytes: 90,
            },
        },
    ]);

    assert_eq!(
        merged,
        metrics::MetricsSnapshot {
            queue: metrics::QueueMetrics {
                current_depth: 3,
                peak_depth: 5,
            },
            lock: metrics::LockMetrics {
                wait_count: 10,
                wait_ns_total: 100,
            },
            copy: metrics::CopyMetrics {
                in_bytes: 120,
                out_bytes: 140,
            },
        }
    );

    let rendered = metrics::render_dashboard(merged);
    assert_eq!(
        rendered,
        "metrics.dashboard.v1\nqueue.current_depth=3\nqueue.peak_depth=5\nlock.wait_count=10\nlock.wait_ns_total=100\ncopy.in_bytes=120\ncopy.out_bytes=140\n"
    );
}
