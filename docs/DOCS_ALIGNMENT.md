# Docs Alignment

## Purpose

이 문서는 current docs와 bridge docs를 함께 읽을 때 생기는 혼선을 줄이기 위한 정렬 문서다.

핵심 원칙:

- `docs/*.md`: 기본은 current truth
- `docs/` 루트의 일부 bridge 문서는 future extension 경계를 함께 설명한다
- historical 표현인 `run <intent-spec>` 와 future draft 표현인 `run <goal>` 은 현재 shipped truth가 아니다

## Inventory

| Path | Role | Status |
|---|---|---|
| `docs/project-charter.md` | 현재 제품 정체성 정의 | current truth |
| `docs/README.md` | 문서 읽기 시작점 | current truth |
| `docs/CAPABILITY_MATRIX.md` | 현재 capability / release blocker 정의 | current truth |
| `docs/CODEK_RUNTIME_CONTRACT.md` | `codek` substrate 계약 | current truth |
| `docs/WORKFLOW_PACK_CONTRACT.md` | workflow pack / adapter 경계 | current truth |
| `docs/RUNBOOK.md` | 현재 운영 절차 | current truth |
| `docs/VERSIONING.md` | 버전/체인지로그 정책 | current truth |
| `docs/AUTONOMOUS_AGENT_TARGET.md` | 남은 목표 브리지 | transition bridge |
| `docs/AUTONOMOUS_AGENT_SPEC.md` | goal/run contract 세부 규칙 | bridge doc |

## Reading Rule

| Topic | Current docs | Transition docs | Reading rule |
|---|---|---|---|
| 제품 정체성 | `project-charter.md` | `AUTONOMOUS_AGENT_TARGET.md` | charter는 현재 truth, transition docs는 future extension 경계 |
| CLI surface | `CAPABILITY_MATRIX.md`, `RUNBOOK.md`, `README.md` | `AUTONOMOUS_AGENT_TARGET.md` | 현재 shipped truth는 `run <goal-file>` 중심 |
| tool/runtime contract | `CAPABILITY_MATRIX.md`, `CODEK_RUNTIME_CONTRACT.md`, `RUNBOOK.md`, `WORKFLOW_PACK_CONTRACT.md` | `AUTONOMOUS_AGENT_SPEC.md` | runtime contract는 current docs가 우선 |
| safety / approval / eval | `RUNBOOK.md`, `CAPABILITY_MATRIX.md`, `WORKFLOW_PACK_CONTRACT.md` | `AUTONOMOUS_AGENT_SPEC.md` | current docs는 shipped behavior를 먼저 설명한다 |

## Historical Conflict

historical docs once treated `run <intent-spec>` as current truth.
future target drafts once used `run <goal>`.
현재 shipped truth는 `run <goal-file>`, `status`, `replay`, `resume`, `abort`다.

## Resolution

- review bundle은 삭제한다
- root docs는 current truth만 설명한다
- workflow pack 경계는 `docs/WORKFLOW_PACK_CONTRACT.md`로 흡수한다

## Recommended Reading Order

현재 제품을 이해하려면:

1. `docs/README.md`
2. `docs/project-charter.md`
3. `docs/CAPABILITY_MATRIX.md`
4. `docs/CODEK_RUNTIME_CONTRACT.md`
5. `docs/RUNBOOK.md`

bridge docs까지 보려면:

1. `docs/AUTONOMOUS_AGENT_TARGET.md`
2. `docs/AUTONOMOUS_AGENT_SPEC.md`
3. `docs/WORKFLOW_PACK_CONTRACT.md`
