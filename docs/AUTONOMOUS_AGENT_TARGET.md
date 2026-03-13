# Autonomous Agent Target

## Purpose

이 문서는 AxonRunner의 **현재 제품 계약**을 바꾸지 않으면서, 다음 목표가 무엇인지 `docs/` 루트에서 짧게 설명하는 브리지 문서다.

현재 truth는 여전히 아래 문서가 소유한다.

- `docs/project-charter.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/CODEK_RUNTIME_CONTRACT.md`
- `docs/RUNBOOK.md`

이 문서는 그 다음 단계에서 무엇으로 전환하려는지 요약한다.

## Target Product

다음 목표는 아래 한 문장으로 요약된다.

> 하나의 agent가 하나의 workspace에서 하나의 goal을 끝까지 수행하고, 그 과정과 결과를 trace, replay, report로 설명 가능하게 남기는 제품.

## What Changes From Today

현재 제품은 `intent-spec` 중심 minimal runtime이다.

- `run <intent-spec>`
- `batch [--reset-state] <intent-spec>...`
- legacy alias: `read`, `write`, `remove`, `freeze`, `halt`

목표 제품은 `goal/run` 중심 autonomous execution contract를 갖는다.

- `run <goal>`
- `status [run-id|latest]`
- `replay <run-id|latest>`
- `doctor [--json]`
- `resume [run-id|latest]`
- `abort [run-id|latest]`

즉, 핵심 전환은 “사실 조작 runtime”에서 “목표 완료 agent”로의 이동이다.

## Required Product Contract

목표 상태에서는 아래가 public contract로 잠겨야 한다.

- goal schema: 무엇을 끝내야 하는가
- done condition: 언제 완료인가
- budget: step/time/token 한도
- approval policy: 언제 사용자 승인이 필요한가
- terminal outcomes: `success`, `blocked`, `budget_exhausted`, `approval_required`, `failed`, `aborted`
- replayable evidence: trace, artifacts, changed paths, verification result

## Non-Negotiable Principles

- single-agent first
- workspace-bound execution
- verify-before-done
- hidden fallback 금지
- operator-visible failure reason
- eval-driven release

## Roadmap References

자세한 전환 근거와 계획은 아래 문서가 소유한다.

1. `docs/roadmap/01_CURRENT_STATUS.md`
2. `docs/roadmap/02_AUTONOMOUS_AGENT_MENTAL_MODELS.md`
3. `docs/roadmap/03_AUTONOMOUS_ROADMAP.md`
4. `docs/roadmap/04_PHASED_IMPLEMENTATION_PLAN.md`
5. `docs/roadmap/05_A_TO_Z_TASKS.md`
