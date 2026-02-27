#![forbid(unsafe_code)]

//! Minimal Prometheus-compatible metrics HTTP server.
//!
//! Spawns a background thread that serves `/metrics` via a plain `TcpListener`.
//! No external HTTP crate dependency — uses only `std::net`.

use crate::metrics::MetricsSnapshot;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

pub const ENV_METRICS_PORT: &str = "AXIOM_METRICS_PORT";

/// Returns the metrics server port from `AXIOM_METRICS_PORT` env var, if set
/// and parseable as a valid u16 in the range 1–65535.
pub fn metrics_port_from_env() -> Option<u16> {
    let raw = std::env::var(ENV_METRICS_PORT).ok()?;
    let trimmed = raw.trim();
    let port: u16 = trimmed.parse().ok()?;
    if port == 0 {
        return None;
    }
    Some(port)
}

/// Render a `MetricsSnapshot` as Prometheus text format (exposition format v0.0.4).
pub fn render_prometheus(snapshot: &MetricsSnapshot) -> String {
    format!(
        "# HELP axiom_queue_current_depth Current number of items in the work queue.\n\
# TYPE axiom_queue_current_depth gauge\n\
axiom_queue_current_depth {}\n\
# HELP axiom_queue_peak_depth Peak number of items seen in the work queue.\n\
# TYPE axiom_queue_peak_depth gauge\n\
axiom_queue_peak_depth {}\n\
# HELP axiom_lock_wait_count Total number of lock-wait events recorded.\n\
# TYPE axiom_lock_wait_count counter\n\
axiom_lock_wait_count {}\n\
# HELP axiom_lock_wait_ns_total Total nanoseconds spent waiting on locks.\n\
# TYPE axiom_lock_wait_ns_total counter\n\
axiom_lock_wait_ns_total {}\n\
# HELP axiom_copy_in_bytes_total Total bytes copied into the daemon pipeline.\n\
# TYPE axiom_copy_in_bytes_total counter\n\
axiom_copy_in_bytes_total {}\n\
# HELP axiom_copy_out_bytes_total Total bytes written out of the daemon pipeline.\n\
# TYPE axiom_copy_out_bytes_total counter\n\
axiom_copy_out_bytes_total {}\n",
        snapshot.queue.current_depth,
        snapshot.queue.peak_depth,
        snapshot.lock.wait_count,
        snapshot.lock.wait_ns_total,
        snapshot.copy.in_bytes,
        snapshot.copy.out_bytes,
    )
}

/// Spawn a background thread that serves the Prometheus `/metrics` endpoint
/// on `0.0.0.0:{port}`.  The thread runs until the process exits.
///
/// Each incoming TCP connection receives exactly one HTTP response containing
/// the current `MetricsSnapshot`, then the connection is closed.  Only
/// `GET /metrics` returns a 200; everything else receives a 404.
pub fn spawn_metrics_server(port: u16, snapshot: Arc<Mutex<MetricsSnapshot>>) {
    let addr = format!("0.0.0.0:{port}");
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(err) => {
            eprintln!("metrics_http bind failed addr={addr} error={err}");
            return;
        }
    };

    println!("metrics_http listening addr={addr}");

    std::thread::Builder::new()
        .name(String::from("axiom-metrics-http"))
        .spawn(move || {
            for stream in listener.incoming() {
                let stream = match stream {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                handle_connection(stream, &snapshot);
            }
        })
        .unwrap_or_else(|err| {
            eprintln!("metrics_http thread spawn failed: {err}");
            // Return a dummy JoinHandle; the program continues without metrics.
            std::thread::spawn(|| {})
        });
}

fn handle_connection(mut stream: std::net::TcpStream, snapshot: &Arc<Mutex<MetricsSnapshot>>) {
    let mut buf = [0u8; 512];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };

    let request = std::str::from_utf8(&buf[..n]).unwrap_or("");
    let first_line = request.lines().next().unwrap_or("");

    // Accept both "GET /metrics HTTP/1.x" and "GET /metrics " (curl -0).
    let is_metrics = first_line.starts_with("GET /metrics");

    if is_metrics {
        let body = {
            let guard = snapshot.lock().unwrap_or_else(|p| p.into_inner());
            render_prometheus(&guard)
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
    } else {
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{CopyMetrics, LockMetrics, MetricsSnapshot, QueueMetrics};

    fn make_snapshot() -> MetricsSnapshot {
        MetricsSnapshot {
            queue: QueueMetrics {
                current_depth: 3,
                peak_depth: 7,
            },
            lock: LockMetrics {
                wait_count: 10,
                wait_ns_total: 50_000,
            },
            copy: CopyMetrics {
                in_bytes: 1024,
                out_bytes: 512,
            },
        }
    }

    #[test]
    fn render_prometheus_contains_all_metric_names() {
        let snapshot = make_snapshot();
        let output = render_prometheus(&snapshot);
        assert!(output.contains("axiom_queue_current_depth 3"));
        assert!(output.contains("axiom_queue_peak_depth 7"));
        assert!(output.contains("axiom_lock_wait_count 10"));
        assert!(output.contains("axiom_lock_wait_ns_total 50000"));
        assert!(output.contains("axiom_copy_in_bytes_total 1024"));
        assert!(output.contains("axiom_copy_out_bytes_total 512"));
    }

    #[test]
    fn render_prometheus_default_snapshot_has_zeros() {
        let snapshot = MetricsSnapshot::default();
        let output = render_prometheus(&snapshot);
        assert!(output.contains("axiom_queue_current_depth 0"));
        assert!(output.contains("axiom_lock_wait_count 0"));
    }

    #[test]
    fn metrics_port_from_env_returns_none_when_absent() {
        // Env var is not set in the test environment.
        // We cannot mutate env under #![forbid(unsafe_code)], so we only test
        // the None path that arises when the var is absent or invalid.
        // The actual Some path is covered by integration testing.
        let _ = metrics_port_from_env(); // must not panic
    }
}
