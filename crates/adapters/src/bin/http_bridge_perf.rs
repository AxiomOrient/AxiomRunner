#![allow(dead_code)]

#[path = "../async_http_bridge.rs"]
mod async_http_bridge;

use async_http_bridge::AsyncHttpBridge;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const DEFAULT_ITERATIONS: usize = 40;
const DEFAULT_REQUESTS: usize = 200;
const DEFAULT_WARMUP: usize = 5;
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;

const USAGE: &str = "usage:
  http_bridge_perf [options]

options:
  --iterations <n>
  --requests <n>
  --warmup <n>
  --output <path>      write JSON report to file (use '-' for stdout)
  --help

defaults:
  iterations=40
  requests=200
  warmup=5";

#[derive(Debug, Clone, PartialEq, Eq)]
struct BenchConfig {
    iterations: usize,
    requests: usize,
    warmup: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BenchTargetResult {
    name: &'static str,
    operations: u64,
    warmup_iterations: usize,
    measured_iterations: usize,
    elapsed_ns: u64,
    avg_ns_per_iteration: u64,
    p50_ns_per_iteration: u64,
    p95_ns_per_iteration: u64,
    p99_ns_per_iteration: u64,
    p50_ns_per_operation: u64,
    p95_ns_per_operation: u64,
    p99_ns_per_operation: u64,
    avg_ns_per_operation: u64,
    ops_per_sec: u64,
    checksum: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BenchReport {
    suite: &'static str,
    config: BenchConfig,
    results: Vec<BenchTargetResult>,
    total_elapsed_ns: u64,
}

enum ParsedArgs {
    Help,
    Run(RunArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunArgs {
    iterations: usize,
    requests: usize,
    warmup: usize,
    output_path: Option<PathBuf>,
}

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(raw_args) {
        Ok(ParsedArgs::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(ParsedArgs::Run(args)) => args,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };

    let config = BenchConfig {
        iterations: args.iterations,
        requests: args.requests,
        warmup: args.warmup,
    };

    let server = match StubHttpServer::start() {
        Ok(server) => server,
        Err(error) => {
            eprintln!("failed to start local benchmark server: {error}");
            return ExitCode::from(1);
        }
    };

    let payload = serde_json::json!({
        "kind": "http_bridge_perf",
        "message": "benchmark"
    });

    let suite_start = Instant::now();
    let blocking = benchmark_blocking_http_post_path(&config, &server.url, &payload);
    let async_bridge = benchmark_async_bridge_http_post_path(&config, &server.url, &payload);
    let report = BenchReport {
        suite: "adapter_http_bridge_perf_v1",
        config,
        results: vec![blocking, async_bridge],
        total_elapsed_ns: u64_from_u128(suite_start.elapsed().as_nanos()),
    };

    let json = report_to_json(&report);
    match args.output_path {
        Some(path) => {
            if let Err(error) = std::fs::write(&path, json) {
                eprintln!("failed to write report '{}': {error}", path.display());
                return ExitCode::from(1);
            }
        }
        None => {
            println!("{json}");
        }
    }

    ExitCode::SUCCESS
}

fn benchmark_blocking_http_post_path(
    config: &BenchConfig,
    url: &str,
    payload: &serde_json::Value,
) -> BenchTargetResult {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());

    measure_target(
        "adapter_http_blocking_post_path",
        config,
        config.requests as u64,
        || {
            let mut checksum = 0_u64;
            for index in 0..config.requests {
                let response = client
                    .post(url)
                    .header("Connection", "close")
                    .json(payload)
                    .send()
                    .expect("blocking request should succeed");
                let status = response.status();
                assert!(status.is_success(), "unexpected blocking status={status}");
                let body = response.text().expect("blocking response text");
                checksum = checksum
                    .wrapping_add(body.len() as u64)
                    .wrapping_add(index as u64)
                    .wrapping_add(status.as_u16() as u64);
            }
            checksum
        },
    )
}

fn benchmark_async_bridge_http_post_path(
    config: &BenchConfig,
    url: &str,
    payload: &serde_json::Value,
) -> BenchTargetResult {
    let bridge = AsyncHttpBridge::with_timeouts(
        Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS),
        Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS),
    )
    .unwrap_or_default();

    measure_target(
        "adapter_http_async_bridge_post_path",
        config,
        config.requests as u64,
        || {
            let mut checksum = 0_u64;
            for index in 0..config.requests {
                let response = bridge
                    .post_json(url, &[("Connection", "close")], payload)
                    .expect("async bridge request should succeed");
                assert!(
                    response.status.is_success(),
                    "unexpected async bridge status={}",
                    response.status
                );
                checksum = checksum
                    .wrapping_add(response.body.len() as u64)
                    .wrapping_add(index as u64)
                    .wrapping_add(response.status.as_u16() as u64);
            }
            checksum
        },
    )
}

fn measure_target(
    name: &'static str,
    config: &BenchConfig,
    operations_per_iteration: u64,
    mut run_iteration: impl FnMut() -> u64,
) -> BenchTargetResult {
    let mut warmup_checksum = 0_u64;
    for _ in 0..config.warmup {
        warmup_checksum = warmup_checksum.wrapping_add(run_iteration());
    }
    std::hint::black_box(warmup_checksum);

    let start = Instant::now();
    let mut checksum = 0_u64;
    let mut iteration_durations_ns: Vec<u64> = Vec::with_capacity(config.iterations);
    for _ in 0..config.iterations {
        let iteration_start = Instant::now();
        checksum = checksum.wrapping_add(run_iteration());
        iteration_durations_ns.push(u64_from_u128(iteration_start.elapsed().as_nanos()));
    }
    let elapsed_ns = u64_from_u128(start.elapsed().as_nanos());
    std::hint::black_box(checksum);

    iteration_durations_ns.sort_unstable();
    let p50_ns_per_iteration = percentile_ns(&iteration_durations_ns, 50);
    let p95_ns_per_iteration = percentile_ns(&iteration_durations_ns, 95);
    let p99_ns_per_iteration = percentile_ns(&iteration_durations_ns, 99);
    let avg_ns_per_iteration = if config.iterations == 0 {
        0
    } else {
        elapsed_ns / config.iterations as u64
    };

    let operations = operations_per_iteration.saturating_mul(config.iterations as u64);
    let p50_ns_per_operation = if operations_per_iteration == 0 {
        0
    } else {
        p50_ns_per_iteration / operations_per_iteration
    };
    let p95_ns_per_operation = if operations_per_iteration == 0 {
        0
    } else {
        p95_ns_per_iteration / operations_per_iteration
    };
    let p99_ns_per_operation = if operations_per_iteration == 0 {
        0
    } else {
        p99_ns_per_iteration / operations_per_iteration
    };
    let avg_ns_per_operation = if operations == 0 {
        0
    } else {
        let elapsed_u128 = u128::from(elapsed_ns);
        let operations_u128 = u128::from(operations);
        u64_from_u128(elapsed_u128 / operations_u128)
    };
    let ops_per_sec = if elapsed_ns == 0 {
        0
    } else {
        let operations_u128 = u128::from(operations);
        let elapsed_u128 = u128::from(elapsed_ns);
        u64_from_u128((operations_u128 * 1_000_000_000_u128) / elapsed_u128)
    };

    BenchTargetResult {
        name,
        operations,
        warmup_iterations: config.warmup,
        measured_iterations: config.iterations,
        elapsed_ns,
        avg_ns_per_iteration,
        p50_ns_per_iteration,
        p95_ns_per_iteration,
        p99_ns_per_iteration,
        p50_ns_per_operation,
        p95_ns_per_operation,
        p99_ns_per_operation,
        avg_ns_per_operation,
        ops_per_sec,
        checksum,
    }
}

fn percentile_ns(sorted_samples_ns: &[u64], percentile: usize) -> u64 {
    if sorted_samples_ns.is_empty() {
        return 0;
    }

    let clamped = percentile.min(100);
    let max_index = sorted_samples_ns.len() - 1;
    let index = (max_index.saturating_mul(clamped).saturating_add(99)) / 100;
    sorted_samples_ns[index]
}

fn parse_args(raw_args: Vec<String>) -> Result<ParsedArgs, String> {
    let mut iterations = DEFAULT_ITERATIONS;
    let mut requests = DEFAULT_REQUESTS;
    let mut warmup = DEFAULT_WARMUP;
    let mut output_path: Option<PathBuf> = None;

    let mut index = 0;
    while index < raw_args.len() {
        let arg = &raw_args[index];
        match arg.as_str() {
            "--help" | "-h" => return Ok(ParsedArgs::Help),
            "--iterations" => {
                let value = raw_args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --iterations".to_string())?;
                iterations = parse_positive_usize("--iterations", value)?;
                index += 2;
            }
            "--requests" => {
                let value = raw_args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --requests".to_string())?;
                requests = parse_positive_usize("--requests", value)?;
                index += 2;
            }
            "--warmup" => {
                let value = raw_args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --warmup".to_string())?;
                warmup = parse_non_negative_usize("--warmup", value)?;
                index += 2;
            }
            "--output" => {
                let value = raw_args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --output".to_string())?;
                if value != "-" {
                    output_path = Some(PathBuf::from(value));
                }
                index += 2;
            }
            unknown => {
                return Err(format!("unknown argument: {unknown}\n\n{USAGE}"));
            }
        }
    }

    Ok(ParsedArgs::Run(RunArgs {
        iterations,
        requests,
        warmup,
        output_path,
    }))
}

fn parse_positive_usize(flag: &str, value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("invalid {flag} value: {value}"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be > 0"));
    }
    Ok(parsed)
}

fn parse_non_negative_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("invalid {flag} value: {value}"))
}

