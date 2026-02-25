#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
BENCH_DIR="${ROOT_DIR}/benchmarks"
RAW_DIR="${BENCH_DIR}/raw"
SUMMARY_TSV="${BENCH_DIR}/summary.tsv"
SUMMARY_MD="${BENCH_DIR}/summary.md"
CONFIG_ENV="${BENCH_DIR}/config.env"

BENCH_PROFILE="${BENCH_PROFILE:-release}"
BENCH_ITERATIONS="${BENCH_ITERATIONS:-40}"
BENCH_RECORDS="${BENCH_RECORDS:-10000}"
BENCH_WARMUP="${BENCH_WARMUP:-5}"
BENCH_CARGO_PACKAGE="${BENCH_CARGO_PACKAGE:-axiom_apps}"
BENCH_CARGO_BIN="${BENCH_CARGO_BIN:-perf_suite}"
BENCH_REDUCE_P95_MAX_MS="${BENCH_REDUCE_P95_MAX_MS:-0.2}"
BENCH_MEMORY_P95_MAX_MS="${BENCH_MEMORY_P95_MAX_MS:-30}"
BENCH_GATEWAY_P95_MAX_MS="${BENCH_GATEWAY_P95_MAX_MS:-10}"
BENCH_REQUIRED_CONSECUTIVE_PASSES="${BENCH_REQUIRED_CONSECUTIVE_PASSES:-3}"
BENCH_MAX_PASSES="${BENCH_MAX_PASSES:-3}"

is_positive_int() {
  case "$1" in
    ''|*[!0-9]*|0) return 1 ;;
    *) return 0 ;;
  esac
}

is_non_negative_int() {
  case "$1" in
    ''|*[!0-9]*) return 1 ;;
    *) return 0 ;;
  esac
}

is_non_negative_number() {
  awk -v n="$1" 'BEGIN { exit (n ~ /^[0-9]+([.][0-9]+)?$/ && n >= 0) ? 0 : 1 }'
}

is_lte() {
  left="$1"
  right="$2"
  awk -v l="$left" -v r="$right" 'BEGIN { exit (l <= r) ? 0 : 1 }'
}

if ! is_positive_int "$BENCH_ITERATIONS"; then
  echo "BENCH_ITERATIONS must be a positive integer" >&2
  exit 1
fi

if ! is_positive_int "$BENCH_RECORDS"; then
  echo "BENCH_RECORDS must be a positive integer" >&2
  exit 1
fi

if ! is_non_negative_int "$BENCH_WARMUP"; then
  echo "BENCH_WARMUP must be a non-negative integer" >&2
  exit 1
fi

if ! is_non_negative_number "$BENCH_REDUCE_P95_MAX_MS"; then
  echo "BENCH_REDUCE_P95_MAX_MS must be a non-negative number" >&2
  exit 1
fi

if ! is_non_negative_number "$BENCH_MEMORY_P95_MAX_MS"; then
  echo "BENCH_MEMORY_P95_MAX_MS must be a non-negative number" >&2
  exit 1
fi

if ! is_non_negative_number "$BENCH_GATEWAY_P95_MAX_MS"; then
  echo "BENCH_GATEWAY_P95_MAX_MS must be a non-negative number" >&2
  exit 1
fi

if ! is_positive_int "$BENCH_REQUIRED_CONSECUTIVE_PASSES"; then
  echo "BENCH_REQUIRED_CONSECUTIVE_PASSES must be a positive integer" >&2
  exit 1
fi

if ! is_positive_int "$BENCH_MAX_PASSES"; then
  echo "BENCH_MAX_PASSES must be a positive integer" >&2
  exit 1
fi

if [ "$BENCH_MAX_PASSES" -lt "$BENCH_REQUIRED_CONSECUTIVE_PASSES" ]; then
  echo "BENCH_MAX_PASSES must be >= BENCH_REQUIRED_CONSECUTIVE_PASSES" >&2
  exit 1
fi

if [ -n "${BENCH_BINARY:-}" ]; then
  BENCH_BINARY_PATH="$BENCH_BINARY"
else
  case "$BENCH_PROFILE" in
    release)
      cargo build --release -p "$BENCH_CARGO_PACKAGE" --bin "$BENCH_CARGO_BIN" >/dev/null
      BENCH_BINARY_PATH="${ROOT_DIR}/target/release/${BENCH_CARGO_BIN}"
      ;;
    debug)
      cargo build -p "$BENCH_CARGO_PACKAGE" --bin "$BENCH_CARGO_BIN" >/dev/null
      BENCH_BINARY_PATH="${ROOT_DIR}/target/debug/${BENCH_CARGO_BIN}"
      ;;
    *)
      echo "BENCH_PROFILE must be 'release' or 'debug'" >&2
      exit 1
      ;;
  esac
fi

if [ ! -x "$BENCH_BINARY_PATH" ]; then
  echo "benchmark binary not executable: $BENCH_BINARY_PATH" >&2
  exit 1
