# AxiomRunner Runbook

공식 identity:

- 제품 이름: `AxiomRunner`
- 바이너리 이름: `axiomrunner_apps`
- 환경 변수 prefix: `AXIOMRUNNER_`

## 1. Build

```bash
cargo build
```

## 1A. Minimal Onboarding

운영자가 최소 흐름만 보려면 아래 순서만 쓴다.

retained CLI surface: `run/status/replay/resume/abort/doctor/health/help`

1. `doctor`
2. `run`
3. `status`
4. `replay`
5. `report.md`

예제 파일:

- `examples/minimal_goal.json`
- `examples/minimal_pack.json`

representative verifier examples:

- `examples/rust_service`
- `examples/node_api`
- `examples/nextjs_app`
- `examples/python_fastapi`
- 실행 예와 파일 설명: `examples/README.md`

## 2. Doctor

```bash
./target/debug/axiomrunner_apps \
  --workspace="$PWD" \
  doctor --json
```

확인할 것:

- `provider_state`
- `memory_state`
- `tool_state`
- provider detail의 `cli_bin`, `version`, `compatibility`

## 3. Run

```bash
cat > GOAL.json <<'EOF'
{
  "summary": "Check one workspace contract",
  "workspace_root": ".",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report artifact exists" }
  ],
  "verification_checks": [
    { "label": "release gate", "detail": "cargo test -p axiomrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}
EOF

./target/debug/axiomrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axiomrunner/state.snapshot" \
  run GOAL.json
```

## 4. Status / Resume / Abort

`resume`은 generic restart가 아니다. `waiting_approval` 상태의 pending run을 승인 후 재개할 때만 쓴다.
`abort`도 rerun이 아니다. 현재 pending run을 terminal outcome으로 닫을 때만 쓴다.

```bash
./target/debug/axiomrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axiomrunner/state.snapshot" \
  status latest

./target/debug/axiomrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axiomrunner/state.snapshot" \
  resume latest

./target/debug/axiomrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axiomrunner/state.snapshot" \
  abort latest
```

확인할 것:

- `run_id`
- `phase`
- `outcome`
- `execution_workspace`
- `resume`은 generic restart가 아니라 `waiting_approval` 상태의 goal-file pending run 승인 후 재개에서만 허용된다.
- `abort`는 rerun이 아니라 pending goal-file control state를 terminal outcome으로 닫을 때만 허용된다.
- normal goal run은 `completed/success`
- `approval_mode=always` 또는 `approval_mode=on-risk` goal은 default pack 경로에서 `waiting_approval`
- budget exhaustion이면 `budget_exhausted`
- weak verifier면 `verification_weak`, `verification_unresolved`, `pack_required` reason으로 `blocked`
- reason

budget rule:

- step budget은 `planned_steps + repair_attempts` 기준으로 읽는다.
- minute budget은 run 전체 `elapsed_ms` 기준으로 읽는다.
- token budget은 runtime provider request ceiling보다 작으면 pre-execution guard로 바로 막는다.
- goal path는 pre-execution guard, repair budget, elapsed minute budget으로 막는다.

## 5. Replay

```bash
./target/debug/axiomrunner_apps \
  --workspace="$PWD" \
  replay run-1
```

확인할 것:

- `step_ids`
- `replay verification`
- `replay verifier_evidence`
- `replay step`
- `changed_paths`
- patch evidence lines
- failure boundary가 있는지

## 6. High-risk Tool Operations

위험도는 아래 3단계로 본다.

- `low`: `list_files`, `read_file`, `search_files`
- `medium`: 작은 `file_write`, bounded `replace_in_file`, 일반 `run_command`
- `high`: `remove_path`, 큰 `file_write`, 큰 `replace_in_file`, `git` 같은 파급력 큰 `run_command`

추가 규칙:

- `remove_path`: 삭제 전 evidence artifact와 trace/report 설명이 남아야 한다.
- `run_command`: allowlisted program만 실행되며 command artifact가 남아야 한다.
- `replace_in_file`: multi-match 치환은 `expected_replacements`가 실제 count와 맞을 때만 허용된다.
- `search_files`: unreadable file은 `skipped_files`로 보여야 한다.
- `high` 작업은 이후 approval policy가 붙을 때 기본 심사 대상이 된다.

## 8. Async Host

- async host는 env 또는 기본값으로 worker/thread budget을 결정한다.
- init failure는 fallback runtime으로 숨기지 않고 `doctor`에 `init_mode=failed,error=...`로 드러나야 한다.

## 9. Single-writer Lock

- mutating command(`run`, `resume`, `abort`)는 workspace별 `.axiomrunner/runtime.lock`을 먼저 잡아야 한다.
- lock이 이미 있으면 새 mutating command는 바로 중단된다.
- lock holder pid가 이미 죽어 있으면 stale lock으로 보고 한 번 자동 정리 후 재시도한다.
- `status`, `replay`, `doctor`, `health`, `help`는 lock 없이 읽을 수 있다.
- 자동 복구가 안 되는 lock만 operator가 직접 확인 후 지운다.
- `doctor`는 `lock_state`와 `lock_path`를 같이 보여 준다.

