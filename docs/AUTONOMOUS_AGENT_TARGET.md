# Autonomous Agent Target

## Purpose

이 문서는 AxonRunner의 **현재 제품 계약**을 바꾸지 않으면서,
남은 목표가 무엇인지 `docs/` 루트에서 짧게 설명하는 브리지 문서다.

현재 truth는 아래 문서가 소유한다.

- `docs/project-charter.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/CODEK_RUNTIME_CONTRACT.md`
- `docs/RUNBOOK.md`

## Current Product

현재 제품은 이미 `goal-file` 중심 CLI runtime이다.

- `run <goal-file>`
- `status [run-id|latest]`
- `replay [run-id|latest]`
- `resume [run-id|latest]`
- `abort [run-id|latest]`
- compatibility surface: `batch`, `read`, `write`, `remove`, `freeze`, `halt`

## Next Backlog

큰 전환은 이미 끝났지만, 현재 cycle에는 아직 닫아야 할 hardening task가 남아 있다.
현재 backlog는 새 기능 추가가 아니라 제품 의미를 잠그는 일에만 쓴다.

- truth lock: 문서, 테스트, release gate가 같은 제품을 말하게 맞춘다.
- operator lock: approval, budget, resume, abort 의미를 status/replay/report까지 같은 뜻으로 고정한다.
- workspace safety: execution workspace, artifact workspace, isolated worktree, rollback evidence를 잠근다.
- developer automation: representative example pack과 verifier 힌트를 operator-facing asset으로 고정한다.
- eval / release hardening: false-success, degraded provider path, nightly dogfood를 출하 기준에 반영한다.

## Required Product Contract

이후 backlog에서도 아래 public contract는 계속 잠겨야 한다.

- goal schema
- done condition
- budget
- approval policy
- terminal outcomes: `success`, `blocked`, `budget_exhausted`, `approval_required`, `failed`, `aborted`
- replayable evidence

## Non-Negotiable Principles

- single-agent first
- workspace-bound execution
- verify-before-done
- hidden fallback 금지
- operator-visible failure reason
- eval-driven release

## Supporting References

자세한 현재 경계와 future extension 근거는 아래 문서가 소유한다.

1. `docs/README.md`
2. `docs/WORKFLOW_PACK_CONTRACT.md`
3. `docs/CAPABILITY_MATRIX.md`
4. `docs/RUNBOOK.md`
