# AxonRunner Runbook

## 1. Build

```bash
cargo build
```

## 2. Doctor

```bash
./target/debug/axonrunner_apps \
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
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  run "write:profile=prod"
```

## 4. Replay

```bash
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  replay latest
```

확인할 것:

- `changed_paths`
- patch evidence lines
- failure boundary가 있는지

## 5. Batch Reset Semantics

- `batch --reset-state`는 persisted domain state snapshot만 초기화한다.
- trace/events와 기존 artifact 파일을 자동 삭제하지 않는다.
- trace/artifact reset surface 분리는 아직 별도 CLI flag로 채택하지 않았다.

## 6. High-risk Tool Operations

- `remove_path`: 삭제 전 evidence artifact와 trace/report 설명이 남아야 한다.
- `run_command`: allowlisted program만 실행되며 command artifact가 남아야 한다.

## 7. Async Host

- async host는 env 또는 기본값으로 worker/thread budget을 결정한다.
- init failure가 나면 fallback host가 뜨고, 이 상태는 operator-visible output으로 확인 가능해야 한다.
