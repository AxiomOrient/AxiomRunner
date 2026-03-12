# F-5 Performance Benchmark Suite

This suite is rerunnable locally and in CI, with an enforced F-5 gate.

## Local run command

Default run:

```bash
bash scripts/run_benchmarks.sh
```

## Unified renewal gate link

Primary renewal readiness command:

```bash
bash scripts/run_renewal_readiness.sh
```

Fast local dry run through the unified gate:

```bash
BENCH_ITERATIONS=1 BENCH_WARMUP=0 BENCH_REQUIRED_CONSECUTIVE_PASSES=1 BENCH_MAX_PASSES=1 bash scripts/run_renewal_readiness.sh
```

Readiness artifacts:

- `benchmarks/summary.md`
- `target/release-readiness/report.json`

Fast validation run (lower sample count, same gate logic):

```bash
BENCH_ITERATIONS=5 BENCH_WARMUP=1 BENCH_REQUIRED_CONSECUTIVE_PASSES=1 BENCH_MAX_PASSES=1 bash scripts/run_benchmarks.sh
```

## Gate behavior (exact)

Each pass runs one `perf_suite` report:

```bash
perf_suite --iterations <BENCH_ITERATIONS> --records <BENCH_RECORDS> --warmup <BENCH_WARMUP> --output benchmarks/raw/perf_suite_passN.json
```

The script parses the report JSON with `awk` (no `jq` dependency) and evaluates `p95_ms_per_operation` for these targets:

- `core_reduce_path` (`p95_ns_per_operation / 1_000_000`) must be `<= 0.2`
- `memory_recall_path` (`p95_ns_per_operation / 1_000_000`) must be `<= 30`
- `gateway_validation_request_path` (`p95_ns_per_operation / 1_000_000`) must be `<= 10`

Final gate success requires `BENCH_REQUIRED_CONSECUTIVE_PASSES` consecutive successful passes (default `3`) within `BENCH_MAX_PASSES` total passes (default `3`).

- On success: script exits `0`.
- On threshold breach / insufficient streak: script exits non-zero and CI fails.
- The run short-circuits early if the remaining passes cannot mathematically reach the required consecutive streak.

## Environment knobs

Execution knobs:

- `BENCH_PROFILE` (`release` or `debug`, default `release`)
- `BENCH_BINARY` (optional explicit binary path; skips cargo build)
- `BENCH_CARGO_PACKAGE` (default `axonrunner_apps`)
- `BENCH_CARGO_BIN` (default `perf_suite`)
- `BENCH_RECORDS` (default `10000`)
- `BENCH_ITERATIONS` (default `40`)
- `BENCH_WARMUP` (default `5`)

Gate knobs:

- `BENCH_REDUCE_P95_MAX_MS` (default `0.2`)
- `BENCH_MEMORY_P95_MAX_MS` (default `30`)
- `BENCH_GATEWAY_P95_MAX_MS` (default `10`)
- `BENCH_REQUIRED_CONSECUTIVE_PASSES` (default `3`)
- `BENCH_MAX_PASSES` (default `3`, must be `>= BENCH_REQUIRED_CONSECUTIVE_PASSES`)

## Artifact layout

All outputs are written under `benchmarks/`:

```text
benchmarks/
  config.env
  summary.tsv
  summary.md
  raw/
    perf_suite_pass1.json
    perf_suite_pass2.json
    perf_suite_pass3.json
```

- `raw/perf_suite_passN.json`: one full benchmark report per pass.
- `summary.tsv`: one row per target per pass with per-operation p50/p95 values, threshold checks, and streak state.
- `summary.md`: human-readable view of `summary.tsv` + gate metadata.
- `config.env`: run configuration, thresholds, pass controls, and target mappings.

CI (`.github/workflows/benchmarks.yml`) invokes `bash scripts/run_benchmarks.sh` with the same F-5 gate variables and explicit record count, so local and CI gating are aligned.
