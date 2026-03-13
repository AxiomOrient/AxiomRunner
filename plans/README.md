# AxonRunner ded0ab6 Planning Bundle

이 디렉터리는 두 층으로 나뉜다.

## Execution Artifacts

다음 두 파일이 다음 구현 루프의 canonical planning artifact다.

- `IMPLEMENTATION-PLAN.md`
- `TASKS.md`

## Review Inputs

아래 묶음은 실행 계획을 만들기 위한 입력 근거다.

- `docs/01_REVIEW_REPORT.md`
- `docs/02_FILE_BY_FILE_AUDIT.md`
- `docs/03_GAP_CATALOG.md`
- `docs/04_DIRECTION_AND_BLUEPRINT.md`
- `docs/05_IMPLEMENTATION_PLAN.md`
- `docs/06_A_TO_Z_TASKS.md`
- `docs/07_SELF_REVIEW.md`
- `data/tasks.csv`
- `data/tasks.json`

`plans/docs/*`와 `plans/data/*`는 review bundle이며 제품 source of truth가 아니다.
실제 다음 작업의 범위, 순서, 의존성, verification contract는
루트 `plans/IMPLEMENTATION-PLAN.md`와 `plans/TASKS.md`를 따른다.
