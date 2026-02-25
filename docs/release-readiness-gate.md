# Unified Renewal Readiness Gate

Run full renewal readiness:

```bash
bash scripts/run_renewal_readiness.sh
```

Related transition operations guide:

- [`docs/e4-cli-transition-ops-guide.md`](e4-cli-transition-ops-guide.md)

## Final evidence (required)

Release readiness is accepted only if every artifact below exists and matches the expected values.

### 1) Canonical status report

`target/release-readiness/report.json` must contain:

- `suite`: `renewal_readiness`
- `release_gate_debug`: `pass`
- `release_gate_release`: `pass`
- `adapter_contract`: `pass`
- `cli_contract`: `pass`
- `transition_gates`: `pass`
- `benchmarks`: `pass`
- `passed`: `true`
- `errors`: empty array

```bash
jq -e '
  .suite == "renewal_readiness" and
  .release_gate_debug == "pass" and
  .release_gate_release == "pass" and
  .adapter_contract == "pass" and
  .cli_contract == "pass" and
  .transition_gates == "pass" and
  .benchmarks == "pass" and
  .passed == true and
  (.errors | length == 0)
' target/release-readiness/report.json
```

### 2) Required execution logs

Required files:

- `target/release-readiness/release_gate_debug.log`
- `target/release-readiness/release_gate_release.log`
- `target/release-readiness/adapter_contract.log`
- `target/release-readiness/cli_contract.log`
- `target/release-readiness/transition_gates.log`
- `target/release-readiness/benchmarks.log`

Required signals:

- gate/contract logs include `test result: ok.`
- benchmark log includes `=> pass`
- transition log includes `step: complete`

```bash
for f in target/release-readiness/{release_gate_debug,release_gate_release,adapter_contract,cli_contract,transition_gates,benchmarks}.log; do
  test -s "$f"
done

grep -q 'test result: ok\.' target/release-readiness/release_gate_debug.log
grep -q 'test result: ok\.' target/release-readiness/release_gate_release.log
grep -q 'test result: ok\.' target/release-readiness/adapter_contract.log
grep -q 'test result: ok\.' target/release-readiness/cli_contract.log
grep -q '=> pass' target/release-readiness/benchmarks.log
grep -q 'step: complete' target/release-readiness/transition_gates.log
```

### 3) Benchmark summary artifact

`benchmarks/summary.md` must show:

- `gate result: pass`
- thresholds line for p95: reduce `0.2`, memory `30`, gateway `10`
- rows for `core_reduce_path`, `memory_recall_path`, and `gateway_validation_request_path`

```bash
grep -q 'gate result: `pass`' benchmarks/summary.md
grep -q 'thresholds p95 (ms/op): reduce `0.2`, memory `30`, gateway `10`' benchmarks/summary.md
grep -q 'core_reduce_path' benchmarks/summary.md
grep -q 'memory_recall_path' benchmarks/summary.md
grep -q 'gateway_validation_request_path' benchmarks/summary.md
```

### 4) H4 transition sample report

`target/transition-gates/h4_sample_report.json` must satisfy:

- `suite`: `h4_transition_rehearsal_v1`
- `h2_gate`: `pass`
- `rollback_recovered`: `true`
- `rollback_elapsed_ms <= rollback_slo_ms`
- `data_loss_files`: `0`
- `passed`: `true`
- `errors`: empty array

```bash
jq -e '
  .suite == "h4_transition_rehearsal_v1" and
  .h2_gate == "pass" and
  .rollback_recovered == true and
  (.rollback_elapsed_ms <= .rollback_slo_ms) and
  .data_loss_files == 0 and
  .passed == true and
  (.errors | length == 0)
' target/transition-gates/h4_sample_report.json
```

## Fast local dry run

```bash
BENCH_ITERATIONS=1 \
BENCH_WARMUP=0 \
BENCH_REQUIRED_CONSECUTIVE_PASSES=1 \
BENCH_MAX_PASSES=1 \
bash scripts/run_renewal_readiness.sh
```
