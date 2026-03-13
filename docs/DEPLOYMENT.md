# AxonRunner Deployment

## Scope

이 문서는 현재 제품면만 다룬다.

정식 명령:

- `run`
- `batch`
- `replay`
- `status`
- `health`
- `help`

legacy alias:

- `read`
- `write`
- `remove`
- `freeze`
- `halt`

`agent`, daemon, gateway, cron, onboarding, skills 배포 설명은 현재 제품면에서 제외한다.

## 정식 설정 표면

다음 값은 CLI 옵션, config file, env 중 하나로 줄 수 있다.

| 의미 | CLI | config file | env | 기본값 |
|---|---|---|---|---|
| profile | `--profile=<name>` | `profile=...` | `AXONRUNNER_PROFILE` | `prod` |
| provider | `--provider=<id>` | `provider=...` | `AXONRUNNER_RUNTIME_PROVIDER` | `mock-local` |
| provider model | `--provider-model=<name>` | `provider_model=...` | `AXONRUNNER_RUNTIME_PROVIDER_MODEL` | provider id |
| workspace | `--workspace=<path>` | `workspace=...` | `AXONRUNNER_RUNTIME_TOOL_WORKSPACE` | 현재 작업 디렉터리 |
| state path | `--state-path=<path>` | `state_path=...` | `AXONRUNNER_RUNTIME_STATE_PATH` | `~/.axonrunner/state.snapshot` |

설정 파일은 `--config-file=<path>` 또는 `--config-file <path>` 로 읽는다.

## Env-only knobs

다음 값은 현재 env-only다.

| 변수 | 기본값 | 설명 |
|---|---|---|
| `AXONRUNNER_RUNTIME_MEMORY_PATH` | `~/.axonrunner/memory.db` | recall memory backend 경로 |
| `AXONRUNNER_RUNTIME_TOOL_LOG_PATH` | `<workspace>/runtime.log` | tool log 경로 |
| `AXONRUNNER_RUNTIME_MAX_TOKENS` | `4096` | provider max tokens |
| `AXONRUNNER_CODEX_BIN` | `codex` | `provider=codek`일 때 사용할 Codex CLI |
| `AXONRUNNER_EXPERIMENTAL_OPENAI` | unset | `provider=openai`를 experimental로 opt-in 할 때만 사용 |
| `OPENAI_API_KEY` | unset | `provider=openai` opt-in 후 실제 호출에 필요 |

## Minimal Local Run

workspace-bound write:

```bash
cargo build

./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  run "write:profile=prod"
```

다른 프로세스에서 persisted read:

```bash
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  run "read:profile"
```

freeze persistence 확인:

```bash
./target/debug/axonrunner_apps --state-path="$PWD/.axonrunner/state.snapshot" freeze
./target/debug/axonrunner_apps --state-path="$PWD/.axonrunner/state.snapshot" run "write:profile=dev"
```

codek-backed run:

```bash
./target/debug/axonrunner_apps \
  --provider=codek \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  run "write:profile=prod"
```

CLI surface:

```bash
./target/debug/axonrunner_apps --help
```

latest replay:

```bash
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  replay latest
```

experimental openai health probe:

```bash
AXONRUNNER_RUNTIME_PROVIDER=openai \
AXONRUNNER_EXPERIMENTAL_OPENAI=1 \
./target/debug/axonrunner_apps health
```

## Verification

배포 전 최소 검증:

```bash
cargo fmt --all --check
cargo test -p axonrunner_core
cargo test -p axonrunner_adapters
cargo test -p axonrunner_apps
```
