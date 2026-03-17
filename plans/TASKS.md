# TASKS

Tasks:
| ID | Goal | Scope | Verification |
|----|----|----|----|
| R1-001 [done] | confirmed blocker를 red test로 먼저 잠근다 | `crates/apps/tests/e2e_cli.rs`, `crates/apps/tests/fault_path_suite.rs`, `crates/apps/src/run_commit.rs` tests | memory 저장 실패, mismatched workspace done-condition, non-isolated commit failure, pack documentary field drift를 재현하는 테스트가 실제로 red였고 이후 green으로 닫혔다 |
| R1-002 [done] | runtime canonical workspace binding 타입을 도입한다 | `crates/apps/src/runtime_compose.rs`, `crates/apps/src/cli_runtime.rs`, 필요 시 `crates/apps/src/doctor.rs` | `workspace_root()` 메서드가 single source로 doctor/status/lock/plan/report 전 경로에서 사용됨. `goal.workspace_root` 직접 실행 참조 없음. `RuntimeWorkspaceBinding` struct는 미사용으로 제거함 |
| R1-003 [done] | `RunGoal.workspace_root`의 실행 의미를 제거하거나 deprecated 처리한다 | `crates/core/src/intent.rs`, `crates/apps/src/goal_file.rs`, `crates/apps/tests/e2e_cli.rs` fixtures | 실행 경로는 더 이상 `workspace_root`를 신뢰하지 않고, 문서는 compatibility field로 강등됐다 |
| R1-004 [done] | done-condition evaluator를 runtime workspace 기준으로 고친다 | `crates/apps/src/cli_runtime/lifecycle.rs`, `crates/apps/src/cli_runtime/run_session.rs` | `e2e_cli_goal_file_done_condition_uses_runtime_workspace_not_goal_workspace_root`가 green이고 `file_exists`는 `execution.provider_cwd` 기준으로 평가된다 |
| R1-005 [done] | planner/status/replay/report의 workspace 표시를 새 binding으로 통일한다 | `crates/apps/src/runtime_compose/plan.rs`, `crates/apps/src/operator_render.rs`, `crates/apps/src/runtime_compose/artifacts.rs`, `crates/apps/src/storage/trace.rs` | plan/replay/report가 `runtime_workspace` 또는 `execution_workspace`를 직접 노출하고 goal `workspace_root`를 더 이상 실행 기준으로 쓰지 않는다 |
| R1-006 [done] | verifier command contract를 한 모듈로 승격한다 | `crates/apps/src/command_contract.rs`, `crates/adapters/src/tool.rs`, `crates/apps/src/runtime_compose/plan.rs`, `crates/apps/src/goal_file.rs` | parse/planning/execution이 모두 `validate_run_command_spec` 기반의 같은 규칙을 쓴다 |
| R1-007 [done] | program-only allowlist를 explicit command spec로 바꾼다 | `crates/apps/src/command_contract.rs`, `crates/adapters/src/tool.rs`, `crates/apps/src/runtime_compose/plan.rs` | `python -c`는 거부되고, examples/fixtures의 `cargo`, `npm`, `python3 -m`, `git status`, `ls .`, `pwd`는 회귀 없이 통과한다 |
| R1-008 [done] | operator allowlist override를 planner/pack parse까지 전파한다 | `crates/apps/src/runtime_compose.rs`, `crates/apps/src/runtime_compose/plan.rs`, `crates/apps/src/goal_file.rs` | `e2e_cli_runtime_command_allowlist_blocks_goal_verifier_before_execution`가 green이고 runtime planning이 operator allowlist를 다시 검증한다 |
| R1-009 [done] | verifier command matrix 테스트를 추가한다 | `crates/apps/src/goal_file.rs` tests, `crates/adapters/tests/tool.rs`, `crates/apps/tests/e2e_cli.rs` | interpreter inline 거부, safe python module 허용, pack `allowed_tools` drift 거부가 테스트로 잠겼다 |
| R1-010 [done] | `workflow_pack.allowed_tools`의 fate를 결정하고 코드에 반영한다 | `crates/core/src/workflow_pack.rs`, `docs/WORKFLOW_PACK_CONTRACT.md`, `crates/apps/src/runtime_compose/plan.rs` | 필드는 유지하되 verifier rule이 `run_command`를 쓰면 pack도 `run_command`를 허용해야 하도록 집행했다 |
| R1-011 [done] | `run_command`를 read-only verifier 전용으로 고정한다 | `crates/adapters/src/tool.rs`, `crates/apps/src/runtime_compose.rs`, 관련 tests | interpreter inline, destructive install/deploy 류 인자 shape가 차단되고 verifier command만 실행된다 |
| R1-012 [done] | patch artifact schema에 restore payload를 추가한다 | `crates/adapters/src/tool_write.rs`, `crates/apps/src/run_commit.rs` | patch artifact JSON이 `restore_mode`와 `restore_artifact_path`를 담고 cleanup이 이를 읽는다 |
| R1-013 [done] | `file_write` / `replace_in_file`의 full restore를 구현한다 | `crates/adapters/src/tool.rs`, `crates/adapters/src/tool_write.rs`, `crates/apps/src/run_commit.rs` | overwrite/append 대상은 restore file artifact를 남기고 commit 실패 시 원본 내용으로 복원된다 |
| R1-014 [done] | `remove_path`의 restore를 구현한다 | `crates/adapters/src/tool.rs`, `crates/adapters/src/tool_write.rs`, `crates/apps/src/run_commit.rs` | file/dir remove는 restore artifact를 남기고 cleanup 경로가 이를 읽어 복원할 수 있다 |
| R1-015 [done] | commit cleanup가 artifact 삭제가 아니라 workspace restore까지 수행하게 만든다 | `crates/apps/src/run_commit.rs`, 필요 시 `crates/apps/src/runtime_compose/artifacts.rs` | snapshot 실패 시 created file 삭제와 overwritten file 복원이 테스트로 잠겼다 |
| R1-016 [done] | memory summary 저장을 warning이 아닌 transaction write로 올린다 | `crates/apps/src/run_commit.rs`, `crates/apps/src/runtime_compose.rs` | `commit_promotes_memory_store_failure_to_transaction_error`가 green이고 memory failure는 `Err`로 승격된다 |
| R1-017 [done] | trace/state failure 시 memory rollback을 추가한다 | `crates/apps/src/run_commit.rs`, `crates/adapters/src/contracts.rs` usage, memory adapters tests | `commit_removes_written_memory_entry_when_snapshot_save_fails`가 green이고 snapshot 실패 뒤 memory entry가 지워진다 |
| R1-018 [done] | memory failure honesty e2e를 추가한다 | `crates/apps/tests/e2e_cli.rs`, `crates/apps/tests/fault_path_suite.rs` | `e2e_cli_memory_failure_is_runtime_execution_error`가 green이고 commit-stage memory failure가 exit code 6과 stderr로 잠겼다 |
| R1-019 [done] | git workspace에서는 isolated worktree를 기본 실행 모드로 올린다 | `crates/apps/src/runtime_compose.rs`, `README.md`, `docs/RUNBOOK.md` | git repo는 기본 isolation 사용, explicit `0`일 때만 끄도록 바뀌었고 test로 잠겼다 |
| R1-020 [done] | non-git workspace integrity 정책을 구현한다 | `crates/apps/src/runtime_compose.rs`, `crates/apps/src/run_commit.rs`, `docs/CAPABILITY_MATRIX.md` | non-git workspace는 기본 비격리 상태를 유지하고, commit rollback은 restore payload 기반 복구 경로를 사용한다 |
| R1-021 [done] | report/replay/operator wording을 새 failure-honest semantics에 맞춘다 | `crates/apps/src/operator_render.rs`, `crates/apps/src/runtime_compose/artifacts.rs`, `crates/apps/src/storage/trace.rs` | commit-stage report write failure 3곳에 `write_commit_report_failure_trace` 추가. memory/state failure는 기존 `rewrite_failed_commit_visibility` 경로 사용. 모든 commit-stage failure가 trace event로 보존됨 |
| R1-022 [done] | goal schema와 examples/generator를 migration 한다 | `examples/`, `tools/dev/generate_goal_stack.py`, `docs/GOAL_STACK_PLAYBOOK.md` | examples/operator docs는 `workspace_root`를 compatibility field로 설명하고 generator도 이를 필수 입력으로 강제하지 않는다 |
| R1-023 [done] | README/charter/runbook/capability/versioning 문서를 `1.0.0` truth로 다시 잠근다 | `README.md`, `docs/project-charter.md`, `docs/RUNBOOK.md`, `docs/CAPABILITY_MATRIX.md`, `docs/WORKFLOW_PACK_CONTRACT.md`, `docs/VERSIONING.md` | 핵심 operator 문서가 기본 isolation, command spec, deprecated `workspace_root`, `1.0.0` release gate를 가리킨다 |
| R1-024 [done] | release gate tests를 새 `1.0.0` contract로 갱신한다 | `crates/apps/tests/release_security_gate.rs`, `crates/apps/tests/autonomous_eval_corpus.rs`, `crates/apps/tests/nightly_dogfood_contract.rs` | release gate와 nightly/autonomous evidence가 `1.0.0` 버전으로 통과한다 |
| R1-025 [done] | workspace/crate version을 `1.0.0`으로 올린다 | workspace `Cargo.toml`, crate `Cargo.toml`, 필요 시 package docs | workspace version을 `1.0.0`으로 올렸고 core/adapters/apps test target이 그 버전으로 빌드된다 |
| R1-026 [done] | full release evidence bundle과 manual smoke를 남긴다 | 로컬 test run, examples smoke, release checklist 문서 | `docs/RELEASE_EVIDENCE.md` 생성. 234 tests passed, release gate 42 passed, 4개 example schema 검증 완료 |

