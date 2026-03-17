# TASKS

Tasks:
| ID | Goal | Scope | Verification |
|----|----|----|----|
| P-001 [done] | `run_commit` cleanup 경계를 완전한 commit boundary로 닫기 | `crates/apps/src/run_commit.rs`, 필요 시 `crates/apps/src/runtime_compose/artifacts.rs` | rollback metadata write 실패, trace append 실패, snapshot save 실패 테스트에서 orphan artifact/checkpoint/rollback/trace가 남지 않는다 |
| P-002 [done] | `resume` 실패를 `run`과 같은 프로세스 실패로 승격 | `crates/apps/src/cli_runtime/run_session.rs` | resume 경로에서 provider/tool failure와 terminal failed/aborted outcome이 `Err`로 반환되고 기존 pending state 처리도 일관된다 |
| P-003 [done] | verifier command 계약을 planner/pack validation/executor에서 공용화 | `crates/apps/src/runtime_compose/plan.rs`, `crates/apps/src/runtime_compose.rs`, `crates/core/src/workflow_pack.rs`, 필요 시 `crates/adapters/src/tool.rs` | `pytest`, `pnpm`, `uv`, shell program, allowlist 밖 명령의 허용/거부가 parse 단계와 실행 단계에서 같은 결과를 낸다 |
| P-004 [done] | done-condition evidence를 workspace-relative 타입으로 강제 | `crates/core/src/intent.rs`, `crates/apps/src/goal_file.rs`, `crates/apps/src/cli_runtime/lifecycle.rs` | 절대경로와 `..` 세그먼트는 goal parse에서 거부되고, `file_exists`/`path_changed`는 workspace 내부 경로만 검사한다 |
| P-005 [done] | workflow pack validation을 runtime enforcement 수준으로 강화 | `crates/core/src/workflow_pack.rs`, `docs/WORKFLOW_PACK_CONTRACT.md` | invalid tool operation, invalid approval mode, 비허용 verifier command가 manifest load 단계에서 fail-closed 된다 |
| P-006 [done] | lock 정책과 문서 truth를 일치시킨다 | `crates/apps/src/workspace_lock.rs`, `docs/RUNBOOK.md`, `docs/CAPABILITY_MATRIX.md`, `README.md` | Windows/non-Unix stale recovery 정책이 코드, 문서, 테스트에서 같은 의미를 가진다 |
| P-007 [done] | release truth 회귀를 막는 테스트와 문서 증거를 보강 | 관련 unit/e2e tests, `docs/project-charter.md` 검토 | release gate 핵심 경로가 새 contract에 맞게 잠기고, README/RUNBOOK/CAPABILITY_MATRIX/WORKFLOW_PACK_CONTRACT가 같은 surface를 가리킨다 |

Open decisions that must be resolved before building:
- [resolved] Windows stale recovery는 제품 기준으로 유지하고 docs/tests를 현재 구현에 맞춰 동기화했다.
- [resolved] verifier validator는 `adapters` 실행 policy를 공용 함수로 노출하고 `apps`가 재사용하는 쪽으로 고정했다.

## 실행 순서

1. `P-001`로 commit integrity를 먼저 닫는다.
2. `P-002`로 core CLI semantics 불일치를 제거한다.
3. `P-003`과 `P-004`로 command/path contract를 parse 단계까지 끌어올린다.
4. `P-005`와 `P-006`으로 pack/doc truth를 정리한다.
5. `P-007`로 회귀 방지 증거를 남긴다.

## 셀프 피드백

- 좋은 점:
  - 작업 단위가 실제 file ownership 기준으로 쪼개져 바로 구현에 들어갈 수 있다.
  - 우선순위가 `commit > CLI semantics > contract > docs/tests`로 명확하다.
- 보완점:
  - `P-003`과 `P-005`는 같이 움직일 가능성이 커서 구현 시 하나의 change set으로 묶일 수 있다.
  - `P-006`은 코드보다 truth lock 문제라서, 결정 없이는 다시 drift가 생길 수 있다.

## 실행 로그

- `P-001 [done]`: `run_commit`가 report target, report patch evidence, execution patch evidence, verifier command artifact, checkpoint/rollback metadata를 실패 경로마다 정리하도록 수정했다. `run_commit::tests` 3개로 rollback write/trace append/snapshot save 실패를 고정했다.
- `P-002 [done]`: `resume`이 `run`과 같은 실패 승격 규칙을 따르도록 바꿨고, commit 뒤 done-condition 실패까지 state snapshot을 복원하도록 닫았다. `e2e_cli_resume_failure_propagates_and_keeps_pending_state`로 보강했다.
- `P-003 [done]`: `validate_run_command_policy`를 공용화하고 planner/default pack/runtime tool allowlist가 같은 retained command 표면을 보도록 맞췄다.
- `P-004 [done]`: `file_exists`와 `path_changed`를 `WorkspaceRelativePath` 타입으로 바꿔 절대경로와 `..`를 parse 단계에서 차단했다.
- `P-005 [done]`: workflow pack의 `allowed_tools`를 retained vocabulary로 좁히고, shell verifier command를 goal file load 단계에서 fail-closed 하도록 보강했다.
- `P-006 [done]`: lock truth를 “Unix와 Windows stale recovery, 그 외 플랫폼 fail-closed”로 문서에 맞췄다.
- `P-007 [done]`: core/adapters/apps release 성격 테스트 묶음을 다시 돌려 계약 회귀가 없는지 확인했다.
