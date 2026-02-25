#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <path-to-axiom_apps-binary>"
  exit 2
fi

apps_bin="$1"
if [ ! -f "$apps_bin" ]; then
  echo "H2 parallel FAIL: apps binary not found: $apps_bin"
  exit 2
fi
if [ ! -x "$apps_bin" ]; then
  echo "H2 parallel FAIL: apps binary is not executable: $apps_bin"
  exit 2
fi

allowed_diff="${H2_ALLOWED_DIFF:-0}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

set +e
output="$(cd "$repo_root" && cargo run -q -p axiom_apps --bin h2_verify -- --apps-bin "$apps_bin" --allowed-diff "$allowed_diff" 2>&1)"
status=$?
set -e

printf '%s\n' "$output"
if [ "$status" -eq 0 ]; then
  echo "H2 parallel PASS: diff gate passed (allowed_diff=$allowed_diff)"
else
  echo "H2 parallel FAIL: diff gate failed (allowed_diff=$allowed_diff)"
fi

exit "$status"
