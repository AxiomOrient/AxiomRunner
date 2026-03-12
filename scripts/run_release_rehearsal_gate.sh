#!/usr/bin/env bash
set -euo pipefail

suite="release_rehearsal_gate_v1"
report_dir="target/release-rehearsal-gate"
report_file="${report_dir}/report.json"

transition_gates="fail"
h4_sample_contract="fail"
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
{"suite":"${suite}","transition_gates":"${transition_gates}","h4_sample_contract":"${h4_sample_contract}","passed":${passed},"errors":${errors_json}}
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
  run_step transition_gates bash scripts/run_transition_gates.sh
  run_step h4_sample_contract perl -MJSON::PP -e '
      use strict;
      use warnings;
      my ($path) = @ARGV;
      open my $fh, "<", $path or die "open failed: $!";
      local $/;
      my $doc = JSON::PP::decode_json(<$fh>);
      die "suite mismatch\n" unless ($doc->{suite} // "") eq "h4_transition_rehearsal_v1";
      die "h2 gate not pass\n" unless ($doc->{h2_gate} // "") eq "pass";
      die "rollback not recovered\n" unless ($doc->{rollback_recovered});
      die "data loss detected\n" unless (($doc->{data_loss_files} // 1) == 0);
      die "rehearsal not passed\n" unless ($doc->{passed});
      my $elapsed = $doc->{rollback_elapsed_ms};
      my $slo = $doc->{rollback_slo_ms};
      die "rollback_elapsed_ms missing\n" unless defined $elapsed && $elapsed =~ /\A[0-9]+\z/;
      die "rollback_slo_ms missing\n" unless defined $slo && $slo =~ /\A[0-9]+\z/;
      die "rollback slo violated\n" if $elapsed > $slo;
    ' target/transition-gates/h4_sample_report.json
fi

if [ "${failed}" -ne 0 ]; then
  exit 1
fi
