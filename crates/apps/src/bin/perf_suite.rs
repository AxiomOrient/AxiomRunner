#![allow(dead_code)]

#[path = "../agent_loop.rs"]
mod agent_loop;
#[path = "../channel_serve.rs"]
mod channel_serve;
#[path = "../env_util.rs"]
mod env_util;
#[path = "../estop.rs"]
mod estop;
#[path = "../parse_util.rs"]
mod parse_util;
#[path = "../cli_perf_suite_args.rs"]
mod perf_suite_args;
#[path = "../cli_perf_suite_report.rs"]
mod perf_suite_report;
#[path = "../cli_perf_suite_targets.rs"]
mod perf_suite_targets;

use crate::perf_suite_args::{ParsedArgs, USAGE, parse_args};
use crate::perf_suite_report::{BenchmarkConfig, BenchmarkReport};
use crate::perf_suite_targets::{
    benchmark_channel_serve_path, benchmark_core_reduce_path,
    benchmark_gateway_validation_request_path, benchmark_memory_recall_path,
};
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(raw_args) {
        Ok(ParsedArgs::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(ParsedArgs::Run(args)) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    let config = BenchmarkConfig {
        iterations: args.iterations,
        records: args.records,
        warmup: args.warmup,
    };

    let suite_start = Instant::now();
    let results = vec![
        benchmark_core_reduce_path(&config),
        benchmark_memory_recall_path(&config),
        benchmark_gateway_validation_request_path(&config),
        benchmark_channel_serve_path(&config),
    ];

    let report = BenchmarkReport {
        suite: "perf_suite_v1",
        config,
        results,
        total_elapsed_ns: suite_start.elapsed().as_nanos(),
    };

    let json = report.to_json();
    match args.output_path.as_deref() {
        Some(path) => {
            if let Err(error) = std::fs::write(path, json) {
                eprintln!("failed to write report to '{}': {error}", path.display());
                return ExitCode::from(1);
            }
        }
        None => {
            println!("{json}");
        }
    }

    ExitCode::SUCCESS
}
