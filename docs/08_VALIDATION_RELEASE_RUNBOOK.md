# 08. Validation and Release Runbook

## 1. Release를 막는 기준

아래 중 하나라도 실패하면 release 금지다.

1. `run`/`batch` 핵심 e2e fail
2. persisted state (`freeze`/`halt`) regression fail
3. provider health 또는 provider blocked handling fail
4. tool workspace boundary regression fail
5. report artifact generation fail
6. exit code taxonomy regression fail
7. docs/help truth mismatch fail
8. dependency/license/security gate fail

## 2. 테스트 계층

### Level 1 — Core deterministic tests

대상:

- reducer
- policy
- event projection
- state invariant

### Level 2 — Adapter contract tests

대상:

- provider contract (`codek`, `mock-local`, `openai experimental gate`)
- memory contract
- tool contract

### Level 3 — CLI integration tests

대상:

- `run`
- `batch`
- `status`
- `health`
- `help`
- release gate / exit code taxonomy

### Level 4 — Product golden checks

현재 golden 성격으로 반드시 잠가야 하는 것:

- false success 금지
- workspace boundary
- provider blocked
- persisted `freeze`/`halt`
- minimal report artifact
- docs/help surface consistency

### Level 5 — Opt-in substrate smoke

대상:

- local `codex` binary 존재/부재
- codek session open-close
- one simple write path

## 3. Golden 설계 원칙

각 golden 성격 검증은 아래를 최소로 가져야 한다.

- 입력 명령
- fixture workspace 또는 isolated home
- expected exit code
- expected stdout/stderr contract
- expected state/report side effect

핵심은 시나리오 수가 아니라, 제품 계약을 깨는 failure boundary를 직접 잠그는 것이다.

## 4. 권장 명령 게이트

### Local deterministic gate

```bash
cargo fmt --all --check
cargo test -p axonrunner_core
cargo test -p axonrunner_adapters
cargo test -p axonrunner_apps
```

### Product contract gate

최소한 아래를 순서대로 통과해야 한다.

- core deterministic tests
- adapter contract tests
- CLI integration tests
- docs truth review

### Opt-in substrate smoke

```bash
AXONRUNNER_RUNTIME_PROVIDER=codek \
AXONRUNNER_CODEX_BIN="$(command -v codex)" \
./target/debug/axonrunner_apps health
```

## 5. Health contract

`health`와 `status`는 최소한 아래를 반영해야 한다.

- selected provider id/model
- provider `ready/degraded/blocked`
- memory/tool init state
- blocked reason detail
- experimental provider gate 여부

## 6. Report contract

각 intent 실행마다 아래 artifact가 남아야 한다.

- `plan`
- `apply`
- `verify`
- `report`

실패한 실행도 report를 남겨야 하며, operator는 이 artifact만 보고 failure boundary를 읽을 수 있어야 한다.

## 7. Release rehearsal

### 목적

- 실제 release 전에 현재 제품면과 운영 경로가 연결되는지 확인

### 절차

1. clean checkout
2. deterministic gate 실행
3. opt-in codek smoke
4. sample run artifact 확인
5. persisted freeze/halt rehearsal
6. docs/help truth review
7. 결과 저장

## 8. Rollback / Failure triage

최소한 아래를 검증한다.

- failed run에서 stderr prefix만으로 failure category를 구분 가능
- report artifact만으로 어떤 단계에서 실패했는지 읽힘
- persisted state가 failed compose를 따라가지 않음
- tool/workspace side effect가 failure semantics와 일치함

## 9. 문서 동기화 게이트

자동 또는 반자동으로 점검해야 할 것:

- README의 CLI surface
- README의 provider 위상 (`codek`/`mock-local`/`openai experimental`)
- DEPLOYMENT의 config/env 표면
- capability matrix의 core/experimental 분류
- charter의 retained surface

## 10. Release candidate 조건

v1 RC는 아래를 만족해야 한다.

1. canonical `run` path 정착
2. false success 금지
3. persisted state semantics 정착
4. tool surface와 report artifact 정착
5. exit code taxonomy 정착
6. experimental provider gating 정착
7. README/DEPLOYMENT/charter/capability matrix 정합화 완료

## 11. Operator triage 절차

### 증상: startup이 바로 실패

- stderr prefix로 `parse/config/release/runtime-init` 중 어디인지 먼저 판별

### 증상: 실행은 되었지만 report가 비정상

- `.axonrunner/artifacts/*`에서 `verify`와 `report`를 먼저 확인

### 증상: write가 거부됨

- persisted state가 `read_only` 또는 `halted` 인지 `status`로 확인

### 증상: provider만 blocked

- `status`/`health`의 provider detail 확인
- `codek` binary 또는 `AXONRUNNER_EXPERIMENTAL_OPENAI`/`OPENAI_API_KEY` 분리

## 12. 핵심 메시지

release의 본질은 기능 수가 아니다.

> `run`/`batch`가 정확히 동작하고,
> 실패가 success처럼 보이지 않으며,
> 상태와 artifact가 결과를 설명해야 한다.

이 세 가지가 무너지면 release하면 안 된다.
