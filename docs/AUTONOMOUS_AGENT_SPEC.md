# Autonomous Agent Spec

## Status

이 문서는 AxiomRunner가 목표로 삼는 canonical goal/run contract를 설명하는 bridge 문서다.
현재 shipped truth는 `README.md`, `docs/project-charter.md`, `docs/RUNBOOK.md`,
`docs/CAPABILITY_MATRIX.md`, `docs/WORKFLOW_PACK_CONTRACT.md`가 우선한다.

## Goal Schema

AxiomRunner의 goal-oriented run은 최소 아래 질문에 답해야 한다.

- objective: 무엇을 끝내야 하는가
- workspace: 어느 local workspace 경계를 만질 수 있는가
- constraints: explicit non-goal, compatibility rule, safety limit
- done condition: 무엇이 완료 증거인가
- verification plan: 어떤 verification이 완료를 증명하는가
- artifacts: 어떤 artifacts가 남아야 하는가

## Constraint Labels

현재 goal schema는 free-form `constraints[]`를 허용하지만,
아래 subset만 enforced subset 대상이다.

- `path_scope`
- `destructive_commands`
- `external_commands`
- `approval_escalation`

### detail 형식

- `path_scope`: comma-separated relative paths. `workspace` 또는 `.`은 whole workspace
- `destructive_commands`: `deny`
- `external_commands`: `deny`
- `approval_escalation`: `required`

위 4개 외 label은 advisory-only다.
advisory-only constraint는 advisory라고 보이게 해야 한다.

## Canonical Core Mapping

현재 core contract와의 매핑은 아래다.

- `RunGoal.summary` -> objective
- `RunGoal.workspace_root` -> workspace boundary
- `RunGoal.constraints[]` -> explicit non-goals and compatibility rules
- `RunGoal.done_conditions[]` -> externally checkable done condition
- `RunGoal.verification_checks[]` -> concrete verification steps
- `RunGoal.budget` -> step/minute/token budget
- `RunGoal.approval_mode` -> `never | on-risk | always`

이 contract는 다음이 모두 참일 때만 유효하다.

- summary non-empty
- workspace_root non-empty
- at least one done condition
- at least one verification check
- every budget dimension > 0

## Done Condition Schema

done condition은 externally checkable해야 한다.
아래 같은 evidence를 써야 한다.

- file existence / file content assertion
- build / test / lint command result
- changed path summary
- replayable patch evidence
- operator-readable report summary

## Verification / Done Relation

`success`는 아래 둘이 모두 참일 때만 허용된다.

- verification status가 `passed`
- every declared done condition이 evidence를 가진다

default goal run도 이 기준을 낮추면 안 된다.

- `verification_weak` => `blocked`
- `verification_unresolved` => `blocked`
- `pack_required` => `blocked`

즉 unresolved verification은 보일 수 있지만 completed success처럼 보이면 안 된다.

## Budget Schema

모든 autonomous run은 명시적 budget를 가져야 한다.

- step budget
- minute budget
- token budget

budget 소진은 silent stop이 아니라 `budget_exhausted` terminal outcome이어야 한다.

## Approval Policy Schema

지원 approval mode:

- `never`
- `on-risk`
- `always`

`on-risk`는 destructive file removal, broad replace, dangerous command execution,
high-risk verifier 같은 작업에 붙는다.

`approval_escalation=required`는 high-risk verifier path에서 pre-execution approval을 요구한다.

## Run Phases

target lifecycle은 아래다.

1. `Planning`
2. `ExecutingStep`
3. `Verifying`
4. `Repairing`
5. `WaitingApproval`
6. `Blocked`
7. `Completed`
8. `Failed`
9. `Aborted`

## Terminal Outcomes

public run contract는 아래 terminal outcomes를 구분해야 한다.

- `success`
- `blocked`
- `budget_exhausted`
- `approval_required`
- `failed`
- `aborted`

각 terminal outcome은 operator-visible reason을 포함해야 한다.

## Replayable Evidence Contract

모든 run은 아래 evidence를 남겨야 한다.

- run identifier와 step identifiers
- selected workspace 또는 worktree binding
- plan / apply / verify / report artifacts
- changed path summary
- patch digest 또는 excerpt
- verification result
- failure boundary
- checkpoint / rollback metadata

## CLI Target Surface

target public surface는 아래다.

- `run <goal>`
- `status [run-id|latest]`
- `replay [run-id|latest]`
- `doctor [--json]`
- `resume [run-id|latest]`
- `abort [run-id|latest]`
- `health`
- `help`

single goal-file commands가 retained runtime surface다.
