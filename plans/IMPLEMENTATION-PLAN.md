# AxonRunner Post-Board Implementation Plan

## 목적

`issue/` 보드에서 다루던 AX-001~AX-015는 종료됐다.
이 문서는 그 이후 남은 작업을 새 범주로 정리한 post-board 실행 계획이다.

핵심 원칙은 세 가지다.

1. 이미 닫힌 제품 계약을 다시 흔들지 않는다.
2. 새로운 기능보다 trace, evidence, reproducibility를 먼저 강화한다.
3. macOS/Linux를 기준 플랫폼으로 우선 완결한다.

## 현재 완료된 기반

- canonical `run/batch/read` 경로
- persisted state snapshot
- provider health probe
- essential tool surface
- timeout/truncation and explicit command cancellation model
- `doctor` / `doctor --json`
- JSONL trace + minimal replay

## 남은 큰 작업 묶음

### Wave A. Patch Evidence Hardening

목표:

- 파일 변경마다 before/after digest와 patch evidence를 남긴다.
- trace가 "무엇이 바뀌었는가"를 artifact 수준에서 설명한다.

완료 조건:

- file write / replace 경로에서 patch artifact metadata가 생성된다.
- replay/doctor가 해당 evidence 위치를 참조할 수 있다.
- regression tests가 stable 하다.

### Wave B. Trace Store Formalization

목표:

- 현재 JSONL trace를 schema-stable storage로 승격한다.
- replay가 append-only event API와 schema evolution 규칙을 갖는다.

완료 조건:

- trace schema versioning 규칙이 코드와 테스트에 고정된다.
- event append/read API가 storage 세부구현과 분리된다.
- legacy JSONL compatibility 전략이 문서화된다.

### Wave C. Validation Corpus Expansion

목표:

- 현재 e2e/regression 묶음을 실제 제품 계약 중심 golden corpus로 올린다.

완료 조건:

- false-success 금지, workspace boundary, provider blocked, persisted control state,
  doctor/replay contract가 독립 시나리오로 고정된다.
- mock-local과 codek blocked path 모두 포함된다.

### Wave D. Runtime Contract Cleanup

목표:

- runtime config와 command verification surface를 더 타입화한다.

완료 조건:

- command allowlist contract가 runtime config 표면과 일치한다.
- search API가 bounded substring/regex semantics로 분리된다.
- macOS/Linux에서 shell-less execution contract가 더 명시적으로 증명된다.

## 비목표

- Windows parity
- daemon/gateway/channel 복원
- browser/composio/delegate 재도입
- multi-memory / RAG 계층 확대

## 실행 순서 권장

1. Wave A
2. Wave B
3. Wave C
4. Wave D

이 순서를 지키면 evidence와 replay를 먼저 강화한 뒤 validation과 contract typing을 닫게 된다.
