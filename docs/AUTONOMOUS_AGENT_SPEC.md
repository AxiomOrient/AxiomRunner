# Autonomous Agent Spec

## Status

This document describes the canonical goal/run contract that the shipped
AxonRunner product now uses. The public runtime surface is
documented by `README.md`, `docs/project-charter.md`,
`docs/CAPABILITY_MATRIX.md`, and `docs/RUNBOOK.md`.

## Goal Schema

A goal-oriented run must answer all of the following:

- objective: what outcome should be completed
- workspace: which local workspace boundary the run may touch
- constraints: explicit non-goals, compatibility rules, and safety limits
- done condition: observable completion checks
- verification plan: which commands, file checks, or assertions prove completion
- artifact expectations: which trace, report, patch, and summary outputs must exist

## Canonical Core Mapping

The current core contract already has a stable goal shape:

- `RunGoal.summary` -> objective
- `RunGoal.workspace_root` -> workspace boundary
- `RunGoal.constraints[]` -> explicit non-goals and compatibility rules
- `RunGoal.done_conditions[]` -> externally checkable completion rules
- `RunGoal.verification_checks[]` -> concrete verification steps
- `RunGoal.budget` -> step/minute/token budget
- `RunGoal.approval_mode` -> `never` / `on-risk` / `always`

The contract is only valid when all required fields are present and non-empty,
there is at least one done condition, there is at least one verification check,
and every budget dimension is greater than zero.

## Done Condition Schema

A run is complete only when every declared `done condition` has evidence.
Done conditions must be externally checkable and should use one or more of:

- file existence or file content assertions
- build, test, or lint commands
- changed-path summaries
- replayable patch evidence
- operator-readable report summaries

## Budget Schema

Every autonomous run must carry an explicit budget:

- step budget
- wall-clock or minute budget
- token budget

Budget exhaustion must produce a visible terminal outcome rather than a silent stop.

## Approval Policy Schema

Supported approval modes:

- `never`
- `on-risk`
- `always`

`on-risk` applies to operations such as destructive file removal, broad replace,
dangerous command execution, or other actions classified as high risk by policy.
현재 default goal workflow-pack path에서는 risk를 보수적으로 취급하므로,
`on-risk` goal은 실행 전에 approval을 요구한다.

## Run Phases

The target lifecycle is:

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

The public run contract should distinguish:

- `success`
- `blocked`
- `budget_exhausted`
- `approval_required`
- `failed`
- `aborted`

Each terminal outcome must include an operator-visible reason.

## Replayable Evidence Contract

Every run should leave evidence that can be consumed by `status`, `replay`,
`doctor`, and release checks:

- run identifier and step identifiers
- selected workspace or worktree binding
- plan/apply/verify/report artifacts
- changed path summary
- patch digest or excerpt
- verification result
- failure boundary when a run stops unsuccessfully

## CLI Target Surface

The target public surface is:

- `run <goal>`
- `status [run-id|latest]`
- `replay [run-id|latest]`
- `doctor [--json]`
- `resume [run-id|latest]`
- `abort [run-id|latest]`

Legacy single-intent commands may remain only as migration or debug paths once
the goal/run contract becomes the canonical truth.
