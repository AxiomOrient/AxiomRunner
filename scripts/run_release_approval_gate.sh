#!/usr/bin/env bash
set -euo pipefail

suite="release_approval_gate_v1"
report_dir="target/release-approval-gate"
report_file="${report_dir}/report.json"

security_gate_debug="fail"
security_gate_release="fail"
perf_gate="fail"
failed=0
declare -a errors=()

RELEASE_GATE_BENCH_PROFILE="${RELEASE_GATE_BENCH_PROFILE:-release}"
RELEASE_GATE_BENCH_ITERATIONS="${RELEASE_GATE_BENCH_ITERATIONS:-20}"
RELEASE_GATE_BENCH_RECORDS="${RELEASE_GATE_BENCH_RECORDS:-2000}"
RELEASE_GATE_BENCH_WARMUP="${RELEASE_GATE_BENCH_WARMUP:-2}"
RELEASE_GATE_BENCH_REQUIRED_CONSECUTIVE_PASSES="${RELEASE_GATE_BENCH_REQUIRED_CONSECUTIVE_PASSES:-1}"
RELEASE_GATE_BENCH_MAX_PASSES="${RELEASE_GATE_BENCH_MAX_PASSES:-1}"
RELEASE_GATE_BENCH_REDUCE_P95_MAX_MS="${RELEASE_GATE_BENCH_REDUCE_P95_MAX_MS:-0.2}"
RELEASE_GATE_BENCH_MEMORY_P95_MAX_MS="${RELEASE_GATE_BENCH_MEMORY_P95_MAX_MS:-30}"
RELEASE_GATE_BENCH_GATEWAY_P95_MAX_MS="${RELEASE_GATE_BENCH_GATEWAY_P95_MAX_MS:-10}"

append_error() {
  errors+=("$1")
  failed=1
}

run_step() {
  local field="$1"
  shift

  local log_file="${report_dir}/${field}.log"
  mkdir -p "${report_dir}"

  if "$@" >"${log_file}" 2>&1; then
    printf -v "${field}" '%s' "pass"
  else
    local rc=$?
    printf -v "${field}" '%s' "fail"
    append_error "${field} failed with exit code ${rc}; log=${log_file}"
  fi
}

write_report() {
  set +e

  mkdir -p "${report_dir}" 2>/dev/null || true

  local errors_json
  if [ "${#errors[@]}" -eq 0 ]; then
    errors_json='[]'
  else
    errors_json="$(printf '%s\n' "${errors[@]}" | perl -MJSON::PP -e 'use strict; use warnings; my @items = <STDIN>; chomp @items; print JSON::PP->new->ascii->encode(\@items);')"
  fi

  local passed="false"
  if [ "${failed}" -eq 0 ]; then
    passed="true"
  fi

  local json
  json="$(cat <<JSON
{"suite":"${suite}","security_gate_debug":"${security_gate_debug}","security_gate_release":"${security_gate_release}","perf_gate":"${perf_gate}","passed":${passed},"errors":${errors_json}}
JSON
)"

  printf '%s\n' "${json}" >"${report_file}" 2>/dev/null || true
  printf '%s\n' "${json}"
}

on_exit() {
  local rc=$?
  if [ "${rc}" -ne 0 ] && [ "${failed}" -eq 0 ]; then
    append_error "script failed with exit code ${rc}"
  fi
  write_report
}
trap on_exit EXIT

if [ ! -f Cargo.toml ]; then
  append_error "run from repository root with Cargo.toml"
else
  run_step security_gate_debug cargo test -q -p axiom_apps --test release_security_gate
  run_step security_gate_release cargo test -q --release -p axiom_apps --test release_security_gate
  run_step perf_gate env \
    BENCH_PROFILE="${RELEASE_GATE_BENCH_PROFILE}" \
    BENCH_ITERATIONS="${RELEASE_GATE_BENCH_ITERATIONS}" \
    BENCH_RECORDS="${RELEASE_GATE_BENCH_RECORDS}" \
    BENCH_WARMUP="${RELEASE_GATE_BENCH_WARMUP}" \
    BENCH_REQUIRED_CONSECUTIVE_PASSES="${RELEASE_GATE_BENCH_REQUIRED_CONSECUTIVE_PASSES}" \
    BENCH_MAX_PASSES="${RELEASE_GATE_BENCH_MAX_PASSES}" \
    BENCH_REDUCE_P95_MAX_MS="${RELEASE_GATE_BENCH_REDUCE_P95_MAX_MS}" \
    BENCH_MEMORY_P95_MAX_MS="${RELEASE_GATE_BENCH_MEMORY_P95_MAX_MS}" \
    BENCH_GATEWAY_P95_MAX_MS="${RELEASE_GATE_BENCH_GATEWAY_P95_MAX_MS}" \
    bash scripts/run_benchmarks.sh
fi

if [ "${failed}" -ne 0 ]; then
  exit 1
fi
