# Autonomous Agent Target

## Purpose

이 문서는 AxiomRunner의 **현재 제품 계약**을 바꾸지 않으면서,
남은 목표를 짧게 고정하는 bridge 문서다.

current truth는 아래 문서가 소유한다.

- `README.md`
- `docs/project-charter.md`
- `docs/RUNBOOK.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/WORKFLOW_PACK_CONTRACT.md`
- `docs/PROJECT_STRUCTURE.md`

bridge 문서와 current truth가 다르면 current truth가 release 기준이다.

## Current Product

현재 제품은 이미 goal 중심 CLI runtime이다.

- `run <goal-file>`
- `status [run-id|latest]`
- `replay [run-id|latest]`
- `resume [run-id|latest]`
- `abort [run-id|latest]`
- `doctor [--json]`
- `health`
- `help`

이 retained surface는 넓히지 않는다.

## Next Backlog

큰 방향 전환은 끝났다.
이후 backlog는 새 플랫폼 기능이 아니라 hardening에만 쓴다.

- truth lock: 문서, usage, release gate가 같은 제품을 말하게 맞춘다.
- verifier truth: weak / unresolved / pack_required가 success처럼 보이지 않게 잠근다.
- operator lock: approval, budget, blocked, failed, aborted 이유가 status/replay/report에서 같게 보이게 한다.
- workspace safety: execution workspace, artifact workspace, isolated worktree, rollback evidence를 운영 루프에 묶는다.
- developer automation: representative example pack과 verifier flow를 operator-facing asset으로 고정한다.
- eval / release hardening: autonomous eval, nightly dogfood, release gate를 출하 기준으로 유지한다.

## Required Product Contract

이후 작업에서도 아래 public contract는 계속 잠겨야 한다.

- goal schema
- done condition
- budget
- approval policy
- verification plan
- terminal outcomes: `success`, `blocked`, `budget_exhausted`, `approval_required`, `failed`, `aborted`
- replayable evidence
- trace / report / checkpoint / rollback artifacts

## Non-Negotiable Principles

- single-agent first
- workspace-bound execution
- verify-before-done
- hidden fallback 금지
- operator-visible failure reason
- eval-driven release
- goal / approval / trace vocabulary 유지

## Supporting References

자세한 계약은 아래 문서가 소유한다.

1. `README.md`
2. `docs/WORKFLOW_PACK_CONTRACT.md`
3. `docs/RUNBOOK.md`
4. `docs/CAPABILITY_MATRIX.md`
5. `docs/AUTONOMOUS_AGENT_SPEC.md`