Open decisions that must be resolved before building:
- [resolved] `RunGoal.workspace_root`는 deprecated compatibility field로 유지하고, 실제 실행 경계는 runtime workspace가 소유한다.
- [resolved] verifier command contract는 adapters의 spec validator를 apps parse/planning/runtime이 재사용하는 구조로 고정했다.
- [resolved] `workflow_pack.allowed_tools`는 제거하지 않고, verifier `run_command`와 실제 enforcement를 연결하는 방향으로 고정했다.
- [resolved] non-git integrity는 restore-payload rollback 경로를 기본으로 지원하는 쪽으로 고정했다.

## 실행 순서

1. `R1-001`로 현재 blocker를 red test로 고정한다.
2. `R1-002`부터 `R1-005`까지로 workspace 의미를 단일화한다.
3. `R1-006`부터 `R1-011`까지로 command contract와 pack surface를 정리한다.
4. `R1-012`부터 `R1-020`까지로 commit transaction을 workspace와 memory까지 확장한다.
5. `R1-021`부터 `R1-026`까지로 docs truth, version, release evidence를 `1.0.0` 기준으로 잠근다.

## 의존 메모

- `R1-004`는 `R1-002`, `R1-003` 이후에 진행한다.
- `R1-007`과 `R1-008`은 `R1-006` 공통 contract가 먼저 필요하다.
- `R1-012`부터 `R1-015`는 `R1-011`의 read-only command 축소와 같이 움직여야 한다.
- `R1-016`과 `R1-017`은 `R1-015` rollback 경계가 먼저 있어야 안전하다.
- `R1-022`와 `R1-023`은 schema/contract 결정이 잠긴 뒤에 일괄 migration 한다.
- `R1-025`는 `R1-024` release gate 갱신 직전에 수행한다.
- `R1-026`은 모든 앞선 task가 green인 상태에서만 실행한다.

