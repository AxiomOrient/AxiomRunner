# AxonRunner

AxonRunner는 로컬 워크스페이스 자동화를 위한 minimal event-sourced CLI runtime이다.

제품 표면은 의도적으로 좁다. 지금 보장하는 것은 `run`, `batch`, `replay`, `status`, `health`, `help`와 legacy single-intent alias(`read`, `write`, `remove`, `freeze`, `halt`) 뿐이다.

## 현재 제품면

정식 명령:

- `run <intent-spec>`
- `batch [--reset-state] <intent-spec>...`
- `replay <intent-id|latest>`
- `status`
- `health`
- `help`

legacy alias:

- `read <key>`
- `write <key> <value>`
- `remove <key>`
- `freeze`
- `halt`

intent spec:

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

도메인 상태와 tool 로그를 남기면서 한 번 실행:

```bash
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  run "write:profile=prod"
```

같은 상태를 다른 프로세스에서 읽기:

```bash
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  run "read:profile"
```

CLI 표면 확인:

```bash
./target/debug/axonrunner_apps --help
```

가장 최근 intent 요약 replay:

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

## 실행 의미

- `read`도 이제 다른 intent와 같은 canonical path를 타며 intent id와 revision이 남는다.
- `freeze`와 `halt`는 persisted state로 유지된다.
- provider/tool/memory 단계 실패는 성공 종료로 숨기지 않고 process failure로 승격된다.
- provider health는 `ready`, `degraded`, `blocked`로 노출된다.
- `openai` provider는 기본 비활성 experimental 경로다. 실제 사용은 `AXONRUNNER_EXPERIMENTAL_OPENAI=1` opt-in 이후에만 허용된다.

## 문서

- 배포/운영 설정: [docs/DEPLOYMENT.md](/Users/axient/repository/AxonRunner/docs/DEPLOYMENT.md)
- 제품 charter: [docs/project-charter.md](/Users/axient/repository/AxonRunner/docs/project-charter.md)
- 과거 blueprint 성격 문서는 `docs/0*.md`와 `issue/` 아래에 남아 있으며 현재 제품 README의 source of truth는 아니다.