## 9A. Trace Append Policy

- trace는 JSONL append only 로 쓴다.
- read 쪽은 마지막 줄이 newline 없이 끊긴 partial write 면 그 줄만 무시하고 앞의 정상 이벤트는 계속 읽는다.
- 이미 newline 까지 끝난 malformed line 은 corruption 으로 보고 그대로 실패한다.
- 따라서 read-while-write 는 trailing partial line 1개까지만 복구 대상으로 본다.

## 10. Release Evidence

release 전 최소 확인 묶음:

- `cargo test -p axiomrunner_apps --test autonomous_eval_corpus`
- `cargo test -p axiomrunner_apps --test fault_path_suite`
- `cargo test -p axiomrunner_apps --test nightly_dogfood_contract`
- `cargo test -p axiomrunner_apps --test release_security_gate`
- `cargo test -p axiomrunner_adapters`

autonomous eval corpus는 아래 representative run을 계속 확인해야 한다.

- goal intake visibility
- approval-required 후 resume
- budget exhaustion visibility
- weak verification / pack required visibility
- workspace lock blocking
- replay quality(step journal / changed paths / failure visibility)

## 10A. Rollback Recovery

- isolated worktree run이 `failed` 또는 `blocked`로 끝나면 `.axiomrunner/artifacts/<intent>.rollback.json` 을 본다.
- `restore_path` 는 원래 workspace다.
- `cleanup_path` 가 있으면 실패한 isolated worktree다.
- 운영자는 먼저 `replay <run-id>` 와 `report.md` 의 `rollback=` 줄을 확인한다.
- 복구는 `restore_path` 를 기준으로 계속 진행하고, 실패한 `cleanup_path` 는 증거 확인 뒤 정리한다.
- `abort` 는 pure terminal control 이라 rollback 메타데이터를 새로 만들지 않는다.
- current support status: git workspace에서 `AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION=1` 일 때만 opt-in worktree isolation을 사용한다.

## 10B. Adapter Authoring Boundary

- user-provided adapter는 provider/tool/memory backend만 제공한다.
- adapter는 `run`, `resume`, `abort`, `done`, `status`, `replay`, `report` 의미를 바꾸면 안 된다.
- workflow pack은 verifier flow와 허용 도구를 제안할 수 있지만 terminal outcome이나 replay schema를 새로 만들 수 없다.
- backend 오류는 숨긴 fallback success로 바꾸지 않고 operator-visible failure로 올린다.

## 11. Nightly Dogfood

GitHub Actions가 없더라도 아래 스크립트로 로컬 nightly dogfood 묶음을 반복 실행할 수 있다.

```bash
sh ./scripts/nightly_dogfood.sh
```

기본 동작:

- representative goal fixture를 순서대로 실행한다.
- 각 fixture마다 별도 workspace / artifact / state 경로를 만든다.
- `run`, `replay latest`, `doctor --json` 결과를 로그로 남긴다.
- 로그는 `var/nightly_dogfood/<timestamp>/` 아래에 쌓인다.

주요 환경 변수:

- `AXIOMRUNNER_NIGHTLY_FIXTURES=intake.json,rust_service.json`
- `AXIOMRUNNER_NIGHTLY_LOG_ROOT=/tmp/axiomrunner-nightly`
- `AXIOMRUNNER_NIGHTLY_SKIP_BUILD=1`
- `AXIOMRUNNER_NIGHTLY_BIN=./target/debug/axiomrunner_apps`

확인할 것:

- `summary.txt`에 fixture별 `run_rc`, `replay_rc`, `doctor_rc`가 남는지
- `summary.txt`에 `failed_intents`, `false_success_intents`, `false_done_intents`가 남는지
- 실패가 있으면 `failures>0`으로 끝나는지
- 각 fixture 로그에 `phase`, `outcome`, `provider_state`가 남는지

## 12. Release Candidate Dry Run

release candidate dry run 기본 묶음:

- `cargo test -p axiomrunner_apps --test autonomous_eval_corpus`
- `cargo test -p axiomrunner_apps --test fault_path_suite`
- `cargo test -p axiomrunner_apps --test nightly_dogfood_contract`
- `cargo test -p axiomrunner_apps --test release_security_gate`

이 묶음이 모두 통과하고, representative fixture summary 와 report artifact 가 읽히면 RC dry run 성공으로 본다.

## 13. Product Milestones

### v0.1 Honest Autonomous Runtime

- retained CLI surface가 `run/status/replay/resume/abort/doctor/health/help` 로 고정된다.
- approval, budget, resume, abort semantics가 status/replay/report에서 같은 뜻으로 보인다.
- release blocker와 exit code가 operator 문서와 테스트에 같이 잠긴다.

### v0.2 Safe Developer Automation

- representative example pack을 공식 operator asset으로 쓴다.
- app/server verifier 흐름을 example goal/pack에서 바로 재현할 수 있다.
- isolated worktree, rollback evidence, nightly dogfood가 운영 루프에 연결된다.
