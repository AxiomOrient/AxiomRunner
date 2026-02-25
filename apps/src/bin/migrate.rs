#[path = "../hex_util.rs"]
mod hex_util;
#[path = "../migrate_args.rs"]
mod migrate_args;
#[path = "../migrate_io.rs"]
mod migrate_io;
#[path = "../migrate_memory.rs"]
mod migrate_memory;
#[path = "../migrate_report.rs"]
mod migrate_report;
#[path = "../migrate_runner.rs"]
mod migrate_runner;
#[path = "../migrate_types.rs"]
mod migrate_types;

use std::fs;
use std::process::ExitCode;

use crate::migrate_args::parse_args;
use crate::migrate_runner::run_migration;
use crate::migrate_types::MigrationReport;

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let mut parse_failed = false;
    let mut report = match parse_args(raw_args) {
        Ok(args) => run_migration(args),
        Err(message) => {
            parse_failed = true;
            MigrationReport::argument_failure(message)
        }
    };

    let mut json = report.to_json();
    if let Some(report_path) = report.source_paths.report.clone()
        && let Err(error) = fs::write(&report_path, &json)
    {
        report.fatal = true;
        report.errors.push(format!(
            "failed to write report '{}': {error}",
            report_path.display()
        ));
        json = report.to_json();
    }

    println!("{json}");

    if parse_failed {
        ExitCode::from(2)
    } else if report.fatal {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
