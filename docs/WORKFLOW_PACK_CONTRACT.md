# Workflow Pack Contract

이 문서는 goal file 계약, workflow pack 경계, verification/done 규칙을 하나로 고정한다.

AxiomRunner가 `goal`, `run`, `resume`, `abort`, `trace`, `report`, `done`의 의미를 소유한다.
workflow pack과 adapter는 도메인별 실행 수단만 제공하며, 이 의미를 재정의할 수 없다.

---

## 1. Goal Schema

goal-oriented run이 반드시 답해야 하는 항목:

| 필드 | 의미 |
|---|---|
| `RunGoal.summary` | 완료해야 할 목표 (objective) |
| `RunGoal.workspace_root` | run이 접근할 수 있는 로컬 workspace 경계 |
| `RunGoal.constraints[]` | 비목표, 호환 규칙, 안전 제한 |
| `RunGoal.done_conditions[]` | 외부에서 확인 가능한 완료 기준 |
| `RunGoal.verification_checks[]` | 완료를 증명하는 concrete 검증 단계 |
| `RunGoal.budget` | step/minute/token budget |
| `RunGoal.approval_mode` | `never` / `always` |

계약 유효 조건: 모든 required 필드가 존재하고 비어 있지 않아야 하며, done condition이 1개 이상, verification check가 1개 이상, 모든 budget dimension이 0 초과여야 한다.

goal file은 선택적으로 `workflow_pack` 경로를 포함할 수 있다.
pack manifest가 주어지면 AxiomRunner는 먼저 manifest를 읽고 검증한다. manifest가 깨졌으면 fail-closed로 멈춘다.

## 2. Constraint Labels

`constraints[]`는 자유 형식이지만, 아래 4개만 현재 runtime이 강제 적용한다.

| label | detail format | 강제 방식 |
|---|---|---|
| `path_scope` | 쉼표로 구분된 workspace-relative 경로. `workspace` 또는 `.`는 전체 workspace | 범위 밖 경로 접근 시 fail-closed |
| `destructive_commands` | `deny` | `rm`, `mv` 등 destructive class command 차단 |
| `external_commands` | `deny` | allowlist 밖 external command 차단 |
| `approval_escalation` | `required` | risk 판단 시 pre-execution approval 요구 |

그 외 constraint label은 advisory-only다. operator에게 보이지만 실행을 막지 않는다.

`approval_mode=always` goal은 default pack 경로에서 실행 전 approval을 요구한다.

## 3. Done Condition Schema

run이 완료되려면 선언된 모든 done condition에 evidence가 있어야 한다.
현재 v1 supported evidence vocabulary:

- `report_artifact_exists`
- `file_exists:<workspace-relative path>`
- `path_changed:<workspace-relative path>`
- `command_exit_zero:<command string>`

free-form evidence는 허용하지 않는다.

## 4. Verification / Done Relation

`success`는 아래 두 조건이 모두 참일 때만 허용된다.

1. `verification.status == "passed"`
2. 선언된 모든 done condition에 evidence 존재

runtime은 default goal run에서 이 기준을 낮추면 안 된다.

| verification.status | 결과 |
|---|---|
| `passed` | done condition까지 충족하면 `success` |
| `verification_weak` | `blocked` — success로 숨기면 안 됨 |
| `verification_unresolved` | `blocked` — success로 숨기면 안 됨 |
| `pack_required` | `blocked` — success로 숨기면 안 됨 |

`verifier_strength` 필드는 `verification.status`에서 순수 함수로 도출된다. 두 필드는 같은 어휘를 쓴다.

## 5. Budget Schema

모든 자율 실행은 explicit budget이 필요하다.

- step budget: planned_steps + repair_attempts 기준
- wall-clock/minute budget: run 전체 elapsed_ms 기준
- token budget: provider request ceiling 미만이면 pre-execution guard로 즉시 차단

budget 소진은 silent stop이 아니라 `budget_exhausted` terminal outcome으로 노출된다.

## 6. Approval Policy

| approval_mode | 의미 |
|---|---|
| `never` | 승인 불필요 |
| `always` | 항상 실행 전 승인 요구 |

`approval_escalation=required` constraint는 planned verifier command가 high-risk로 분류될 때 추가 pre-execution approval을 요구한다.

## 7. Run Phases

| phase | 의미 |
|---|---|
| `Planning` | plan 생성 |
| `ExecutingStep` | 단계 실행 |
| `Verifying` | verification 실행 |
| `Repairing` | repair 시도 |
| `WaitingApproval` | operator approval 대기 |
| `Blocked` | verification 실패로 차단 |
| `Completed` | 성공 완료 |
| `Failed` | 실패 종료 |
| `Aborted` | operator abort |

