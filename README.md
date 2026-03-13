# AxonRunner

AxonRunner는 로컬 워크스페이스 자동화를 위한 goal-file 중심 CLI agent runtime이다.

제품 표면은 의도적으로 좁다. 지금 정식으로 노출하는 것은 `run`, `status`, `replay`, `resume`, `abort`, `doctor`, `health`, `help`다. `batch`와 legacy single-intent alias(`read`, `write`, `remove`, `freeze`, `halt`)는 compatibility 경로로만 유지한다.

## 현재 제품면

정식 명령:

- `run <goal-file>`
- `status [run-id|latest]`
- `replay [run-id|latest]`
- `resume [run-id|latest]`
- `abort [run-id|latest]`
- `doctor [--json]`
- `health`
- `help`

compatibility surface:

- `batch [--reset-state] <intent-spec>...`
- `read <key>`
- `write <key> <value>`
- `remove <key>`
- `freeze`
- `halt`

legacy intent spec:

- `read:<key>`
- `write:<key>=<value>`
- `remove:<key>`
- `freeze`
- `halt`

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
    { "label": "release gate", "detail": "cargo test -p axonrunner_apps --test release_security_gate" }
  ],
  "budget": { "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 },
  "approval_mode": "never"
}
EOF

./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  run GOAL.json
```

대기 중인 run을 다시 진행:

```bash
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  resume latest
```

CLI 표면 확인:

```bash
./target/debug/axonrunner_apps --help
```

현재 runtime / path / provider 상태 점검:

```bash
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  doctor --json
```

가장 최근 run 요약 replay:

```bash
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  replay latest
```

## 설정 표면

정식 config surface:

- `--profile=<name>` / `profile=...`
- `--provider=<id>` / `provider=...`
- `--provider-model=<name>` / `provider_model=...`
- `--workspace=<path>` / `workspace=...`
- `--state-path=<path>` / `state_path=...`

`--config-file=<path>` 또는 `--config-file <path>` 로 설정 파일을 읽을 수 있다.

환경 변수로도 같은 값을 줄 수 있다:

- `AXONRUNNER_PROFILE`
- `AXONRUNNER_RUNTIME_PROVIDER`
- `AXONRUNNER_RUNTIME_PROVIDER_MODEL`
- `AXONRUNNER_RUNTIME_TOOL_WORKSPACE`
- `AXONRUNNER_RUNTIME_STATE_PATH`

env-only runtime knobs:

- `AXONRUNNER_RUNTIME_MEMORY_PATH`
- `AXONRUNNER_RUNTIME_TOOL_LOG_PATH`
- `AXONRUNNER_RUNTIME_MAX_TOKENS`
- `AXONRUNNER_CODEX_BIN`
- `AXONRUNNER_EXPERIMENTAL_OPENAI`
- `OPENAI_API_KEY`

`provider=codek` contract:

- bundled crate pin: `codex-runtime 0.5.0`
- minimum supported Codex CLI: `0.104.0`
- `doctor --json` exposes `cli_bin`, detected `version`, and `compatibility`
  through provider health detail
- session reuse는 `cwd`와 `model`이 같을 때만 허용된다

## 실행 의미

- `run <goal-file>`은 run id, step journal, verify/report artifact를 남긴다.
- `resume`과 `abort`는 pending run control state를 기준으로 동작한다.
- compatibility 경로의 `read`, `write`, `remove`, `freeze`, `halt`도 기존 canonical path를 유지한다.
- provider/tool/memory 단계 실패는 성공 종료로 숨기지 않고 process failure로 승격된다.
- provider health는 `ready`, `degraded`, `blocked`로 노출된다.
- `openai` provider는 기본 비활성 experimental 경로다. 실제 사용은 `AXONRUNNER_EXPERIMENTAL_OPENAI=1` opt-in 이후에만 허용된다.

## 문서

- capability matrix: [docs/CAPABILITY_MATRIX.md](docs/CAPABILITY_MATRIX.md)
- substrate contract: [docs/CODEK_RUNTIME_CONTRACT.md](docs/CODEK_RUNTIME_CONTRACT.md)
- runbook: [docs/RUNBOOK.md](docs/RUNBOOK.md)
- versioning policy: [docs/VERSIONING.md](docs/VERSIONING.md)
- 제품 charter: [docs/project-charter.md](docs/project-charter.md)
- autonomous target bridge: [docs/AUTONOMOUS_AGENT_TARGET.md](docs/AUTONOMOUS_AGENT_TARGET.md)
- autonomous run contract draft: [docs/AUTONOMOUS_AGENT_SPEC.md](docs/AUTONOMOUS_AGENT_SPEC.md)
- docs alignment guide: [docs/DOCS_ALIGNMENT.md](docs/DOCS_ALIGNMENT.md)
- transition index: [docs/transition/README.md](docs/transition/README.md)
