#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
# Official nightly entrypoint identity: axiomrunner_apps + AXIOMRUNNER_*.
BIN_PATH=${AXIOMRUNNER_NIGHTLY_BIN:-"$REPO_ROOT/target/debug/axiomrunner_apps"}
LOG_ROOT=${AXIOMRUNNER_NIGHTLY_LOG_ROOT:-"$REPO_ROOT/var/nightly_dogfood"}
FIXTURES=${AXIOMRUNNER_NIGHTLY_FIXTURES:-"intake.json,approval.json,on_risk.json,budget_exhausted.json,rust_service.json,node_api.json,nextjs_app.json,python_fastapi.json"}
SKIP_BUILD=${AXIOMRUNNER_NIGHTLY_SKIP_BUILD:-0}
TIMESTAMP=${AXIOMRUNNER_NIGHTLY_TIMESTAMP:-$(date -u +"%Y%m%dT%H%M%SZ")}

RUN_ROOT="$LOG_ROOT/$TIMESTAMP"
LOG_DIR="$RUN_ROOT/logs"
WORKSPACE_DIR="$RUN_ROOT/workspaces"
ARTIFACT_DIR="$RUN_ROOT/artifacts"
STATE_DIR="$RUN_ROOT/state"
SUMMARY_PATH="$RUN_ROOT/summary.txt"

mkdir -p "$LOG_DIR" "$WORKSPACE_DIR" "$ARTIFACT_DIR" "$STATE_DIR"

if [ "$SKIP_BUILD" != "1" ]; then
  cargo build -p axiomrunner_apps --bin axiomrunner_apps
fi

scaffold_fixture_workspace() {
  fixture_name=$1
  workspace=$2
  template_root="$REPO_ROOT/crates/apps/tests/fixtures/workspaces"
  template_path=""
  case "$fixture_name" in
    rust_service) template_path="$template_root/rust_service" ;;
    node_api|nextjs_app) template_path="$template_root/node_common" ;;
    python_fastapi) template_path="$template_root/python_fastapi" ;;
    *) ;;
  esac
  if [ -n "$template_path" ] && [ -d "$template_path" ]; then
    cp -R "$template_path"/. "$workspace"/
  fi
}

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
  scaffold_fixture_workspace "$fixture_name" "$workspace"

  run_rc=0
  AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE="$workspace" \
  AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE="$artifacts" \
  AXIOMRUNNER_RUNTIME_STATE_PATH="$state_path" \
  "$BIN_PATH" run "$fixture_path" >"$run_stdout" 2>"$run_stderr" || run_rc=$?

  replay_rc=0
  AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE="$workspace" \
  AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE="$artifacts" \
  "$BIN_PATH" replay latest >"$replay_stdout" 2>"$replay_stderr" || replay_rc=$?

  doctor_rc=0
  AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE="$workspace" \
  AXIOMRUNNER_RUNTIME_ARTIFACT_WORKSPACE="$artifacts" \
  AXIOMRUNNER_RUNTIME_STATE_PATH="$state_path" \
  "$BIN_PATH" doctor --json >"$doctor_json" 2>"$doctor_stderr" || doctor_rc=$?

  failed_intents=unknown
  false_success_intents=unknown
  false_done_intents=unknown
  weak_verifications=0
  unresolved_verifications=0
  pack_required_verifications=0
  if [ -f "$replay_stdout" ]; then
    replay_summary=$(grep '^replay summary ' "$replay_stdout" | tail -n 1 || true)
    if [ -n "$replay_summary" ]; then
      failed_intents=$(printf '%s\n' "$replay_summary" | sed -n 's/.*failed_intents=\([0-9][0-9]*\).*/\1/p')
      false_success_intents=$(printf '%s\n' "$replay_summary" | sed -n 's/.*false_success_intents=\([0-9][0-9]*\).*/\1/p')
      false_done_intents=$(printf '%s\n' "$replay_summary" | sed -n 's/.*false_done_intents=\([0-9][0-9]*\).*/\1/p')
      [ -n "$failed_intents" ] || failed_intents=unknown
      [ -n "$false_success_intents" ] || false_success_intents=unknown
      [ -n "$false_done_intents" ] || false_done_intents=unknown
    fi
    weak_verifications=$(grep -c 'replay verification status=verification_weak' "$replay_stdout" || true)
    unresolved_verifications=$(grep -c 'replay verification status=verification_unresolved' "$replay_stdout" || true)
    pack_required_verifications=$(grep -c 'replay verification status=pack_required' "$replay_stdout" || true)
  fi

  status=ok
  if [ "$run_rc" -ne 0 ] || [ "$replay_rc" -ne 0 ] || [ "$doctor_rc" -ne 0 ]; then
    status=failed
    FAILURES=$((FAILURES + 1))
  elif [ "$false_success_intents" != "0" ] || [ "$false_done_intents" != "0" ] \
    || [ "$weak_verifications" != "0" ] || [ "$unresolved_verifications" != "0" ] \
    || [ "$pack_required_verifications" != "0" ]; then
    status=failed
    FAILURES=$((FAILURES + 1))
  fi

  printf '%s fixture=%s run_rc=%s replay_rc=%s doctor_rc=%s failed_intents=%s false_success_intents=%s false_done_intents=%s weak_verifications=%s unresolved_verifications=%s pack_required_verifications=%s\n' \
    "$status" "$fixture" "$run_rc" "$replay_rc" "$doctor_rc" "$failed_intents" "$false_success_intents" "$false_done_intents" "$weak_verifications" "$unresolved_verifications" "$pack_required_verifications" >>"$SUMMARY_PATH"
done

printf 'failures=%s\n' "$FAILURES" >>"$SUMMARY_PATH"

if [ "$FAILURES" -ne 0 ]; then
  exit 1
fi
