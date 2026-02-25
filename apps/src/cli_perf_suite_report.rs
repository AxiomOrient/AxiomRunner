use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BenchmarkConfig {
    pub(crate) iterations: usize,
    pub(crate) records: usize,
    pub(crate) warmup: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BenchmarkReport {
    pub(crate) suite: &'static str,
    pub(crate) config: BenchmarkConfig,
    pub(crate) results: Vec<BenchmarkTargetResult>,
    pub(crate) total_elapsed_ns: u128,
}

impl BenchmarkReport {
    pub(crate) fn to_json(&self) -> String {
        let mut out = String::new();
        out.push('{');

        out.push_str("\"suite\":");
        write_json_string(&mut out, self.suite);

        out.push_str(",\"config\":{");
        out.push_str("\"iterations\":");
        out.push_str(&self.config.iterations.to_string());
        out.push_str(",\"records\":");
        out.push_str(&self.config.records.to_string());
        out.push_str(",\"warmup\":");
        out.push_str(&self.config.warmup.to_string());
        out.push('}');

        out.push_str(",\"results\":[");
        for (index, result) in self.results.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            result.write_json(&mut out);
        }
        out.push(']');

        out.push_str(",\"total_elapsed_ns\":");
        out.push_str(&self.total_elapsed_ns.to_string());

        out.push('}');
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BenchmarkTargetResult {
    pub(crate) name: &'static str,
    pub(crate) operations: u64,
    pub(crate) warmup_iterations: usize,
    pub(crate) measured_iterations: usize,
    pub(crate) elapsed_ns: u128,
    pub(crate) avg_ns_per_iteration: u128,
    pub(crate) p50_ns_per_iteration: u128,
    pub(crate) p95_ns_per_iteration: u128,
    pub(crate) p50_ns_per_operation: u128,
    pub(crate) p95_ns_per_operation: u128,
    pub(crate) avg_ns_per_operation: u128,
    pub(crate) ops_per_sec: u64,
    pub(crate) checksum: u64,
}

impl BenchmarkTargetResult {
    fn write_json(&self, out: &mut String) {
        out.push('{');

        out.push_str("\"name\":");
        write_json_string(out, self.name);

        out.push_str(",\"operations\":");
        out.push_str(&self.operations.to_string());

        out.push_str(",\"warmup_iterations\":");
        out.push_str(&self.warmup_iterations.to_string());

        out.push_str(",\"measured_iterations\":");
        out.push_str(&self.measured_iterations.to_string());

        out.push_str(",\"elapsed_ns\":");
        out.push_str(&self.elapsed_ns.to_string());

        out.push_str(",\"avg_ns_per_iteration\":");
        out.push_str(&self.avg_ns_per_iteration.to_string());

        out.push_str(",\"p50_ns_per_iteration\":");
        out.push_str(&self.p50_ns_per_iteration.to_string());

        out.push_str(",\"p95_ns_per_iteration\":");
        out.push_str(&self.p95_ns_per_iteration.to_string());

        out.push_str(",\"p50_ns_per_operation\":");
        out.push_str(&self.p50_ns_per_operation.to_string());

        out.push_str(",\"p95_ns_per_operation\":");
        out.push_str(&self.p95_ns_per_operation.to_string());

        out.push_str(",\"avg_ns_per_operation\":");
        out.push_str(&self.avg_ns_per_operation.to_string());

        out.push_str(",\"ops_per_sec\":");
        out.push_str(&self.ops_per_sec.to_string());

        out.push_str(",\"checksum\":");
        out.push_str(&self.checksum.to_string());

        out.push('}');
    }
}

fn write_json_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ if character.is_control() => {
                let _ = write!(out, "\\u{:04x}", character as u32);
            }
            _ => out.push(character),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_json_contains_three_targets() {
        let report = BenchmarkReport {
            suite: "perf_suite_v1",
            config: BenchmarkConfig {
                iterations: 1,
                records: 2,
                warmup: 0,
            },
            results: vec![
                BenchmarkTargetResult {
                    name: "core_reduce_path",
                    operations: 1,
                    warmup_iterations: 0,
                    measured_iterations: 1,
                    elapsed_ns: 100,
                    avg_ns_per_iteration: 100,
                    p50_ns_per_iteration: 100,
                    p95_ns_per_iteration: 100,
                    p50_ns_per_operation: 100,
                    p95_ns_per_operation: 100,
                    avg_ns_per_operation: 100,
                    ops_per_sec: 10,
                    checksum: 1,
                },
                BenchmarkTargetResult {
                    name: "memory_recall_path",
                    operations: 1,
                    warmup_iterations: 0,
                    measured_iterations: 1,
                    elapsed_ns: 100,
                    avg_ns_per_iteration: 100,
                    p50_ns_per_iteration: 100,
                    p95_ns_per_iteration: 100,
                    p50_ns_per_operation: 100,
                    p95_ns_per_operation: 100,
                    avg_ns_per_operation: 100,
                    ops_per_sec: 10,
                    checksum: 2,
                },
                BenchmarkTargetResult {
                    name: "gateway_validation_request_path",
                    operations: 1,
                    warmup_iterations: 0,
                    measured_iterations: 1,
                    elapsed_ns: 100,
                    avg_ns_per_iteration: 100,
                    p50_ns_per_iteration: 100,
                    p95_ns_per_iteration: 100,
                    p50_ns_per_operation: 100,
                    p95_ns_per_operation: 100,
                    avg_ns_per_operation: 100,
                    ops_per_sec: 10,
                    checksum: 3,
                },
            ],
            total_elapsed_ns: 300,
        };

        let json = report.to_json();
        assert!(json.contains("\"core_reduce_path\""));
        assert!(json.contains("\"memory_recall_path\""));
        assert!(json.contains("\"gateway_validation_request_path\""));
        assert!(json.contains("\"p50_ns_per_iteration\":100"));
        assert!(json.contains("\"p95_ns_per_iteration\":100"));
        assert!(json.contains("\"p50_ns_per_operation\":100"));
        assert!(json.contains("\"p95_ns_per_operation\":100"));
    }

    #[test]
    fn write_json_string_escapes_quotes() {
        let mut out = String::new();
        write_json_string(&mut out, "a\"b");
        assert_eq!(out, "\"a\\\"b\"");
    }
}
