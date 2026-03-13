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

## 4. Status / Resume / Abort

```bash
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  status latest

./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  resume latest

./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  --state-path="$PWD/.axonrunner/state.snapshot" \
  abort latest
```

확인할 것:

- `run_id`
- `phase`
- `outcome`
- pending approval 또는 budget exhaustion reason

## 5. Replay

```bash
./target/debug/axonrunner_apps \
  --workspace="$PWD" \
  replay run-1
```

확인할 것:

- `step_ids`
- `replay step`
- `changed_paths`
- patch evidence lines
- failure boundary가 있는지

## 6. Compatibility Reset Semantics

compatibility CLI surface:

```bash
./target/debug/axonrunner_apps --workspace="$PWD" batch --reset-state write:alpha=1
./target/debug/axonrunner_apps --workspace="$PWD" read alpha
./target/debug/axonrunner_apps --workspace="$PWD" health
./target/debug/axonrunner_apps --workspace="$PWD" help
```

- `batch --reset-state`는 persisted domain state snapshot만 초기화한다.
- trace/events와 기존 artifact 파일을 자동 삭제하지 않는다.
- trace/artifact reset surface 분리는 아직 별도 CLI flag로 채택하지 않았다.

## 7. High-risk Tool Operations

위험도는 아래 3단계로 본다.

- `low`: `list_files`, `read_file`, `search_files`
- `medium`: 작은 `file_write`, bounded `replace_in_file`, 일반 `run_command`
- `high`: `remove_path`, 큰 `file_write`, 큰 `replace_in_file`, `git` 같은 파급력 큰 `run_command`

추가 규칙:

- `remove_path`: 삭제 전 evidence artifact와 trace/report 설명이 남아야 한다.
- `run_command`: allowlisted program만 실행되며 command artifact가 남아야 한다.
- `high` 작업은 이후 approval policy가 붙을 때 기본 심사 대상이 된다.

## 8. Async Host

- async host는 env 또는 기본값으로 worker/thread budget을 결정한다.
- init failure가 나면 fallback host가 뜨고, 이 상태는 operator-visible output으로 확인 가능해야 한다.

## 9. Single-writer Lock

- mutating command(`run`, `batch`, `resume`, `abort`)는 workspace별 `.axonrunner/runtime.lock`을 먼저 잡아야 한다.
- lock이 이미 있으면 새 mutating command는 바로 중단된다.
- `status`, `replay`, `doctor`, `health`, `help`는 lock 없이 읽을 수 있다.
- 비정상 종료 뒤 lock이 남아 있으면, 실제로 다른 run이 없는 것을 확인한 뒤 lock 파일을 지운다.

## 10. Release Evidence

release 전 최소 확인 묶음:

- `cargo test -p axonrunner_apps --test autonomous_eval_corpus`
- `cargo test -p axonrunner_apps --test release_security_gate`
- `cargo test -p axonrunner_adapters`

autonomous eval corpus는 아래 representative run을 계속 확인해야 한다.

- goal intake visibility
- approval-required 후 resume
- budget exhaustion visibility
- workspace lock blocking
- replay quality(step journal / changed paths / failure visibility)
