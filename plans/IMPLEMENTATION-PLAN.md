# IMPLEMENTATION-PLAN

## 범위 계약

- REQUEST: `7037552` 기준 남은 release blocker와 contract mismatch를 코드와 문서 양쪽에서 닫는다.
- TARGET_SCOPE:
  - `crates/apps/src/run_commit.rs`
  - `crates/apps/src/cli_runtime/run_session.rs`
  - `crates/apps/src/cli_runtime/lifecycle.rs`
  - `crates/apps/src/runtime_compose.rs`
  - `crates/apps/src/runtime_compose/plan.rs`
  - `crates/core/src/intent.rs`
  - `crates/core/src/workflow_pack.rs`
  - `crates/apps/src/goal_file.rs`
  - `docs/WORKFLOW_PACK_CONTRACT.md`
  - `docs/RUNBOOK.md`
  - `docs/CAPABILITY_MATRIX.md`
  - `README.md`
- DONE_CONDITION:
  - commit 경계 실패 시 checkpoint/report/rollback/patch/trace/state가 부분 반영되지 않는다.
  - `resume` 실패가 `run`과 같은 수준으로 프로세스 실패로 승격된다.
  - verifier command 계약이 planner, pack validation, executor에서 같은 validator를 쓴다.
  - `file_exists` / `path_changed` evidence는 workspace-relative 규칙을 parse 단계에서 강제한다.
  - 문서 truth가 현재 구현과 다시 일치한다.

## 설계 요약

- `run_commit`은 필요한 파일이 아니라, `run` / `resume` / `abort`가 공통으로 쓰는 persistence boundary다.
  이 경계가 있어야 report, checkpoint, rollback, trace, snapshot, memory summary를 한 곳에서 같은 순서와 같은 rollback 규칙으로 처리할 수 있다.
- 현재 문제는 경계의 존재가 아니라 경계 완결성이다.
  특히 rollback metadata 작성 실패와 trace/state 저장 실패에서 cleanup 대상이 불완전하다.
- `resume`은 문서상 core capability이며 `run`과 같은 failure propagation을 가져야 한다.
  현재는 결과를 출력한 뒤 `Ok(())`로 끝나서 제품 의미와 충돌한다.
- goal evidence와 verifier command는 모두 "workspace-bound single-agent kernel" 원칙을 따라야 한다.
  현재는 타입과 validator가 느슨해서 parse 시점보다 실행 시점에 늦게 거부되는 경로가 남아 있다.

## 구현 순서

1. commit boundary를 transaction 성격으로 다시 묶는다.
2. `resume` failure propagation을 `run`과 대칭으로 맞춘다.
3. verifier command validator를 공용화한다.
4. workspace-relative evidence type을 도입하고 goal parse를 fail-closed로 바꾼다.
5. workflow pack validation을 runtime enforcement 수준으로 끌어올린다.
6. Windows lock recovery와 문서 truth를 하나로 맞춘다.

## 위험과 대응

- commit 정리 중 trace/state rollback 순서가 바뀌면 `replay` 가시성이 깨질 수 있다.
  대응: artifact/trace/state failure path를 각각 단위 테스트로 잠근다.
- verifier validator 공용화 시 기존 goal fixture 일부가 parse 단계에서 실패할 수 있다.
  대응: pack fixture와 examples를 같이 점검하고 weak fallback로 숨기지 않는다.
- workspace-relative type 도입 시 이미 절대경로를 쓰는 테스트/예제가 깨질 수 있다.
  대응: parse error 메시지를 명확히 하고 docs 예시를 같이 갱신한다.

## 열린 결정

- [resolved] Windows stale recovery는 유지하고, 문서 truth를 현재 구현과 같은 의미로 맞췄다.
- [resolved] 공용 verifier validator는 `adapters` 실행 policy를 공용 함수로 노출하고 `apps`가 재사용하는 구조로 고정했다.

## Expanded Atomic Path

- `$scout-boundaries`
- `$plan-what-it-does`
- `$plan-how-to-build`
- `$plan-task-breakdown`

## 셀프 피드백

- 좋은 점:
  - 현재 blocker를 code path, contract path, docs path로 분리해 우선순위를 명확히 잡았다.
  - 기능 추가 없이 retained kernel 의미를 강화하는 방향으로 범위를 제한했다.
- 보완점:
  - verifier command 공용 validator의 소유 크레이트는 구현 전에 한 번 더 결정해야 한다.
  - Windows lock 항목은 코드 문제라기보다 truth lock 문제라서, 구현 전에 제품 기준을 먼저 고르는 편이 안전하다.
