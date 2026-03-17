# TASKS

## 작업 표

| TASK_ID | ACTION | DONE_WHEN | EVIDENCE_REQUIRED | DEPENDS_ON |
|---|---|---|---|---|
| T-001 [done] | checkpoint 생성과 cleanup을 `run_commit` 기준으로 재설계 | checkpoint/report/trace/snapshot 실패 경로에서 orphan checkpoint/report/trace가 남지 않는다 | `run_commit.rs`, `run_session.rs`, 관련 tests | - |
| T-002 [done] | verifier command 모델을 structured contract로 통일 | pack rule, default verifier derivation, executor policy가 같은 command model을 사용한다 | `runtime_compose/plan.rs`, pack fixtures/examples, release gate evidence | T-001 |
| T-003 [done] | `path_changed` done-condition을 path-aware 로직으로 교체 | 경로 정규화 기준으로 하위 경로 변경만 통과하고 문자열 prefix false positive가 없다 | `cli_runtime/lifecycle.rs`, patch artifact path handling, tests | T-002 |
| T-004 [done] | non-Unix `workspace_lock` 보장을 구현으로 닫기 | Unix/non-Unix 모두 stale lock 처리 규칙이 코드와 docs/tests에 같이 반영된다 | `workspace_lock.rs`, `RUNBOOK.md`, `CAPABILITY_MATRIX.md`, tests | T-003 |
| T-005 [done] | report/display의 잔여 contract drift를 제거 | `changed_files` 중복, 불필요 fallback, stale 표현이 제거되고 docs와 출력이 맞는다 | `runtime_compose/artifacts.rs`, `display.rs`, docs, release gate | T-004 |
| T-006 [done] | release hardening 검증 묶음을 마감 | release/security/eval/nightly 검증이 모두 통과한다 | 테스트 로그와 release gate 통과 결과 | T-005 |

## 결정 게이트

| Gate | Check | Pass Condition | On Fail |
|---|---|---|---|
| G1 | checkpoint lifecycle | checkpoint가 commit pipeline 안에 있거나 cleanup이 완전하다 | T-001 재설계 유지 |
| G2 | verifier contract | free-form strong path가 사라지고 structured model 하나만 남는다 | default derivation을 weak fallback 중심으로 축소 |
| G3 | non-Unix lock policy | cross-platform stale detection이 구현되고 operator-facing 의미가 하나로 맞는다 | probe 전략을 재설계하고 fail-closed fallback 유지 |

## 검증 체크리스트

- [x] checkpoint / report / trace / snapshot failure path에 orphan evidence가 남지 않는다
- [x] verifier command contract와 executor policy가 같은 모델을 사용한다
- [x] `path_changed`가 path semantics로 판정된다
- [x] non-Unix `workspace_lock` 보장이 실제 구현으로 닫힌다
- [x] report/display/docs drift가 제거된다
- [x] release gate 검증 묶음이 통과한다

## 실행 메모

- `T-001`: checkpoint 생성은 commit 경계 안으로 이동했고, report/trace/state 실패 정리 시 checkpoint metadata까지 같이 정리한다.
- `T-002`: default verifier derivation은 shell 금지와 command class 규칙을 executor와 같이 쓴다.
- `T-003`: `path_changed`는 slash/backslash 정규화 후 path segment 기준으로 판정한다.
- `T-004`: Windows는 `tasklist` 기반 stale PID probe를 쓰고, Unix는 `kill -0` 경로를 유지한다.
- `T-005`: report의 `changed_files` 중복을 제거했고 `ExecutionMode` fallback은 operator-facing unknown 문자열 대신 unreachable branch로 정리했다.
- `T-006`: `cargo test -q` 전체 통과.
