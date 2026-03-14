# Docs Guide

AxonRunner 문서는 두 층으로 읽는다.

- current truth: 현재 제품 계약과 운영 기준
- transition docs: 이번 사이클 종료 정리와 다음 확장 경계

과거 review bundle과 임시 분석 문서는 정리됐다.
현재 기준은 이 디렉터리와 루트 `README.md`만 본다.

## Recommended Reading Order

현재 제품만 빠르게 보려면:

1. `docs/project-charter.md`
2. `docs/CAPABILITY_MATRIX.md`
3. `docs/CODEK_RUNTIME_CONTRACT.md`
4. `docs/RUNBOOK.md`

문서 경계까지 같이 보려면:

1. `docs/DOCS_ALIGNMENT.md`
2. `docs/AUTONOMOUS_AGENT_TARGET.md`
3. `docs/AUTONOMOUS_AGENT_SPEC.md`
4. `docs/transition/README.md`

## Current Truth

- `docs/project-charter.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/CODEK_RUNTIME_CONTRACT.md`
- `docs/RUNBOOK.md`
- `docs/VERSIONING.md`

v1 core doc set은 위 5개로 고정한다.

## Transition Docs

- `docs/AUTONOMOUS_AGENT_TARGET.md`
- `docs/AUTONOMOUS_AGENT_SPEC.md`
- `docs/transition/README.md`
- `docs/transition/WORKFLOW_PACK_CONTRACT.md`

## Rules

- 루트 `README.md`는 제품 진입점이다.
- `docs/*.md` 안에서도 bridge나 transition 문서는 explicit하게 표시된 것만 예외다.
- 현재 계약과 전환 문서가 다르면 current truth 문서가 우선이다.