## 실행 로그

- `R1-001 [done]`: red test를 추가해 memory failure, workspace done-condition 혼선, pack `allowed_tools` drift, commit dirty file 잔존을 실제로 재현했다.
- `R1-002 [done]`: `workspace_root()` 메서드가 doctor/status/lock/plan/report 전 경로의 single source로 사용됨. `goal.workspace_root` 직접 실행 참조 없음. 미사용 `RuntimeWorkspaceBinding` struct 제거.
- `R1-004 [done]`: done-condition 평가를 `goal.workspace_root`가 아니라 실제 execution workspace 기준으로 바꿨다.
- `R1-003 [done]`: `workspace_root`는 문서에서 deprecated compatibility field로 낮추고, 실행 경로에서는 더 이상 기준으로 쓰지 않게 했다.
- `R1-005 [done]`: plan/replay/report가 같은 execution workspace 의미를 직접 출력하게 맞췄다.
- `R1-006 [done]`: parse/planning/execution이 같은 command spec validator를 쓰도록 맞췄다.
- `R1-007 [done]`: `python -c` 같은 inline interpreter 실행은 차단하고, 저장소가 실제로 쓰는 verifier command 패턴은 유지했다.
- `R1-008 [done]`: runtime planning이 operator allowlist를 다시 검증하도록 넣어 planning/execution drift를 막았다.
- `R1-009 [done]`: goal-file, e2e, adapter 테스트에 command/pack gate 회귀를 추가했다.
- `R1-010 [done]`: `allowed_tools`는 유지하되 verifier `run_command`와 실제로 연결되게 집행했다.
- `R1-011 [done]`: `run_command` 표면을 read-only verifier 중심으로 좁혔다.
- `R1-012 [done]`: patch artifact JSON에 restore metadata를 추가했다.
- `R1-013 [done]`: file overwrite/append는 restore artifact를 남기고 commit cleanup이 이를 사용하게 했다.
- `R1-014 [done]`: file/dir remove를 위한 restore artifact 생성 경로를 넣었다.
- `R1-015 [done]`: commit 실패 시 created file 삭제뿐 아니라 overwritten file 복원도 테스트로 잠갔다.
- `R1-016 [done]`: memory summary 저장 실패를 warning이 아니라 commit failure로 승격했다.
- `R1-017 [done]`: snapshot 실패 시 trace cleanup 뒤 memory entry까지 지우도록 닫았다.
- `R1-018 [done]`: CLI 레벨에서 commit-stage memory failure를 실제로 재현하는 e2e를 추가했다.
- `R1-019 [done]`: git repo는 기본 isolation, explicit `0`이면 disable 되도록 바꿨다.
- `R1-020 [done]`: non-git workspace는 기본 비격리 상태를 유지하고 rollback은 restore payload 경로를 사용하도록 기준을 세웠다.
- `R1-021 [done]`: commit-stage report write failure 3 경로에 `write_commit_report_failure_trace` 추가. 모든 commit-stage failure가 trace event로 보존됨.
- `R1-022 [done]`: GOAL_STACK playbook과 generator를 compatibility-field 기준으로 맞췄다.
- `R1-023 [done]`: operator 문서와 versioning 문서를 `1.0.0` truth로 다시 맞췄다.
- `R1-024 [done]`: release gate, autonomous eval, nightly contract를 `1.0.0` 버전 빌드로 다시 통과시켰다.
- `R1-025 [done]`: workspace version을 `1.0.0`으로 올렸다.
- `R1-026 [done]`: `docs/RELEASE_EVIDENCE.md` 생성. 234 tests passed, release gate 42 passed, 4개 example schema 검증.
