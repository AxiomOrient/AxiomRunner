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

developer automation milestone:

- `v0.1`: honest autonomous runtime lock
- `v0.2`: representative app/server example assets + safer workspace execution

## 설정 표면

정식 config surface:

- `--profile=<name>` / `profile=...`
- `--provider=<id>` / `provider=...`
- `--provider-model=<name>` / `provider_model=...`
- `--workspace=<path>` / `workspace=...`
- `--state-path=<path>` / `state_path=...`

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
- session reuse는 `cwd`와 `model`이 같을 때만 허용된다

## 실행 의미

- `run <goal-file>`은 run id, step journal, verify/report artifact를 남긴다.
- `resume`은 generic restart가 아니라 `waiting_approval` 상태의 goal-file pending run 승인 후 재개 전용이다.
- `abort`는 rerun이 아니라 pending goal-file control state를 terminal outcome으로 닫는 control이다.
- git workspace에서는 `AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION=1` 로 opt-in isolated worktree 실행을 지원한다.
- default goal path는 verification detail에서 command를 직접 파생한다.
- detail에서 안전한 strong verifier를 만들 수 없으면 `verification_weak`, `verification_unresolved`, `pack_required` 로 드러나며 `success`로 숨기지 않는다.
- provider/tool/memory 단계 실패는 성공 종료로 숨기지 않고 process failure로 승격된다.
- provider health는 `ready`, `degraded`, `blocked`로 노출된다.
- `openai` provider는 기본 비활성 experimental 경로다. 실제 사용은 `AXIOMRUNNER_EXPERIMENTAL_OPENAI=1` opt-in 이후에만 허용된다.

## 문서

- 문서 입구: [docs/README.md](docs/README.md)
- 구조 설명: [docs/PROJECT_STRUCTURE.md](docs/PROJECT_STRUCTURE.md)
- capability matrix: [docs/CAPABILITY_MATRIX.md](docs/CAPABILITY_MATRIX.md)
- substrate contract: [docs/CODEK_RUNTIME_CONTRACT.md](docs/CODEK_RUNTIME_CONTRACT.md)
- workflow pack contract: [docs/WORKFLOW_PACK_CONTRACT.md](docs/WORKFLOW_PACK_CONTRACT.md)
- runbook: [docs/RUNBOOK.md](docs/RUNBOOK.md)
- versioning policy: [docs/VERSIONING.md](docs/VERSIONING.md)
- 제품 charter: [docs/project-charter.md](docs/project-charter.md)
- autonomous target bridge: [docs/AUTONOMOUS_AGENT_TARGET.md](docs/AUTONOMOUS_AGENT_TARGET.md)
- autonomous run contract: [docs/AUTONOMOUS_AGENT_SPEC.md](docs/AUTONOMOUS_AGENT_SPEC.md)
