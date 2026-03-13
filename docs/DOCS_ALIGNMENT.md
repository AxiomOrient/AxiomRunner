# Docs Alignment

## Purpose

이 문서는 `docs/`와 기존 `roadmap/docs` 문서를 함께 읽을 때 생기는 혼선을 줄이기 위한 정렬 문서다.

핵심 원칙은 아래와 같다.

- `docs/*.md`: 현재 저장소가 **지금 보장하는 제품 계약**을 설명한다.
- `docs/roadmap/*.md`: AxonRunner를 **autonomous agent 제품으로 전환하기 위한 분석, 원칙, 단계 계획**을 설명한다.
- roadmap 문서는 현재 제품 계약을 즉시 대체하지 않는다. 현재 문서와 다른 내용이 있으면, 그것은 대부분 **미래 전환 목표**를 뜻한다.

## Inventory

| Path | Role | Status |
|---|---|---|
| `docs/project-charter.md` | 현재 제품 정체성 정의 | current truth |
| `docs/CAPABILITY_MATRIX.md` | 현재 capability / release blocker 정의 | current truth |
| `docs/CODEK_RUNTIME_CONTRACT.md` | `codek` substrate 계약 | current truth |
| `docs/RUNBOOK.md` | 현재 운영 절차 | current truth |
| `docs/VERSIONING.md` | 버전/체인지로그 정책 | current truth |
| `docs/AUTONOMOUS_AGENT_TARGET.md` | 자율 에이전트 목표 상태 브리지 문서 | transition bridge |
| `docs/roadmap/01_CURRENT_STATUS.md` | 현재 상태 분석과 갭 설명 | transition analysis |
| `docs/roadmap/02_AUTONOMOUS_AGENT_MENTAL_MODELS.md` | 전환 설계 원칙 | transition guidance |
| `docs/roadmap/03_AUTONOMOUS_ROADMAP.md` | 목표 상태와 단계별 roadmap | transition plan |
| `docs/roadmap/04_PHASED_IMPLEMENTATION_PLAN.md` | 구현 단계와 수락 기준 | transition plan |
| `docs/roadmap/05_A_TO_Z_TASKS.md` | backlog/task ledger | execution backlog |
| `docs/roadmap/06_SELF_REVIEW.md` | 분석 한계와 리스크 메모 | review note |
| `docs/roadmap/07_REFERENCE_BASIS.md` | 참고 근거 | reference note |

## Confirmed Overlap

중복은 있지만 대부분 해로운 중복은 아니다. 동일 주제를 다른 시간축에서 다룬다.

| Topic | Current docs | Roadmap docs | Reading rule |
|---|---|---|---|
| 제품 정체성 | `project-charter.md` | `01_CURRENT_STATUS.md`, `03_AUTONOMOUS_ROADMAP.md` | charter는 현재 truth, roadmap은 다음 목표 |
| CLI surface | `CAPABILITY_MATRIX.md`, `RUNBOOK.md`, `README.md` | `03_AUTONOMOUS_ROADMAP.md`, `04_PHASED_IMPLEMENTATION_PLAN.md` | 현재 surface는 `run <intent-spec>` 기준, roadmap의 `run <goal>`은 future |
| tool/runtime contract | `CAPABILITY_MATRIX.md`, `CODEK_RUNTIME_CONTRACT.md`, `RUNBOOK.md` | `01_CURRENT_STATUS.md`, `04_PHASED_IMPLEMENTATION_PLAN.md` | runtime contract는 현재 docs가 우선 |
| safety / approval / eval | `RUNBOOK.md`, `CAPABILITY_MATRIX.md` | `02_AUTONOMOUS_AGENT_MENTAL_MODELS.md`, `03_AUTONOMOUS_ROADMAP.md`, `05_A_TO_Z_TASKS.md` | 현재 docs는 운영 의미, roadmap은 확장 목표 |

## Actual Conflict

아래는 같은 시점의 사실처럼 읽으면 충돌하는 지점이다.

1. 현재 제품 정체성
현재 `docs/project-charter.md`와 `README.md`는 AxonRunner를 `minimal event-sourced CLI runtime`으로 규정한다.
반면 roadmap 문서는 목표 제품을 `goal-oriented autonomous agent`로 규정한다.

2. 현재 CLI contract
현재 `docs/CAPABILITY_MATRIX.md`와 `docs/RUNBOOK.md`는 `run <intent-spec>`, `batch`, legacy alias를 public surface로 설명한다.
반면 roadmap 문서는 `run <goal>`, `resume`, `abort` 중심의 새 surface를 목표로 둔다.

3. 현재 capability 범위
현재 docs는 `batch`, `health`, legacy alias, persisted fact/state를 core capability로 본다.
반면 roadmap은 이 표면을 transitional 또는 compatibility path로 밀어내고 goal/run state machine을 중심 truth로 바꾸려 한다.

이 차이는 문서 오류라기보다 **현재 상태와 목표 상태가 섞여 있던 구조 문제**다.

## Resolution

이번 정리에서 적용한 기준:

- roadmap 핵심 문서를 모두 `docs/roadmap/` 아래로 이동했다.
- 현재 계약 문서와 전환 계획 문서를 같은 `docs` 트리 안에 두되, 역할을 분리했다.
- 이 문서를 기준 인덱스로 삼아, reader가 현재 truth와 future plan을 혼동하지 않도록 했다.

## Recommended Reading Order

현재 제품을 이해하려면:

1. `docs/project-charter.md`
2. `docs/CAPABILITY_MATRIX.md`
3. `docs/CODEK_RUNTIME_CONTRACT.md`
4. `docs/RUNBOOK.md`

자율 에이전트 전환 계획까지 보려면 그 다음에:

1. `docs/roadmap/01_CURRENT_STATUS.md`
2. `docs/roadmap/02_AUTONOMOUS_AGENT_MENTAL_MODELS.md`
3. `docs/roadmap/03_AUTONOMOUS_ROADMAP.md`
4. `docs/roadmap/04_PHASED_IMPLEMENTATION_PLAN.md`
5. `docs/roadmap/05_A_TO_Z_TASKS.md`
