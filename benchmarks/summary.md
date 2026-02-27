# F-5 Benchmark Summary

- binary: `/Users/axient/repository/axiomAi/target/release/perf_suite`
- profile: `release`
- records: `10000`
- iterations: `40`
- warmup: `5`
- required consecutive passes: `3`
- max passes: `3`
- passes executed: `3`
- gate result: `pass`
- thresholds p95 (ms/op): reduce `0.2`, memory `30`, gateway `10`

| pass | target | p50 (ms/op) | p95 (ms/op) | p95 threshold (ms/op) | target gate | pass gate | streak | report |
| ---: | --- | ---: | ---: | ---: | --- | --- | ---: | --- |
| 1 | core_reduce_path | 0.000173 | 0.000198 | 0.2 | pass | pass | 1 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass1.json` |
| 1 | memory_recall_path | 0.000065 | 0.000069 | 30 | pass | pass | 1 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass1.json` |
| 1 | gateway_validation_request_path | 1.015208 | 1.198783 | 10 | pass | pass | 1 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass1.json` |
| 2 | core_reduce_path | 0.000148 | 0.000161 | 0.2 | pass | pass | 2 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass2.json` |
| 2 | memory_recall_path | 0.000064 | 0.000066 | 30 | pass | pass | 2 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass2.json` |
| 2 | gateway_validation_request_path | 1.017233 | 1.067847 | 10 | pass | pass | 2 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass2.json` |
| 3 | core_reduce_path | 0.000148 | 0.000153 | 0.2 | pass | pass | 3 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass3.json` |
| 3 | memory_recall_path | 0.000068 | 0.000094 | 30 | pass | pass | 3 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass3.json` |
| 3 | gateway_validation_request_path | 1.041335 | 1.111207 | 10 | pass | pass | 3 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass3.json` |
