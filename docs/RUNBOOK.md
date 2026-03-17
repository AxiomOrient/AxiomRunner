# AxiomRunner Runbook

공식 identity: 제품 이름 `AxiomRunner` / 바이너리 `axiomrunner_apps` / env prefix `AXIOMRUNNER_`

## 1. Build

```bash
cargo build
```

## 2. Doctor

```bash
./target/debug/axiomrunner_apps \
  --workspace="$PWD" \
  doctor --json
```

확인 항목: `provider_state`, `memory_state`, `tool_state`, `lock_state`, `lock_path`
provider detail에서 `cli_bin`, `version`, `compatibility` 확인.
tool detail에서 `workspace`, `command_timeout_ms` 확인.

## 3. Run

```bash
cat > GOAL.json <<'EOF'
{
  "summary": "Check one workspace contract",
  "workspace_root": ".",
  "constraints": [],
  "done_conditions": [
    { "label": "report", "evidence": "report_artifact_exists" }
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

minimal onboarding 순서: `doctor` → `run` → `status` → `replay` → `report.md`

예제 파일: `examples/minimal_goal.json`, `examples/minimal_pack.json`

직접 goal을 만들 때는 raw `goal.json`부터 시작하지 말고 아래 순서를 권장한다:

1. brief 작성
2. static example goal을 복사하거나 `python3 tools/dev/generate_goal_stack.py ...` 를 dev helper로 사용
3. generated goal을 실행 전 검토하고 필요하면 supported done DSL로 수정

참고:

- guide: `docs/GOAL_STACK_PLAYBOOK.md`
- sample brief: `examples/goal_stacks/axiomrunner_dogfood.brief.json`
- sample output: `examples/goal_stacks/axiomrunner_dogfood/`

## 4. Status / Resume / Abort

`resume`은 `waiting_approval` 상태 pending run만 허용된다. generic restart가 아니다.
`abort`는 pending run을 terminal outcome으로 닫는다. rerun이 아니다.

```bash
./target/debug/axiomrunner_apps --workspace="$PWD" \
  --state-path="$PWD/.axiomrunner/state.snapshot" status latest

./target/debug/axiomrunner_apps --workspace="$PWD" \
  --state-path="$PWD/.axiomrunner/state.snapshot" resume latest

./target/debug/axiomrunner_apps --workspace="$PWD" \
  --state-path="$PWD/.axiomrunner/state.snapshot" abort latest
```

**Terminal outcomes**:

| outcome | 의미 |
|---|---|
| `success` | verification passed + done conditions 모두 증거 있음 |
| `approval_required` | `waiting_approval` 상태. `resume`으로 재개 |
| `budget_exhausted` | step/minute/token budget 소진 |
| `blocked` | weak verifier (`verification_weak`, `verification_unresolved`, `pack_required`) |
| `failed` | provider/tool/workspace 실패 |
| `aborted` | operator `abort` 명령으로 닫힘 |

**Budget rule**:
- step budget: `planned_steps + repair_attempts` 기준
- minute budget: run 전체 `elapsed_ms` 기준
- token budget: runtime provider request ceiling 미만이면 pre-execution guard로 즉시 차단

**Verifier strength**: `verification.status` 값이 `verifier_strength`와 동일한 어휘를 쓴다.
`verification_weak` / `verification_unresolved` / `pack_required` run은 `success`로 보이면 안 된다.

## 5. Replay

```bash
./target/debug/axiomrunner_apps \
  --workspace="$PWD" replay latest