## 8. Terminal Outcomes

| outcome | 의미 | operator next action |
|---|---|---|
| `success` | verification passed + done conditions 모두 증거 있음 | report와 replay evidence 확인 |
| `approval_required` | `waiting_approval` 상태 | `resume`으로 승인 후 재개 |
| `budget_exhausted` | step/minute/token budget 소진 | budget 상향 또는 scope 축소 |
| `blocked` | weak verifier 또는 unresolved verification | verifier summary 확인 후 unblock |
| `failed` | provider/tool/workspace 실패 | failure boundary 확인 후 repair |
| `aborted` | operator `abort` 명령 | 필요시 새 run 시작 |

각 terminal outcome은 operator-visible reason을 포함해야 한다.

## 9. Replayable Evidence Contract

모든 run은 `status`, `replay`, `doctor`, release check가 소비할 수 있는 evidence를 남겨야 한다.

| evidence | 경로/필드 |
|---|---|
| run identifier + step identifiers | `run_id`, `step_ids` |
| workspace/worktree binding | `execution_workspace`, checkpoint metadata |
| plan/apply/verify/report artifacts | `.axiomrunner/artifacts/<intent_id>.(plan\|apply\|verify\|report).md` |
| changed path summary | `changed_paths` |
| patch digest + excerpt | `before_digest`, `after_digest`, `before_excerpt`, `after_excerpt`, `unified_diff` |
| verification result | `verification.status`, `verification.summary`, `verification.checks` |
| failure boundary | `first_failure.stage`, `first_failure.message` |

---

## 10. Pack Shape

workflow pack manifest 필수 필드:

- `pack_id`
- `version`
- `entry_goal`
- `recommended_verifier_flow[]`
- `allowed_tools[]`
- `verifier_rules[]`
- `approval_mode`

## 11. Allowed Tools

`allowed_tools[]`는 기존 tool contract 안의 operation만 고를 수 있다.

- `list_files`, `read_file`, `search_files`, `file_write`, `replace_in_file`, `remove_path`, `run_command`

각 항목은 operation 이름과 허용 scope를 함께 가져야 한다.

## 12. Verifier Rules

`verifier_rules[]`는 기존 verifier profile만 쓸 수 있다: `build`, `test`, `lint`, `generic`

각 verifier rule 필수 필드:

| 필드 | 의미 |
|---|---|
| `label` | rule 이름 |
| `profile` | `build` / `test` / `lint` / `generic` |
| `command.program` | 실행 파일 |
| `command.args[]` | 인자 목록 |
| `artifact_expectation` | 기대 artifact |
| `strength` | `strong` / `weak` / `unresolved` / `pack_required` |
| `required` | 필수 여부 |

**strength 의미**:
- `strong` — 직접적인 검증 경로
- `weak` — 약한 fallback probe. success로 숨기면 안 됨
- `unresolved` — 안전한 strong verifier를 만들지 못함
- `pack_required` — 도메인용 explicit pack이 필요

`recommended_verifier_flow[]`는 `build` → `test` → `lint` → `generic` 순서 힌트다. 실제 verifier rule을 대체하지 않는다.

## 13. Approval Mode

pack은 `approval_mode`만 선언할 수 있다.

- `never`
- `always`

추가 risk hint 필드는 retained contract가 아니다.

## 14. Ownership Boundary

**pack이 할 수 있는 것**:
- 허용 도구 범위 축소
- verifier rule 제공
- 도메인별 기본 흐름 제안

**pack이 하면 안 되는 것**:
- 새 terminal outcome 정의
- `status`/`replay` 출력 형식 변경
- `done` 판단 규칙 우회
- adapter마다 다른 resume/abort 의미 정의

**adapter가 소유하는 것**: provider substrate 연결, tool execution backend, memory backend, health probe detail

**adapter가 소유하지 않는 것**: `run`/`resume`/`abort` phase 의미, terminal outcome 의미, `status`/`replay`/`report` schema, verify-before-done rule

## 15. Example Pack

```text
pack_id: rust-service-basic
version: 1
entry_goal: implement one bounded Rust service task
recommended_verifier_flow:
  - build
  - test
  - lint
allowed_tools:
  - run_command within workspace
  - read_file within workspace
verifier_rules:
  - test via command.program=cargo command.args=[test], required=true, strength=strong
  - lint via command.program=cargo command.args=[clippy,--,-D,warnings], required=false, strength=weak
approval_mode: always
```
