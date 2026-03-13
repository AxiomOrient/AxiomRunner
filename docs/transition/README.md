# Transition Docs

## Purpose

이 디렉터리는 AxonRunner의 **closure context와 다음 backlog 경계**를 읽는 정식 시작점이다.

- `docs/*.md`: current truth
- `docs/transition/*.md`: 현재 truth 이후에 남기는 closure note와 future extension boundary

이전 review bundle은 삭제됐다.
그 review bundle은 current truth를 설명하지 못하므로 더 이상 기준으로 읽지 않는다.

## Reading Order

1. `docs/AUTONOMOUS_AGENT_TARGET.md`
2. `docs/AUTONOMOUS_AGENT_SPEC.md`
3. `docs/DOCS_ALIGNMENT.md`
4. `docs/transition/REMAINING_GAPS.md`
5. `docs/transition/WORKFLOW_PACK_CONTRACT.md`

## Transition Sources

현재 cycle 종료 이후의 전환/확장 근거는 아래 문서가 소유한다.

1. `docs/AUTONOMOUS_AGENT_TARGET.md`
2. `docs/AUTONOMOUS_AGENT_SPEC.md`
3. `docs/transition/REMAINING_GAPS.md`
4. `docs/transition/WORKFLOW_PACK_CONTRACT.md`

## Rules

- 현재 제품 계약은 `README.md`, `docs/project-charter.md`, `docs/CAPABILITY_MATRIX.md`, `docs/RUNBOOK.md`가 소유한다.
- 전환 문서는 현재 계약을 즉시 대체하지 않는다.
- `REMAINING_GAPS.md` 는 active backlog가 아니라 closure note다.
- 삭제된 review bundle은 더 이상 current source of truth가 아니다.