fi

mkdir -p "$RAW_DIR"
rm -f "$RAW_DIR"/perf_suite_pass*.json "$SUMMARY_TSV" "$SUMMARY_MD" "$CONFIG_ENV"

run_perf_suite_pass() {
  report_file="$1"
  "$BENCH_BINARY_PATH" \
    --iterations "$BENCH_ITERATIONS" \
    --records "$BENCH_RECORDS" \
    --warmup "$BENCH_WARMUP" \
    --output "$report_file"
}

extract_target_number() {
  report_file="$1"
  target_name="$2"
  field_name="$3"

  awk -v target="$target_name" -v field="$field_name" '
    BEGIN { found = 0 }
    {
      pattern = "\"name\":\"" target "\"[^}]*\"" field "\":[0-9][0-9]*"
      if (match($0, pattern)) {
        value = substr($0, RSTART, RLENGTH)
        sub(".*\"" field "\":", "", value)
        gsub("[^0-9].*", "", value)
        print value
        found = 1
        exit
      }
    }
    END {
      if (!found) {
        exit 1
      }
    }
  ' "$report_file"
}

ns_to_ms() {
  ns_value="$1"
  awk -v ns="$ns_value" 'BEGIN { printf "%.6f", ns / 1000000.0 }'
}

evaluate_target() {
  report_file="$1"
  target_name="$2"
  threshold_ms="$3"

  target_p50_ns="$(extract_target_number "$report_file" "$target_name" "p50_ns_per_operation")" || {
    echo "failed to parse p50_ns_per_operation for target '$target_name' from $report_file" >&2
    exit 1
  }
  target_p95_ns="$(extract_target_number "$report_file" "$target_name" "p95_ns_per_operation")" || {
    echo "failed to parse p95_ns_per_operation for target '$target_name' from $report_file" >&2
    exit 1
  }

  target_p50_ms="$(ns_to_ms "$target_p50_ns")"
  target_p95_ms="$(ns_to_ms "$target_p95_ns")"
  target_status="fail"
  if is_lte "$target_p95_ms" "$threshold_ms"; then
    target_status="pass"
  fi
}

append_summary_row() {
  pass_number="$1"
  target="$2"
  p50_ms="$3"
  p95_ms="$4"
  threshold="$5"
  target_gate="$6"
  pass_gate="$7"
  streak="$8"
  report_file="$9"

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$pass_number" "$target" "$BENCH_RECORDS" "$BENCH_ITERATIONS" "$BENCH_WARMUP" "$p50_ms" "$p95_ms" \
    "$threshold" "$target_gate" "$pass_gate" "$streak" "$report_file" >> "$SUMMARY_TSV"
}

printf 'pass\ttarget\trecords\titerations\twarmup\tp50_ms_per_operation\tp95_ms_per_operation\tthreshold_p95_ms_per_operation\ttarget_gate\tpass_gate\tconsecutive_passes\treport_file\n' > "$SUMMARY_TSV"

consecutive_passes=0
pass_index=1
passes_run=0

while [ "$pass_index" -le "$BENCH_MAX_PASSES" ]; do
  passes_run="$pass_index"
  report_file="${RAW_DIR}/perf_suite_pass${pass_index}.json"
  run_perf_suite_pass "$report_file"

  evaluate_target "$report_file" "core_reduce_path" "$BENCH_REDUCE_P95_MAX_MS"
  reduce_p50="$target_p50_ms"
  reduce_p95="$target_p95_ms"
  reduce_gate="$target_status"

  evaluate_target "$report_file" "memory_recall_path" "$BENCH_MEMORY_P95_MAX_MS"
  memory_p50="$target_p50_ms"
  memory_p95="$target_p95_ms"
  memory_gate="$target_status"

  evaluate_target "$report_file" "gateway_validation_request_path" "$BENCH_GATEWAY_P95_MAX_MS"
  gateway_p50="$target_p50_ms"
  gateway_p95="$target_p95_ms"
  gateway_gate="$target_status"

  pass_gate="fail"
  if [ "$reduce_gate" = "pass" ] && [ "$memory_gate" = "pass" ] && [ "$gateway_gate" = "pass" ]; then
    pass_gate="pass"
    consecutive_passes=$((consecutive_passes + 1))
  else
    consecutive_passes=0
  fi

  append_summary_row "$pass_index" "core_reduce_path" "$reduce_p50" "$reduce_p95" \
    "$BENCH_REDUCE_P95_MAX_MS" "$reduce_gate" "$pass_gate" "$consecutive_passes" "$report_file"
  append_summary_row "$pass_index" "memory_recall_path" "$memory_p50" "$memory_p95" \
    "$BENCH_MEMORY_P95_MAX_MS" "$memory_gate" "$pass_gate" "$consecutive_passes" "$report_file"
  append_summary_row "$pass_index" "gateway_validation_request_path" "$gateway_p50" "$gateway_p95" \
    "$BENCH_GATEWAY_P95_MAX_MS" "$gateway_gate" "$pass_gate" "$consecutive_passes" "$report_file"

  printf 'pass %s: reduce p95/op=%sms<=%sms (%s), memory p95/op=%sms<=%sms (%s), gateway p95/op=%sms<=%sms (%s) => %s (streak %s/%s)\n' \
    "$pass_index" \
    "$reduce_p95" "$BENCH_REDUCE_P95_MAX_MS" "$reduce_gate" \
    "$memory_p95" "$BENCH_MEMORY_P95_MAX_MS" "$memory_gate" \
    "$gateway_p95" "$BENCH_GATEWAY_P95_MAX_MS" "$gateway_gate" \
    "$pass_gate" "$consecutive_passes" "$BENCH_REQUIRED_CONSECUTIVE_PASSES"

  if [ "$consecutive_passes" -ge "$BENCH_REQUIRED_CONSECUTIVE_PASSES" ]; then
    break
  fi

  remaining_passes=$((BENCH_MAX_PASSES - pass_index))
  max_possible_streak=$((consecutive_passes + remaining_passes))
  if [ "$max_possible_streak" -lt "$BENCH_REQUIRED_CONSECUTIVE_PASSES" ]; then
    break
  fi

  pass_index=$((pass_index + 1))
