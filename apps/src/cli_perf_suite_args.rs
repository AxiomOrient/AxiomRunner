use std::path::PathBuf;

pub(crate) const DEFAULT_ITERATIONS: usize = 100;
pub(crate) const DEFAULT_RECORDS: usize = 100;
pub(crate) const DEFAULT_WARMUP: usize = 10;

pub(crate) const USAGE: &str = "usage:\n  perf_suite [options]\n\noptions:\n  --iterations <n>\n  --records <n>\n  --warmup <n>\n  --output <path>      write JSON report to file (use '-' for stdout)\n  --help\n\ndefaults:\n  iterations=100\n  records=100\n  warmup=10";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParsedArgs {
    Help,
    Run(CliArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CliArgs {
    pub(crate) iterations: usize,
    pub(crate) records: usize,
    pub(crate) warmup: usize,
    pub(crate) output_path: Option<PathBuf>,
}

pub(crate) fn parse_args(args: Vec<String>) -> Result<ParsedArgs, String> {
    let mut parsed = CliArgs {
        iterations: DEFAULT_ITERATIONS,
        records: DEFAULT_RECORDS,
        warmup: DEFAULT_WARMUP,
        output_path: None,
    };

    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];

        if arg == "--help" || arg == "-h" {
            return Ok(ParsedArgs::Help);
        }

        if arg == "--iterations" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| String::from("--iterations requires a value"))?;
            parsed.iterations = parse_positive_usize("--iterations", value)?;
            index += 1;
            continue;
        }

        if arg == "--records" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| String::from("--records requires a value"))?;
            parsed.records = parse_positive_usize("--records", value)?;
            index += 1;
            continue;
        }

        if arg == "--warmup" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| String::from("--warmup requires a value"))?;
            parsed.warmup = parse_non_negative_usize("--warmup", value)?;
            index += 1;
            continue;
        }

        if arg == "--output" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| String::from("--output requires a value"))?;
            parsed.output_path = parse_output_path(value)?;
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--iterations=") {
            parsed.iterations = parse_positive_usize("--iterations", value)?;
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--records=") {
            parsed.records = parse_positive_usize("--records", value)?;
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--warmup=") {
            parsed.warmup = parse_non_negative_usize("--warmup", value)?;
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--output=") {
            parsed.output_path = parse_output_path(value)?;
            index += 1;
            continue;
        }

        return Err(format!("unknown option '{arg}'\n{USAGE}"));
    }

    Ok(ParsedArgs::Run(parsed))
}

fn parse_positive_usize(option: &str, raw: &str) -> Result<usize, String> {
    let value = parse_non_negative_usize(option, raw)?;
    if value == 0 {
        return Err(format!("{option} must be greater than 0"));
    }
    Ok(value)
}

fn parse_non_negative_usize(option: &str, raw: &str) -> Result<usize, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{option} requires a value"));
    }

    trimmed
        .parse::<usize>()
        .map_err(|error| format!("invalid value for {option} ('{raw}'): {error}"))
}

fn parse_output_path(raw: &str) -> Result<Option<PathBuf>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(String::from("--output requires a non-empty path"));
    }

    if trimmed == "-" {
        return Ok(None);
    }

    Ok(Some(PathBuf::from(trimmed)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parse_args_defaults() {
        let parsed = parse_args(Vec::new()).expect("default args should parse");
        let ParsedArgs::Run(args) = parsed else {
            panic!("expected run mode");
        };

        assert_eq!(args.iterations, DEFAULT_ITERATIONS);
        assert_eq!(args.records, DEFAULT_RECORDS);
        assert_eq!(args.warmup, DEFAULT_WARMUP);
        assert_eq!(args.output_path, None);
    }

    #[test]
    fn parse_args_supports_equals_and_output_stdout() {
        let parsed = parse_args(vec![
            String::from("--iterations=12"),
            String::from("--records=30"),
            String::from("--warmup=7"),
            String::from("--output=-"),
        ])
        .expect("args should parse");

        let ParsedArgs::Run(args) = parsed else {
            panic!("expected run mode");
        };

        assert_eq!(args.iterations, 12);
        assert_eq!(args.records, 30);
        assert_eq!(args.warmup, 7);
        assert_eq!(args.output_path, None);
    }

    #[test]
    fn parse_output_path_rejects_empty() {
        let error = parse_output_path("   ").expect_err("empty path should fail");
        assert!(error.contains("--output requires a non-empty path"));
    }

    #[test]
    fn parse_positive_usize_rejects_zero() {
        let error = parse_positive_usize("--iterations", "0").expect_err("zero must fail");
        assert!(error.contains("must be greater than 0"));
    }

    #[test]
    fn parse_args_help() {
        let parsed = parse_args(vec![String::from("--help")]).expect("help should parse");
        assert!(matches!(parsed, ParsedArgs::Help));
    }

    #[test]
    fn parse_args_unknown_option_fails() {
        let error = parse_args(vec![String::from("--nope")]).expect_err("unknown should fail");
        assert!(error.contains("unknown option"));
    }

    #[test]
    fn parse_args_output_path_value() {
        let parsed = parse_args(vec![
            String::from("--output"),
            String::from("./bench.json"),
            String::from("--iterations"),
            String::from("1"),
            String::from("--records"),
            String::from("1"),
        ])
        .expect("args should parse");

        let ParsedArgs::Run(args) = parsed else {
            panic!("expected run mode");
        };

        assert_eq!(args.output_path.as_deref(), Some(Path::new("./bench.json")));
    }
}
