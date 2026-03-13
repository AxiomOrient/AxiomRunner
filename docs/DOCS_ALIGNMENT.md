# Docs Alignment

## Purpose

이 문서는 `docs/`와 전환 문서 묶음을 함께 읽을 때 생기는 혼선을 줄이기 위한 정렬 문서다.

핵심 원칙은 아래와 같다.

- `docs/*.md`: 현재 저장소가 **지금 보장하는 제품 계약**을 설명한다.
- `docs/transition/README.md`: 전환 문서를 읽는 정식 시작점이다.
- `axonrunner_7674ced_autonomous_review/docs/*.md`: AxonRunner를 **autonomous agent 제품으로 전환하기 위한 분석, 원칙, 단계 계획**을 담은 전환 근거 묶음이다.
- 전환 문서는 현재 제품 계약을 즉시 대체하지 않는다. 현재 문서와 다른 내용이 있으면, 그것은 대부분 **미래 전환 목표**를 뜻한다.

## Inventory

| Path | Role | Status |
|---|---|---|
| `docs/project-charter.md` | 현재 제품 정체성 정의 | current truth |
| `docs/CAPABILITY_MATRIX.md` | 현재 capability / release blocker 정의 | current truth |
| `docs/CODEK_RUNTIME_CONTRACT.md` | `codek` substrate 계약 | current truth |
| `docs/RUNBOOK.md` | 현재 운영 절차 | current truth |
| `docs/VERSIONING.md` | 버전/체인지로그 정책 | current truth |
| `docs/AUTONOMOUS_AGENT_TARGET.md` | 자율 에이전트 목표 상태 브리지 문서 | transition bridge |
| `docs/transition/README.md` | 전환 문서 인덱스 | transition index |
| `axonrunner_7674ced_autonomous_review/docs/01_REVIEW_REPORT.md` | 현재 상태 분석과 갭 설명 | transition analysis |
| `axonrunner_7674ced_autonomous_review/docs/03_PRODUCT_QUALITY_FINDINGS.md` | 제품 품질 리스크 정리 | transition analysis |
| `axonrunner_7674ced_autonomous_review/docs/04_NEXT_PRODUCT_DEFINITION.md` | 목표 제품 정의 | transition guidance |
| `axonrunner_7674ced_autonomous_review/docs/05_ROADMAP.md` | 목표 상태와 단계별 roadmap | transition plan |
| `axonrunner_7674ced_autonomous_review/docs/06_PHASED_IMPLEMENTATION_PLAN.md` | 구현 단계와 수락 기준 | transition plan |
| `axonrunner_7674ced_autonomous_review/docs/07_A_TO_Z_TASKS.md` | backlog/task ledger | execution backlog |
| `axonrunner_7674ced_autonomous_review/docs/08_SELF_REVIEW.md` | 분석 한계와 리스크 메모 | review note |
| `axonrunner_7674ced_autonomous_review/docs/09_REFERENCE_BASIS.md` | 참고 근거 | reference note |

## Confirmed Overlap

중복은 있지만 대부분 해로운 중복은 아니다. 동일 주제를 다른 시간축에서 다룬다.

| Topic | Current docs | Transition docs | Reading rule |
|---|---|---|---|
| 제품 정체성 | `project-charter.md` | `01_REVIEW_REPORT.md`, `05_ROADMAP.md` | charter는 현재 truth, transition docs는 다음 목표 |
| CLI surface | `CAPABILITY_MATRIX.md`, `RUNBOOK.md`, `README.md` | `05_ROADMAP.md`, `06_PHASED_IMPLEMENTATION_PLAN.md` | 현재 surface는 `run <intent-spec>` 기준, transition docs의 `run <goal>`은 future |
| tool/runtime contract | `CAPABILITY_MATRIX.md`, `CODEK_RUNTIME_CONTRACT.md`, `RUNBOOK.md` | `01_REVIEW_REPORT.md`, `06_PHASED_IMPLEMENTATION_PLAN.md` | runtime contract는 현재 docs가 우선 |
| safety / approval / eval | `RUNBOOK.md`, `CAPABILITY_MATRIX.md` | `03_PRODUCT_QUALITY_FINDINGS.md`, `05_ROADMAP.md`, `07_A_TO_Z_TASKS.md` | 현재 docs는 운영 의미, transition docs는 확장 목표 |

## Actual Conflict

아래는 같은 시점의 사실처럼 읽으면 충돌하는 지점이다.

1. 현재 제품 정체성
현재 `docs/project-charter.md`와 `README.md`는 AxonRunner를 `minimal event-sourced CLI runtime`으로 규정한다.
반면 roadmap 문서는 목표 제품을 `goal-oriented autonomous agent`로 규정한다.

2. 현재 CLI contract
현재 `docs/CAPABILITY_MATRIX.md`와 `docs/RUNBOOK.md`는 `run <intent-spec>`, `batch`, legacy alias를 public surface로 설명한다.
반면 transition 문서는 `run <goal>`, `resume`, `abort` 중심의 새 surface를 목표로 둔다.

3. 현재 capability 범위
현재 docs는 `batch`, `health`, legacy alias, persisted fact/state를 core capability로 본다.
반면 transition 문서는 이 표면을 transitional 또는 compatibility path로 밀어내고 goal/run state machine을 중심 truth로 바꾸려 한다.

이 차이는 문서 오류라기보다 **현재 상태와 목표 상태가 섞여 있던 구조 문제**다.

## Resolution

이번 정리에서 적용한 기준:

- `docs/transition/README.md`를 전환 문서의 정식 시작점으로 둔다.
- 현재 계약 문서와 전환 근거 문서를 역할별로 분리한다.
- 이 문서를 기준 인덱스로 삼아, reader가 현재 truth와 future plan을 혼동하지 않도록 한다.

## Recommended Reading Order

현재 제품을 이해하려면:

1. `docs/project-charter.md`
2. `docs/CAPABILITY_MATRIX.md`
3. `docs/CODEK_RUNTIME_CONTRACT.md`
4. `docs/RUNBOOK.md`

자율 에이전트 전환 계획까지 보려면 그 다음에:

1. `docs/transition/README.md`
2. `axonrunner_7674ced_autonomous_review/docs/01_REVIEW_REPORT.md`
3. `axonrunner_7674ced_autonomous_review/docs/04_NEXT_PRODUCT_DEFINITION.md`
4. `axonrunner_7674ced_autonomous_review/docs/05_ROADMAP.md`
5. `axonrunner_7674ced_autonomous_review/docs/06_PHASED_IMPLEMENTATION_PLAN.md`
