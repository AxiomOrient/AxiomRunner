#!/usr/bin/env bash
set -euo pipefail

suite="renewal_readiness"
report_dir="target/release-readiness"
report_file="${report_dir}/report.json"

release_gate_debug="fail"
release_gate_release="fail"
adapter_contract="fail"
cli_contract="fail"
transition_gates="fail"
benchmarks="fail"
failed=0
declare -a errors=()

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
{"suite":"${suite}","release_gate_debug":"${release_gate_debug}","release_gate_release":"${release_gate_release}","adapter_contract":"${adapter_contract}","cli_contract":"${cli_contract}","transition_gates":"${transition_gates}","benchmarks":"${benchmarks}","passed":${passed},"errors":${errors_json}}
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
  run_step release_gate_debug cargo test -q -p axiom_apps --test release_security_gate
  run_step release_gate_release cargo test -q --release -p axiom_apps --test release_security_gate
  run_step adapter_contract cargo test -q -p axiom_adapters --tests
  run_step cli_contract cargo test -q -p axiom_apps e2e_cli
  run_step transition_gates bash scripts/run_transition_gates.sh
  run_step benchmarks bash scripts/run_benchmarks.sh
fi

if [ "${failed}" -ne 0 ]; then
  exit 1
fi