```

확인 항목: `step_ids`, `replay verification`, `replay verifier_evidence`, `replay step`, `changed_paths`, patch evidence, failure boundary

## 6. High-risk Tool Operations

| 위험도 | operations |
|---|---|
| low | `list_files`, `read_file`, `search_files` |
| medium | 작은 `file_write`, bounded `replace_in_file`, 일반 `run_command` |
| high | `remove_path`, 큰 `file_write`/`replace_in_file`, `git` 등 파급 큰 `run_command` |

추가 규칙:
- `remove_path`: 삭제 전 evidence artifact와 trace/report 설명이 남아야 한다.
- `run_command`: allowlisted program만 실행. command artifact가 남아야 한다.
- `replace_in_file`: multi-match는 `expected_replacements`가 실제 count와 맞을 때만 허용.
- `search_files`: unreadable file은 `skipped_files`로 노출.
- `high` 작업은 approval policy 적용 시 기본 심사 대상.

## 7. Async Host

- async host는 env 또는 기본값으로 worker/thread budget을 결정한다.
- init failure는 fallback runtime으로 숨기지 않고 `doctor`에 `init_mode=failed,error=...`로 노출한다.

## 8. Single-writer Lock

- mutating command(`run`, `resume`, `abort`)는 `.axiomrunner/runtime.lock`을 먼저 잡는다.
- lock이 이미 있으면 새 mutating command는 즉시 중단.
- Unix에서는 lock holder pid가 죽어 있으면 stale lock으로 보고 한 번 자동 정리 후 재시도한다.
- 비-Unix에서는 stale 여부를 확정하지 않고 active로 본다.
- `status`, `replay`, `doctor`, `health`, `help`는 lock 없이 읽는다.
- 자동 복구 불가 lock만 operator가 직접 확인 후 제거.

## 9. Trace Append Policy

- trace는 JSONL append-only.
- partial write(newline 없이 끊긴 trailing line)는 무시하고 앞의 정상 이벤트는 계속 읽는다.
- newline까지 끝난 malformed line은 corruption으로 보고 실패한다.

## 10. Rollback Recovery

isolated worktree run이 `failed` 또는 `blocked`로 끝나면:
1. `.axiomrunner/artifacts/<intent>.rollback.json`을 확인한다.
2. `restore_path`: 원래 workspace. `cleanup_path`: 실패한 isolated worktree.
3. `replay <run-id>`와 `report.md`의 `rollback=` 줄을 먼저 확인한다.
4. `restore_path` 기준으로 복구 진행. `cleanup_path`는 증거 확인 후 정리.

`abort`는 pure terminal control이며 rollback 메타데이터를 생성하지 않는다.

현재 opt-in: git workspace에서 `AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION=1`일 때만 worktree isolation 사용.

## 11. Nightly Dogfood

```bash
sh ./scripts/nightly_dogfood.sh
```

로그: `var/nightly_dogfood/<timestamp>/` 아래 fixture별 `run_rc`, `replay_rc`, `doctor_rc`, `summary.txt`

주요 환경 변수:
- `AXIOMRUNNER_NIGHTLY_FIXTURES=intake.json,rust_service.json`
- `AXIOMRUNNER_NIGHTLY_LOG_ROOT=/tmp/axiomrunner-nightly`
- `AXIOMRUNNER_NIGHTLY_SKIP_BUILD=1`
- `AXIOMRUNNER_NIGHTLY_BIN=./target/debug/axiomrunner_apps`

## 12. Release Evidence

```bash
cargo test -p axiomrunner_apps --test autonomous_eval_corpus
cargo test -p axiomrunner_apps --test fault_path_suite
cargo test -p axiomrunner_apps --test nightly_dogfood_contract
cargo test -p axiomrunner_apps --test release_security_gate
cargo test -p axiomrunner_adapters
```

대표 수동 dogfood:

```bash
cargo run -q -p axiomrunner_apps -- \
  --provider=codek \
  --workspace="$PWD/examples/rust_service" \
  run examples/rust_service/goal.json

cargo run -q -p axiomrunner_apps -- \
  --provider=codek \
  --workspace="$PWD/examples/node_api" \
  run examples/node_api/goal.json
```

autonomous eval corpus 확인 항목:
- goal intake visibility
- approval-required 후 resume
- budget exhaustion visibility
- weak verification / pack required visibility
- workspace lock blocking
- replay quality (step journal / changed paths / failure visibility)

## 13. Versioning Policy

- retained CLI surface를 깨면 minor가 아니라 breaking release로 취급한다.
- provider compatibility minimum 상향 시 CHANGELOG와 decision record를 함께 남긴다.
- experimental capability 변화는 primary product contract와 분리해 기록한다.
- exit code도 retained CLI surface 일부다.

**Exit Codes**:

| code | 의미 |
|---|---|
| 0 | success |
| 2 | parse error |
| 3 | config error |
| 4 | release gate error |
| 5 | runtime init error |
| 6 | runtime execution error |
| 7 | runtime shutdown error |

**Changelog 규칙**:
- retained commands 변화는 첫 줄에 드러낸다.
- removed surface와 experimental surface는 분리해서 기록한다.
- substrate pin 변경은 version, 이유, rollback condition을 함께 기록한다.
- exit code 변경은 breaking change로 기록한다.

## 14. Product Milestones

**v0.1 Honest Autonomous Runtime**
- retained CLI surface `run/status/replay/resume/abort/doctor/health/help` 고정
- approval, budget, resume, abort semantics가 status/replay/report에서 일관
- release blocker와 exit code가 operator 문서와 테스트에 함께 잠김

**v0.2 Safe Developer Automation**
- representative example pack을 공식 operator asset으로 사용
- app/server verifier 흐름을 example goal/pack에서 재현 가능
- isolated worktree, rollback evidence, nightly dogfood가 운영 루프에 연결
