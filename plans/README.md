# Plans Folder

이 폴더는 **계획 문서**와 **작업 문서**만 두는 곳이다.

넣어도 되는 것:

- `IMPLEMENTATION-PLAN.md`
- `TASKS.md`
- 같은 성격의 계획/작업 문서 쌍

넣지 않는 것:

- 리뷰 문서
- 회의 메모
- 조사 노트
- 회고 문서
- 임시 초안

## 기본 규칙

한 작업 단위는 아래 2개를 한 쌍으로 둔다.

- 구현 계획: `plans/IMPLEMENTATION-PLAN.md`
- 작업 원장: `plans/TASKS.md`

동시에 여러 작업을 굴릴 때만 이름을 분리한다.

- `plans/<topic>-IMPLEMENTATION-PLAN.md`
- `plans/<topic>-TASKS.md`

## `plan-task-breakdown` 스킬과의 연동

이 폴더의 기본 출력 경로는 아래다.

- `PLAN_OUTPUT_PATH=plans/IMPLEMENTATION-PLAN.md`
- `TASKS_OUTPUT_PATH=plans/TASKS.md`

권장 입력 형태:

```text
$plan-task-breakdown
PLANNING_GOAL: [무엇을 끝내야 하는지]
TARGET_SCOPE: [repo|folder|module|artifact]
DONE_CONDITION:
- [관찰 가능한 완료 조건 1]
- [관찰 가능한 완료 조건 2]
CONSTRAINTS:
- [비목표/호환/안전 제약]
```

권장 출력 원칙:

- 계획 문서는 결정, 범위, 검증 기준을 적는다.
- 작업 문서는 실행 순서와 완료 증거를 적는다.
- task id는 한 delivery cycle 안에서 바꾸지 않는다.
- `done when`은 구현 설명이 아니라 관찰 가능한 상태로 적는다.
- `evidence required`는 명령, 파일, 보고서처럼 바로 확인 가능한 형태로 적는다.

## 권장 템플릿

### 1. `IMPLEMENTATION-PLAN.md`

```md
# IMPLEMENTATION-PLAN

## 목표

- PLANNING_GOAL:
- TARGET_SCOPE:

## 완료 조건

- [DONE_CONDITION 1]
- [DONE_CONDITION 2]

## 제약

- [CONSTRAINT 1]
- [CONSTRAINT 2]

## 핵심 결정

- Critical path: [첫 작업 -> ... -> 마지막 작업]
- Out of scope:
- 위험:

## 결정 게이트

| Gate | Check | Pass Condition | On Fail |
|---|---|---|---|
| G1 | [무엇을 확인하는지] | [통과 조건] | [실패 시 조치] |
| G2 | [무엇을 확인하는지] | [통과 조건] | [실패 시 조치] |

## 구현 순서

1. [phase / step]
2. [phase / step]
3. [phase / step]

## 검증 전략

- 명령:
- 파일:
- 운영 증거:

## 열린 항목

- [아직 확정되지 않은 점]
```

### 2. `TASKS.md`

```md
# TASKS

## 작업 표

| TASK_ID | ACTION | DONE_WHEN | EVIDENCE_REQUIRED | DEPENDS_ON |
|---|---|---|---|---|
| T-001 | [해야 할 일] | [관찰 가능한 완료 상태] | [파일/명령/보고서] | - |
| T-002 | [해야 할 일] | [관찰 가능한 완료 상태] | [파일/명령/보고서] | T-001 |
| T-003 | [해야 할 일] | [관찰 가능한 완료 상태] | [파일/명령/보고서] | T-002 |

## 결정 게이트

| Gate | Check | Pass Condition | On Fail |
|---|---|---|---|
| G1 | [검사] | [통과 기준] | [실패 시 조치] |

## 검증 체크리스트

- [ ] [완료 조건 1]
- [ ] [완료 조건 2]
```

## 최적 형태 요약

`plan-task-breakdown`이 가장 잘 맞는 형태는 아래다.

- 계획 문서는 서술형
- 작업 문서는 표 중심
- task row는 `TASK_ID | ACTION | DONE_WHEN | EVIDENCE_REQUIRED | DEPENDS_ON`
- gate는 별도 표로 분리
- 마지막에는 체크리스트로 완료 계약을 다시 적기

즉, 이 폴더의 표준은 다음 2개다.

- `IMPLEMENTATION-PLAN.md`: 왜 이 순서로 가는지
- `TASKS.md`: 무엇을 끝내야 done인지
