#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueueMetrics {
    pub current_depth: u64,
    pub peak_depth: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LockMetrics {
    pub wait_count: u64,
    pub wait_ns_total: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CopyMetrics {
    pub in_bytes: u64,
    pub out_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MetricsSnapshot {
    pub queue: QueueMetrics,
    pub lock: LockMetrics,
    pub copy: CopyMetrics,
}

pub fn record_queue_depth(snapshot: MetricsSnapshot, current_depth: u64) -> MetricsSnapshot {
    MetricsSnapshot {
        queue: QueueMetrics {
            current_depth,
            peak_depth: snapshot.queue.peak_depth.max(current_depth),
        },
        ..snapshot
    }
}

pub fn record_lock_wait_ns(snapshot: MetricsSnapshot, wait_ns: u64) -> MetricsSnapshot {
    MetricsSnapshot {
        lock: LockMetrics {
            wait_count: snapshot.lock.wait_count.saturating_add(1),
            wait_ns_total: snapshot.lock.wait_ns_total.saturating_add(wait_ns),
        },
        ..snapshot
    }
}

pub fn record_copy_bytes(
    snapshot: MetricsSnapshot,
    in_bytes: u64,
    out_bytes: u64,
) -> MetricsSnapshot {
    MetricsSnapshot {
        copy: CopyMetrics {
            in_bytes: snapshot.copy.in_bytes.saturating_add(in_bytes),
            out_bytes: snapshot.copy.out_bytes.saturating_add(out_bytes),
        },
        ..snapshot
    }
}

pub fn merge_snapshots(left: MetricsSnapshot, right: MetricsSnapshot) -> MetricsSnapshot {
    MetricsSnapshot {
        queue: QueueMetrics {
            current_depth: left
                .queue
                .current_depth
                .saturating_add(right.queue.current_depth),
            peak_depth: left.queue.peak_depth.max(right.queue.peak_depth),
        },
        lock: LockMetrics {
            wait_count: left.lock.wait_count.saturating_add(right.lock.wait_count),
            wait_ns_total: left
                .lock
                .wait_ns_total
                .saturating_add(right.lock.wait_ns_total),
        },
        copy: CopyMetrics {
            in_bytes: left.copy.in_bytes.saturating_add(right.copy.in_bytes),
            out_bytes: left.copy.out_bytes.saturating_add(right.copy.out_bytes),
        },
    }
}

pub fn merge_snapshot_iter(
    snapshots: impl IntoIterator<Item = MetricsSnapshot>,
) -> MetricsSnapshot {
    snapshots
        .into_iter()
        .fold(MetricsSnapshot::default(), merge_snapshots)
}

pub fn render_dashboard(snapshot: MetricsSnapshot) -> String {
    format!(
        "metrics.dashboard.v1\nqueue.current_depth={}\nqueue.peak_depth={}\nlock.wait_count={}\nlock.wait_ns_total={}\ncopy.in_bytes={}\ncopy.out_bytes={}\n",
        snapshot.queue.current_depth,
        snapshot.queue.peak_depth,
        snapshot.lock.wait_count,
        snapshot.lock.wait_ns_total,
        snapshot.copy.in_bytes,
        snapshot.copy.out_bytes
    )
}
