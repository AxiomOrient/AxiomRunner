# AxiomRunner

AxiomRunner는 로컬 워크스페이스 자동화를 위한 goal-file 중심 CLI agent runtime이다.

공식 identity:

- 제품 이름: `AxiomRunner`
- 바이너리 이름: `axiomrunner_apps`
- 환경 변수 prefix: `AXIOMRUNNER_`

제품 표면은 의도적으로 좁다. 지금 정식으로 노출하는 것은 `run`, `status`, `replay`, `resume`, `abort`, `doctor`, `health`, `help`다.

## 현재 제품면

retained CLI surface: `run/status/replay/resume/abort/doctor/health/help`

정식 명령:

- `run <goal-file>`
- `status [run-id|latest]`
- `replay [run-id|latest]`
- `resume [run-id|latest]`
- `abort [run-id|latest]`
- `doctor [--json]`
- `health`
- `help`

## 폴더 역할

- `plans/` — 계획 문서와 작업 문서만 두는 폴더. 기본 형식은 `plans/IMPLEMENTATION-PLAN.md`, `plans/TASKS.md`, 설명은 `plans/README.md`
- `scripts/` — 제품 운영 스크립트만 둔다. 현재 retained ops script는 `nightly_dogfood.sh`
- `tools/dev/` — 개발 보조 도구만 둔다. `generate_goal_stack.py`는 제품 운영 경로가 아니라 dev helper다

## 빠른 시작

빌드:

```bash
cargo build
```

goal file을 받아 run id를 남기며 한 번 실행:

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

`resume`은 generic restart가 아니다. `waiting_approval` 상태의 pending run을 승인 후 재개할 때만 쓴다.
`abort`도 rerun이 아니다. 현재 pending run을 terminal outcome으로 닫을 때만 쓴다:

```bash
./target/debug/axiomrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axiomrunner/state.snapshot" \
  resume latest
```

CLI 표면 확인:

```bash
./target/debug/axiomrunner_apps --help
```

현재 runtime / path / provider 상태 점검:

```bash
./target/debug/axiomrunner_apps \
  --workspace="$PWD" \
  doctor --json
```

가장 최근 run 요약 replay:

```bash
./target/debug/axiomrunner_apps \
  --workspace="$PWD" \
  replay latest
```

autonomy evidence 기본 묶음:

- `cargo test -p axiomrunner_apps --test autonomous_eval_corpus`
- `cargo test -p axiomrunner_apps --test fault_path_suite`
- `cargo test -p axiomrunner_apps --test nightly_dogfood_contract`
- `cargo test -p axiomrunner_apps --test release_security_gate`

nightly 운영 경로:

- 기본 경로는 GitHub CI가 아니라 로컬/외부 스케줄러 + `scripts/nightly_dogfood.sh`

representative verifier examples:

- `examples/rust_service`
- `examples/node_api`
- `examples/nextjs_app`
- `examples/python_fastapi`
- 실행 방법: [examples/README.md](examples/README.md)

직접 goal을 잘게 쪼개서 만들 때의 권장 경로:

- brief 작성: `examples/goal_stacks/axiomrunner_dogfood.brief.json`
- 작성 가이드: [docs/GOAL_STACK_PLAYBOOK.md](docs/GOAL_STACK_PLAYBOOK.md)
- 생성 스크립트: `python3 tools/dev/generate_goal_stack.py ...` (dev helper, 기본 done condition은 `report_artifact_exists`만 생성)

release target:

- `1.0.0`: workspace-bound, verifier-first, failure-honest local runtime

## 설정 표면

정식 config surface:

- `--profile=<name>` 또는 `--profile <name>` / `profile=...`
- `--provider=<id>` 또는 `--provider <id>` / `provider=...`
- `--provider-model=<name>` 또는 `--provider-model <name>` / `provider_model=...`
- `--workspace=<path>` 또는 `--workspace <path>` / `workspace=...`
- `--state-path=<path>` 또는 `--state-path <path>` / `state_path=...`
- `--command-allowlist=<cmds>` 또는 `--command-allowlist <cmds>` / `command_allowlist=...`

