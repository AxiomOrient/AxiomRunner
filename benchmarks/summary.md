# F-5 Benchmark Summary

- binary: `/Users/axient/repository/axiomAi/target/release/perf_suite`
- profile: `release`
- records: `100`
- iterations: `1`
- warmup: `0`
- required consecutive passes: `1`
- max passes: `1`
- passes executed: `1`
- gate result: `pass`
- thresholds p95 (ms/op): reduce `0.2`, memory `30`, gateway `10`

| pass | target | p50 (ms/op) | p95 (ms/op) | p95 threshold (ms/op) | target gate | pass gate | streak | report |
| ---: | --- | ---: | ---: | ---: | --- | --- | ---: | --- |
| 1 | core_reduce_path | 0.000597 | 0.000597 | 0.2 | pass | pass | 1 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass1.json` |
| 1 | memory_recall_path | 0.000140 | 0.000140 | 30 | pass | pass | 1 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass1.json` |
| 1 | gateway_validation_request_path | 0.025531 | 0.025531 | 10 | pass | pass | 1 | `/Users/axient/repository/axiomAi/benchmarks/raw/perf_suite_pass1.json` |
