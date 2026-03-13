# Docs Alignment

## Purpose

이 문서는 `docs/`와 `docs/transition/`을 함께 읽을 때 생기는 혼선을 줄이기 위한 정렬 문서다.

핵심 원칙:

- `docs/*.md`: current truth
- `docs/transition/*.md`: closure context와 future extension 경계
- historical 표현인 `run <intent-spec>` 와 future draft 표현인 `run <goal>` 은 현재 shipped truth가 아니다

## Inventory

| Path | Role | Status |
|---|---|---|
| `docs/project-charter.md` | 현재 제품 정체성 정의 | current truth |
| `docs/CAPABILITY_MATRIX.md` | 현재 capability / release blocker 정의 | current truth |
| `docs/CODEK_RUNTIME_CONTRACT.md` | `codek` substrate 계약 | current truth |
| `docs/RUNBOOK.md` | 현재 운영 절차 | current truth |
| `docs/VERSIONING.md` | 버전/체인지로그 정책 | current truth |
| `docs/AUTONOMOUS_AGENT_TARGET.md` | 남은 목표 브리지 | transition bridge |
| `docs/transition/README.md` | 전환 문서 인덱스 | transition index |
| `docs/transition/REMAINING_GAPS.md` | 이번 cycle closure note와 다음 backlog 경계 | transition analysis |
| `docs/transition/WORKFLOW_PACK_CONTRACT.md` | workflow pack / adapter 경계 | transition guidance |

## Reading Rule

| Topic | Current docs | Transition docs | Reading rule |
|---|---|---|---|
| 제품 정체성 | `project-charter.md` | `AUTONOMOUS_AGENT_TARGET.md` | charter는 현재 truth, transition docs는 future extension 경계 |
| CLI surface | `CAPABILITY_MATRIX.md`, `RUNBOOK.md`, `README.md` | `AUTONOMOUS_AGENT_TARGET.md` | 현재 shipped truth는 `run <goal-file>` 중심 |
| tool/runtime contract | `CAPABILITY_MATRIX.md`, `CODEK_RUNTIME_CONTRACT.md`, `RUNBOOK.md` | `REMAINING_GAPS.md` | runtime contract는 현재 docs가 우선 |
| safety / approval / eval | `RUNBOOK.md`, `CAPABILITY_MATRIX.md` | `REMAINING_GAPS.md`, `WORKFLOW_PACK_CONTRACT.md` | current docs는 shipped behavior, transition docs는 closure context와 future extension 경계 |

## Historical Conflict

historical docs once treated `run <intent-spec>` as current truth.
future target drafts once used `run <goal>`.
현재 shipped truth는 `run <goal-file>`, `status`, `replay`, `resume`, `abort`다.

## Resolution

- review bundle은 삭제한다
- root docs는 current truth만 설명한다
- `docs/transition/`은 closure context와 다음 backlog 경계 문서로 유지한다

## Recommended Reading Order

현재 제품을 이해하려면:

1. `docs/project-charter.md`
2. `docs/CAPABILITY_MATRIX.md`
3. `docs/CODEK_RUNTIME_CONTRACT.md`
4. `docs/RUNBOOK.md`

closure context와 future extension까지 보려면:

1. `docs/transition/README.md`
2. `docs/transition/REMAINING_GAPS.md`
3. `docs/transition/WORKFLOW_PACK_CONTRACT.md`
