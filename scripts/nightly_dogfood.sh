#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
BIN_PATH=${AXONRUNNER_NIGHTLY_BIN:-"$REPO_ROOT/target/debug/axonrunner_apps"}
LOG_ROOT=${AXONRUNNER_NIGHTLY_LOG_ROOT:-"$REPO_ROOT/var/nightly_dogfood"}
FIXTURES=${AXONRUNNER_NIGHTLY_FIXTURES:-"intake.json,approval.json,on_risk.json,budget_exhausted.json,rust_service.json,node_api.json,nextjs_app.json,python_fastapi.json"}
SKIP_BUILD=${AXONRUNNER_NIGHTLY_SKIP_BUILD:-0}
TIMESTAMP=${AXONRUNNER_NIGHTLY_TIMESTAMP:-$(date -u +"%Y%m%dT%H%M%SZ")}

RUN_ROOT="$LOG_ROOT/$TIMESTAMP"
LOG_DIR="$RUN_ROOT/logs"
WORKSPACE_DIR="$RUN_ROOT/workspaces"
ARTIFACT_DIR="$RUN_ROOT/artifacts"
STATE_DIR="$RUN_ROOT/state"
SUMMARY_PATH="$RUN_ROOT/summary.txt"

mkdir -p "$LOG_DIR" "$WORKSPACE_DIR" "$ARTIFACT_DIR" "$STATE_DIR"

if [ "$SKIP_BUILD" != "1" ]; then
  cargo build -p axonrunner_apps --bin axonrunner_apps
fi

FAILURES=0
: > "$SUMMARY_PATH"

OLD_IFS=$IFS
IFS=','
set -- $FIXTURES
IFS=$OLD_IFS

for fixture in "$@"; do
  fixture=$(printf '%s' "$fixture" | tr -d '[:space:]')
  [ -n "$fixture" ] || continue

  fixture_path="$REPO_ROOT/crates/apps/tests/fixtures/goals/$fixture"
  fixture_name=${fixture%.json}
  workspace="$WORKSPACE_DIR/$fixture_name"
  artifacts="$ARTIFACT_DIR/$fixture_name"
  state_path="$STATE_DIR/$fixture_name.snapshot"
  run_stdout="$LOG_DIR/$fixture_name.run.stdout.log"
  run_stderr="$LOG_DIR/$fixture_name.run.stderr.log"
  replay_stdout="$LOG_DIR/$fixture_name.replay.stdout.log"
  replay_stderr="$LOG_DIR/$fixture_name.replay.stderr.log"
  doctor_json="$LOG_DIR/$fixture_name.doctor.json"
  doctor_stderr="$LOG_DIR/$fixture_name.doctor.stderr.log"

  mkdir -p "$workspace" "$artifacts"

  run_rc=0
  AXONRUNNER_RUNTIME_TOOL_WORKSPACE="$workspace" \
  AXONRUNNER_RUNTIME_ARTIFACT_WORKSPACE="$artifacts" \
  AXONRUNNER_RUNTIME_STATE_PATH="$state_path" \
  "$BIN_PATH" run "$fixture_path" >"$run_stdout" 2>"$run_stderr" || run_rc=$?

  replay_rc=0
  AXONRUNNER_RUNTIME_TOOL_WORKSPACE="$workspace" \
  AXONRUNNER_RUNTIME_ARTIFACT_WORKSPACE="$artifacts" \
  "$BIN_PATH" replay latest >"$replay_stdout" 2>"$replay_stderr" || replay_rc=$?

  doctor_rc=0
  AXONRUNNER_RUNTIME_TOOL_WORKSPACE="$workspace" \
  AXONRUNNER_RUNTIME_ARTIFACT_WORKSPACE="$artifacts" \
  AXONRUNNER_RUNTIME_STATE_PATH="$state_path" \
  "$BIN_PATH" doctor --json >"$doctor_json" 2>"$doctor_stderr" || doctor_rc=$?

  status=ok
  if [ "$run_rc" -ne 0 ] || [ "$replay_rc" -ne 0 ] || [ "$doctor_rc" -ne 0 ]; then
    status=failed
    FAILURES=$((FAILURES + 1))
  fi

  printf '%s fixture=%s run_rc=%s replay_rc=%s doctor_rc=%s\n' \
    "$status" "$fixture" "$run_rc" "$replay_rc" "$doctor_rc" >>"$SUMMARY_PATH"
done

printf 'failures=%s\n' "$FAILURES" >>"$SUMMARY_PATH"

if [ "$FAILURES" -ne 0 ]; then
  exit 1
fi
