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

큰 전환은 이미 끝났고, 현재 cycle 기준으로 열린 hardening task는 없다.
이후 작업은 새 backlog에서만 연다.

- richer workflow-pack selection
- richer verifier schema
- finer-grained risk classifier

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

## Transition References

자세한 closure context와 future extension 근거는 아래 문서가 소유한다.

1. `docs/transition/README.md`
2. `docs/transition/REMAINING_GAPS.md`
3. `docs/transition/WORKFLOW_PACK_CONTRACT.md`
