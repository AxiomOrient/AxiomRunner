# 08. Validation and Release Runbook

## 1. release를 막는 기준

아래 중 하나라도 실패하면 release 금지.

1. `run` golden corpus fail
2. `doctor` probe fail
3. `replay` fail
4. path boundary regression fail
5. command safety regression fail
6. backend compatibility smoke fail
7. docs/CLI truth mismatch fail
8. dependency/license/security gate fail

## 2. 테스트 계층

### Level 1 — Core deterministic tests

대상:
- reducer
- policy
- event projection
- validation
- path normalization
- patch application

### Level 2 — Contract tests

대상:
- backend contract (`codek`, `mock`)
- trace store
- command runner
- workspace scanner

### Level 3 — CLI integration tests

대상:
- `run`
- `doctor`
- `replay`

### Level 4 — Golden task corpus

대상 예시:
- 저장소 요약
- 특정 파일 수정
- failing test fix
- config update
- 문서 patch
- no-op task
- command verification fail
- path escape attempt

### Level 5 — Opt-in real backend smoke

대상:
- local `codex` binary / codek session open-close
- one simple read-only task
- one controlled write task

## 3. golden task corpus 설계 원칙

각 golden task는 아래를 가져야 한다.

- 입력 task
- workspace fixture
- expected plan shape
- expected file diff
- expected command list
- expected final summary
- expected pass/fail classification

## 4. 권장 명령 게이트

### Local deterministic gate

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

### Product contract gate

```bash
bash scripts/run_product_contract_gate.sh
```

이 스크립트는 최소한 아래를 수행해야 한다.

- core/unit tests
- CLI integration tests
- golden corpus
- replay tests
- docs snapshot sync

### Opt-in backend smoke

```bash
AXONRUNNER_SMOKE_CODEK=1 bash scripts/run_backend_smoke.sh
```

## 5. doctor contract

`doctor`는 아래 항목을 보고해야 한다.

- workspace exists / writable
- config parseable
- backend selected
- `codex` binary present/absent
- backend compatibility pass/fail
- trace store writable
- allowlist command spec loaded
- experimental features enabled/disabled

### JSON mode

```bash
axonrunner doctor --workspace . --json
```

운영 자동화나 로컬 래퍼는 이 JSON만 파싱하면 된다.

## 6. replay contract

`replay`는 아래를 출력해야 한다.

- run metadata
- stage timeline
- files changed
- commands executed
- verification outcome
- final answer snapshot
- primary failure boundary (if failed)

## 7. release rehearsal

### 목적
- 실제 release 전에 문서/게이트/trace/rollback이 연결되는지 확인

### 절차

1. clean checkout
2. full deterministic gate
3. opt-in backend smoke
4. RC artifact 생성
5. replay on sample runs
6. rollback rehearsal
7. report 저장

### 산출물

- `target/product-release/report.json`
- `target/product-release/golden-summary.json`
- `target/product-release/backend-smoke.log`
- `target/product-release/replay-samples/`

## 8. rollback rehearsal

AxonRunner는 deployment platform이 아니더라도, **변경 rollback thought process**는 가져가야 한다.

최소한 다음을 검증한다.

- failed patch run에서 file snapshot과 diff로 원인 추적 가능
- run-level final report만으로 operator가 수동 복구 가능
- trace DB 손상 없이 실패가 남음

## 9. 문서 동기화 게이트

다음은 snapshot 또는 lint로 자동 검증해야 한다.

- README의 CLI usage
- README의 capability table
- doctor sample output
- backend selection 설명
- experimental 기능 목록

## 10. release candidate 조건

v1.0 RC는 아래를 만족해야 한다.

1. `coclai` 흔적 제거 완료
2. `codek` backend canonical path 적용 완료
3. golden task corpus 안정화
4. replay/doctor 완료
5. non-core surface 격리 완료
6. README/DEPLOYMENT/CLI/help 정합화 완료
7. rehearsal/rollback report 생성 완료

## 11. operator triage 절차

### 증상: `run`이 바로 실패
- `doctor` 먼저 실행
- backend/config/workspace 문제 분리

### 증상: 파일 수정은 되었는데 검증 실패
- `replay --run-id` 실행
- command summary와 file patches 확인

### 증상: backend smoke만 실패
- `codex` binary / version / codek compatibility guard 확인
- product logic가 아니라 substrate 문제인지 분리

### 증상: docs와 실제 동작 불일치
- docs snapshot gate 누락 여부 확인
- release 금지

## 12. 핵심 메시지

release의 본질은 기능 수가 아니다.

> `run`이 정확하게 작동하고,
> `doctor`가 즉시 진단하고,
> `replay`가 실패를 설명해야 한다.

이 세 가지가 무너지면 release해서는 안 된다.
