# IMPLEMENTATION-PLAN

## 목표

- PLANNING_GOAL: 정적 코드 검토에서 아직 유효한 release blocker와 contract mismatch를 정리해, 현재 `main` 기준 제품 의미를 깨지 않고 출하 가능성 판단이 가능한 상태로 만든다.
- TARGET_SCOPE: repo

## 완료 조건

- checkpoint / report / trace / snapshot failure path에서 orphan evidence가 남지 않는다.
- verifier command 계약이 plan/parser/executor에서 같은 모델을 사용한다.
- `path_changed` done-condition이 문자열 prefix가 아니라 경로 의미로 판정된다.
- non-Unix `workspace_lock` 보장이 실제 구현으로 닫힌다.
- report/display/docs에 남은 작은 contract drift(`changed_files` 중복, stale bridge/pack 표현 등)가 제거된다.

## 제약

- 현재 `main`의 제품 의미가 기준이다.
- 이미 정리된 surface(`single-agent`, retained CLI, typed done-condition, core-owned workflow pack)는 되돌리지 않는다.
- 기능 확장보다 contract hardening을 우선한다.
- plan 단계에서는 구현하지 않는다.

## 핵심 결정

- Critical path: `commit boundary 정리 → verifier command contract 정리 → path semantics 정리 → cross-platform workspace lock 구현 → report/docs cleanup → release verification`
- Out of scope:
  - multi-agent, orchestrator, 새 CLI surface
  - 장기 memory/platform 확장
  - 미관 위주의 문서 재작성
- 위험:
  - commit integrity 수정 시 run/resume/abort 흐름이 함께 흔들릴 수 있음
  - verifier contract 축소 시 example/fixture/doc 동기화 범위가 넓음

## 결정 게이트

| Gate | Check | Pass Condition | On Fail |
|---|---|---|---|
| G1 | checkpoint lifecycle 방식 | checkpoint 생성이 commit pipeline 안으로 들어가거나, 실패 cleanup이 checkpoint까지 완전 복구 | `run_session`/`run_commit` 구조부터 다시 설계 |
| G2 | verifier contract 방식 | strong verifier path가 structured command 하나의 모델로 고정 | default derivation 범위를 더 좁히고 weak fallback로 강등 |
| G3 | non-Unix lock 정책 | cross-platform stale detection이 구현되고 Unix/non-Unix 모두 같은 operator contract를 가진다 | process probe 전략을 추상화하고 fail-closed fallback을 유지 |

## 구현 순서

1. commit boundary와 cleanup 경로를 정리한다.
2. verifier command contract를 structured model로 통일한다.
3. `path_changed` 판정을 path-aware 로직으로 바꾼다.
4. `workspace_lock`의 non-Unix 보장을 실제 구현으로 닫고 문서/테스트를 맞춘다.
5. report/display/docs의 잔여 drift를 제거한다.
6. release gate 기준 검증 묶음으로 마감한다.

## 검증 전략

- 명령:
  - `cargo test -p axiomrunner_apps --test release_security_gate`
  - `cargo test -p axiomrunner_apps --test autonomous_eval_corpus`
  - `cargo test -p axiomrunner_apps --test fault_path_suite`
  - `cargo test -p axiomrunner_apps --test nightly_dogfood_contract`
- 파일:
  - `crates/apps/src/run_commit.rs`
  - `crates/apps/src/cli_runtime/run_session.rs`
  - `crates/apps/src/runtime_compose/plan.rs`
  - `crates/apps/src/cli_runtime/lifecycle.rs`
  - `crates/apps/src/workspace_lock.rs`
  - `crates/apps/src/runtime_compose/artifacts.rs`
- 운영 증거:
  - report/trace/checkpoint/rollback failure path 일관성
  - release security gate 통과

## 열린 항목

- 없음. non-Unix stale lock은 구현으로 닫는 방향으로 확정됨.

## 셀프 피드백

- 좋은 점:
  - critical path가 release blocker 순서와 맞다.
  - 이미 정리된 제품 의미를 건드리지 않고 kernel hardening에 집중한다.
- 보완점:
  - `T-001`과 `T-004`는 실패 정리/프로세스 생존 판정이 플랫폼별로 달라서 설계 메모를 먼저 짧게 남기는 편이 안전하다.
  - `T-006`은 단순 테스트 통과가 아니라 orphan evidence가 없는지 artifact/trace까지 같이 보는 종료 기준이 필요하다.
