# AxonRunner Deployment

## Scope

이 문서는 현재 retained surface 기준으로만 설명한다.

남아 있는 실행 표면:
- `run`
- `batch`
- `status`
- `health`

legacy single-intent aliases:
- `read`
- `write`
- `remove`
- `freeze`
- `halt`

채널, 데몬, 게이트웨이, cron, service, onboarding, skills 배포 설명은 제거했다.

## Required Environment

| 변수 | 기본값 | 설명 |
|---|---|---|
| `AXONRUNNER_PROFILE` | `prod` | 실행 프로파일 |
| `AXONRUNNER_RUNTIME_PROVIDER` | `mock-local` | `mock-local`, `codek`, `openai` |
| `AXONRUNNER_RUNTIME_PROVIDER_MODEL` | provider id | provider 모델명 |
| `AXONRUNNER_RUNTIME_MAX_TOKENS` | `4096` | provider 최대 토큰 |
| `AXONRUNNER_RUNTIME_MEMORY_PATH` | `~/.axonrunner/memory.db` | 메모리 경로 |
| `AXONRUNNER_RUNTIME_TOOL_WORKSPACE` | `~/.axonrunner/workspace` | tool workspace |
| `AXONRUNNER_RUNTIME_TOOL_LOG_PATH` | `runtime.log` | tool log 파일 |
| `AXONRUNNER_CODEX_BIN` | `codex` | `AXONRUNNER_RUNTIME_PROVIDER=codek`일 때 사용할 Codex CLI 경로 |
| `OPENAI_API_KEY` | unset | `AXONRUNNER_RUNTIME_PROVIDER=openai`일 때 필수 |

## Minimal Local Run

canonical run:

```bash
cargo build

AXONRUNNER_RUNTIME_MEMORY_PATH="$HOME/.axonrunner/memory.md" \
AXONRUNNER_RUNTIME_TOOL_WORKSPACE="$HOME/.axonrunner/workspace" \
./target/debug/axonrunner_apps run "write:profile=prod"

./target/debug/axonrunner_apps run "read:profile"
```

codek-backed write attempt:

```bash
AXONRUNNER_RUNTIME_PROVIDER=codek \
AXONRUNNER_CODEX_BIN="$(command -v codex)" \
./target/debug/axonrunner_apps run "write:profile=prod"
```

single process batch:

```bash
./target/debug/axonrunner_apps batch "write:profile=prod" "read:profile" "remove:profile"
```

## Verification

배포 전 최소 검증:

```bash
cargo check
cargo test --workspace --no-run
cargo test -p axonrunner_apps --test e2e_cli
```