done

gate_status="fail"
if [ "$consecutive_passes" -ge "$BENCH_REQUIRED_CONSECUTIVE_PASSES" ]; then
  gate_status="pass"
fi

{
  printf 'BENCH_PROFILE=%s\n' "$BENCH_PROFILE"
  printf 'BENCH_BINARY=%s\n' "$BENCH_BINARY_PATH"
  printf 'BENCH_ITERATIONS=%s\n' "$BENCH_ITERATIONS"
  printf 'BENCH_RECORDS=%s\n' "$BENCH_RECORDS"
  printf 'BENCH_WARMUP=%s\n' "$BENCH_WARMUP"
  printf 'BENCH_REDUCE_P95_MAX_MS=%s\n' "$BENCH_REDUCE_P95_MAX_MS"
  printf 'BENCH_MEMORY_P95_MAX_MS=%s\n' "$BENCH_MEMORY_P95_MAX_MS"
  printf 'BENCH_GATEWAY_P95_MAX_MS=%s\n' "$BENCH_GATEWAY_P95_MAX_MS"
  printf 'BENCH_REQUIRED_CONSECUTIVE_PASSES=%s\n' "$BENCH_REQUIRED_CONSECUTIVE_PASSES"
  printf 'BENCH_MAX_PASSES=%s\n' "$BENCH_MAX_PASSES"
  printf 'BENCH_PASSES_RUN=%s\n' "$passes_run"
  printf 'BENCH_GATE_STATUS=%s\n' "$gate_status"
  printf 'TARGET_reduce=core_reduce_path\n'
  printf 'TARGET_memory=memory_recall_path\n'
  printf 'TARGET_gateway=gateway_validation_request_path\n'
} > "$CONFIG_ENV"

{
  printf '# F-5 Benchmark Summary\n\n'
  printf -- '- binary: `%s`\n' "$BENCH_BINARY_PATH"
  printf -- '- profile: `%s`\n' "$BENCH_PROFILE"
  printf -- '- records: `%s`\n' "$BENCH_RECORDS"
  printf -- '- iterations: `%s`\n' "$BENCH_ITERATIONS"
  printf -- '- warmup: `%s`\n' "$BENCH_WARMUP"
  printf -- '- required consecutive passes: `%s`\n' "$BENCH_REQUIRED_CONSECUTIVE_PASSES"
  printf -- '- max passes: `%s`\n' "$BENCH_MAX_PASSES"
  printf -- '- passes executed: `%s`\n' "$passes_run"
  printf -- '- gate result: `%s`\n' "$gate_status"
  printf -- '- thresholds p95 (ms/op): reduce `%s`, memory `%s`, gateway `%s`\n\n' \
    "$BENCH_REDUCE_P95_MAX_MS" "$BENCH_MEMORY_P95_MAX_MS" "$BENCH_GATEWAY_P95_MAX_MS"
  printf '| pass | target | p50 (ms/op) | p95 (ms/op) | p95 threshold (ms/op) | target gate | pass gate | streak | report |\n'
  printf '| ---: | --- | ---: | ---: | ---: | --- | --- | ---: | --- |\n'
  awk 'NR > 1 { printf "| %s | %s | %s | %s | %s | %s | %s | %s | `%s` |\n", $1, $2, $6, $7, $8, $9, $10, $11, $12 }' "$SUMMARY_TSV"
} > "$SUMMARY_MD"

cat "$SUMMARY_TSV"

if [ "$gate_status" != "pass" ]; then
  echo "F-5 gate failed: required ${BENCH_REQUIRED_CONSECUTIVE_PASSES} consecutive pass(es), got ${consecutive_passes} within ${passes_run} executed pass(es)." >&2
  exit 1
fi