fn report_to_json(report: &BenchReport) -> String {
    let results = report
        .results
        .iter()
        .map(|result| {
            serde_json::json!({
                "name": result.name,
                "operations": result.operations,
                "warmup_iterations": result.warmup_iterations,
                "measured_iterations": result.measured_iterations,
                "elapsed_ns": result.elapsed_ns,
                "avg_ns_per_iteration": result.avg_ns_per_iteration,
                "p50_ns_per_iteration": result.p50_ns_per_iteration,
                "p95_ns_per_iteration": result.p95_ns_per_iteration,
                "p99_ns_per_iteration": result.p99_ns_per_iteration,
                "p50_ns_per_operation": result.p50_ns_per_operation,
                "p95_ns_per_operation": result.p95_ns_per_operation,
                "p99_ns_per_operation": result.p99_ns_per_operation,
                "avg_ns_per_operation": result.avg_ns_per_operation,
                "ops_per_sec": result.ops_per_sec,
                "checksum": result.checksum
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "suite": report.suite,
        "config": {
            "iterations": report.config.iterations,
            "requests": report.config.requests,
            "warmup": report.config.warmup
        },
        "results": results,
        "total_elapsed_ns": report.total_elapsed_ns
    })
    .to_string()
}

fn u64_from_u128(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

struct StubHttpServer {
    url: String,
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl StubHttpServer {
    fn start() -> Result<Self, String> {
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|error| format!("bind failed: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("set_nonblocking failed: {error}"))?;

        let addr = listener
            .local_addr()
            .map_err(|error| format!("local_addr failed: {error}"))?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = thread::spawn(move || run_stub_server(listener, shutdown_clone));
        Ok(Self {
            url: format!("http://{addr}/bench"),
            addr,
            shutdown,
            handle: Some(handle),
        })
    }
}

impl Drop for StubHttpServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run_stub_server(listener: TcpListener, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_nonblocking(false);
                let _ = respond_ok_json(&mut stream);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(_) => break,
        }
    }
}

fn respond_ok_json(stream: &mut TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .map_err(|error| format!("set_read_timeout failed: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .map_err(|error| format!("set_write_timeout failed: {error}"))?;

    read_http_request(stream)?;

    const BODY: &str = "{\"ok\":true}";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        BODY.len(),
        BODY
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| format!("write_all failed: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("flush failed: {error}"))?;
    Ok(())
}

fn read_http_request(stream: &mut TcpStream) -> Result<(), String> {
    const MAX_REQUEST_BYTES: usize = 256 * 1024;
    let mut bytes = Vec::with_capacity(4096);
    let mut buffer = [0_u8; 2048];
    let mut header_end: Option<usize> = None;
    let mut content_length = 0_usize;

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                bytes.extend_from_slice(&buffer[..read]);
                if bytes.len() > MAX_REQUEST_BYTES {
                    return Err("request too large".to_string());
                }

                if header_end.is_none()
                    && let Some(end) = find_subsequence(&bytes, b"\r\n\r\n")
                {
                    let end = end + 4;
                    header_end = Some(end);
                    content_length = parse_content_length(&bytes[..end]);
                }

                if let Some(end) = header_end
                    && bytes.len() >= end.saturating_add(content_length)
                {
                    break;
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                if let Some(end) = header_end
                    && bytes.len() >= end.saturating_add(content_length)
                {
                    break;
                }
                return Err("request read timed out before full body was received".to_string());
            }
            Err(error) => {
                return Err(format!("read failed: {error}"));
            }
        }
    }

    Ok(())
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_content_length(header_bytes: &[u8]) -> usize {
    let headers = String::from_utf8_lossy(header_bytes);
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:")
            && let Ok(length) = rest.trim().parse::<usize>()
        {
            return length;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{find_subsequence, parse_content_length, percentile_ns};

    #[test]
    fn percentile_ceil_indexing_includes_p99() {
        let mut samples = vec![100_u64, 40, 70, 10, 90];
        samples.sort_unstable();
        assert_eq!(percentile_ns(&samples, 50), 70);
        assert_eq!(percentile_ns(&samples, 95), 100);
        assert_eq!(percentile_ns(&samples, 99), 100);
    }

    #[test]
    fn parse_content_length_defaults_to_zero() {
        assert_eq!(parse_content_length(b"POST / HTTP/1.1\r\n\r\n"), 0);
        assert_eq!(
            parse_content_length(b"POST / HTTP/1.1\r\nContent-Length: 42\r\n\r\n"),
            42
        );
    }

    #[test]
    fn find_subsequence_finds_header_delimiter() {
        let bytes = b"GET / HTTP/1.1\r\nHost: local\r\n\r\nbody";
        assert_eq!(find_subsequence(bytes, b"\r\n\r\n"), Some(27));
    }
}
