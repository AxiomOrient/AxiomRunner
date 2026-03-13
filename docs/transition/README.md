# Transition Docs

## Purpose

이 디렉터리는 AxonRunner의 다음 제품 전환 문서를 읽는 **정식 시작점**이다.

- `docs/*.md`: current truth
- `docs/transition/README.md`: 다음 제품 전환 인덱스

현재 전환 분석의 원본 review bundle은 `axonrunner_7674ced_autonomous_review/docs/`에 있다.
그 묶음은 이번 전환 동안 참고 입력으로 유지하되, 루트 문서가 가리키는
정식 읽기 시작점은 이 문서 하나로 고정한다.

## Reading Order

이 문서를 읽은 다음 순서는 아래를 권장한다.

1. `docs/AUTONOMOUS_AGENT_TARGET.md`
2. `docs/AUTONOMOUS_AGENT_SPEC.md`
3. `docs/DOCS_ALIGNMENT.md`
4. `axonrunner_7674ced_autonomous_review/docs/01_REVIEW_REPORT.md`
5. `axonrunner_7674ced_autonomous_review/docs/05_ROADMAP.md`
6. `docs/transition/WORKFLOW_PACK_CONTRACT.md`

## Transition Source Bundle

다음 전환 분석과 구현 근거는 아래 문서 묶음이 소유한다.

1. `axonrunner_7674ced_autonomous_review/docs/01_REVIEW_REPORT.md`
2. `axonrunner_7674ced_autonomous_review/docs/03_PRODUCT_QUALITY_FINDINGS.md`
3. `axonrunner_7674ced_autonomous_review/docs/04_NEXT_PRODUCT_DEFINITION.md`
4. `axonrunner_7674ced_autonomous_review/docs/05_ROADMAP.md`
5. `axonrunner_7674ced_autonomous_review/docs/06_PHASED_IMPLEMENTATION_PLAN.md`
6. `axonrunner_7674ced_autonomous_review/docs/07_A_TO_Z_TASKS.md`

## Rules

- 현재 제품 계약은 여전히 `README.md`, `docs/project-charter.md`,
  `docs/CAPABILITY_MATRIX.md`, `docs/RUNBOOK.md`가 소유한다.
- 전환 문서는 현재 계약을 즉시 대체하지 않는다.
- 루트 문서에서는 더 이상 삭제된 옛 roadmap 경로를 live reference로 쓰지 않는다.