`--config-file=<path>` 또는 `--config-file <path>` 로 설정 파일을 읽을 수 있다.

환경 변수로도 같은 값을 줄 수 있다:

- `AXIOMRUNNER_PROFILE`
- `AXIOMRUNNER_RUNTIME_PROVIDER`
- `AXIOMRUNNER_RUNTIME_PROVIDER_MODEL`
- `AXIOMRUNNER_RUNTIME_TOOL_WORKSPACE`
- `AXIOMRUNNER_RUNTIME_STATE_PATH`

env-only runtime knobs:

- `AXIOMRUNNER_RUNTIME_MEMORY_PATH`
- `AXIOMRUNNER_RUNTIME_TOOL_LOG_PATH`
- `AXIOMRUNNER_RUNTIME_COMMAND_TIMEOUT_MS`
- `AXIOMRUNNER_RUNTIME_MAX_TOKENS`
- `AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION`
- `AXIOMRUNNER_CODEX_BIN`
- `AXIOMRUNNER_EXPERIMENTAL_OPENAI`
- `OPENAI_API_KEY`

`provider=codek` contract:

- bundled crate pin: `codex-runtime 0.5.0`
- minimum supported Codex CLI: `0.104.0`
- `doctor --json` exposes `cli_bin`, detected `version`, and `compatibility`
  through provider health detail
- `doctor --json` tool detail exposes `workspace` and `command_timeout_ms`
- session reuse는 `cwd`와 `model`이 같을 때만 허용된다

## 실행 의미

- `run <goal-file>`은 run id, step journal, verify/report artifact를 남긴다.
- `resume`은 generic restart가 아니라 `waiting_approval` 상태의 goal-file pending run 승인 후 재개 전용이다.
- `abort`는 rerun이 아니라 pending goal-file control state를 terminal outcome으로 닫는 control이다.
- git workspace에서는 기본적으로 isolated worktree 실행을 사용한다. `AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION=0` 으로만 끌 수 있다.
- `workspace_root`는 입력 호환용 필드다. 실제 실행 경계와 done-condition 기준은 runtime `--workspace`가 결정한다.
- default goal path는 verification detail에서 command를 직접 파생한다.
- 기본 verifier command surface는 allowlisted program이 아니라 allowlisted command spec이다. `--command-allowlist`는 이 spec 집합을 더 좁히는 operator override다.
- detail에서 안전한 strong verifier를 만들 수 없으면 `verification_weak`, `verification_unresolved`, `pack_required` 로 드러나며 `success`로 숨기지 않는다.
- provider/tool/memory 단계 실패는 성공 종료로 숨기지 않고 process failure로 승격된다.
- provider health는 `ready`, `degraded`, `blocked`로 노출된다.
- `openai` provider는 기본 비활성 experimental 경로다. 실제 사용은 `AXIOMRUNNER_EXPERIMENTAL_OPENAI=1` 이후에만 허용된다.

## 문서

- 문서 입구: [docs/README.md](docs/README.md)
- 제품 charter: [docs/project-charter.md](docs/project-charter.md)
- runbook: [docs/RUNBOOK.md](docs/RUNBOOK.md)
- capability matrix: [docs/CAPABILITY_MATRIX.md](docs/CAPABILITY_MATRIX.md)
- goal/pack contract: [docs/WORKFLOW_PACK_CONTRACT.md](docs/WORKFLOW_PACK_CONTRACT.md)
- goal stack playbook: [docs/GOAL_STACK_PLAYBOOK.md](docs/GOAL_STACK_PLAYBOOK.md)
- 구조 설명: [docs/PROJECT_STRUCTURE.md](docs/PROJECT_STRUCTURE.md)
- bridge note: [docs/AUTONOMOUS_AGENT_BRIDGE.md](docs/AUTONOMOUS_AGENT_BRIDGE.md)
- versioning: [docs/VERSIONING.md](docs/VERSIONING.md)
