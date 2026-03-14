# Docs Guide

문서를 처음 읽을 때는 아래처럼 보면 된다.

## 제일 먼저 볼 것

1. `README.md`
2. `docs/project-charter.md`
3. `docs/RUNBOOK.md`
4. `docs/CAPABILITY_MATRIX.md`
5. `docs/PROJECT_STRUCTURE.md`

여기까지 읽으면 이 저장소가 무엇이고, 어떻게 돌리며, 어디를 봐야 하는지 잡힌다.

현재 shipped truth의 시작점은 `run <goal-file>` 중심 CLI다.

## docs 안에서 꼭 필요한 파일

- `docs/project-charter.md` — 제품 정의
- `docs/RUNBOOK.md` — 실행 순서
- `docs/CAPABILITY_MATRIX.md` — 공식 지원 범위
- `docs/CODEK_RUNTIME_CONTRACT.md` — codek runtime 규칙
- `docs/WORKFLOW_PACK_CONTRACT.md` — workflow pack 경계
- `docs/VERSIONING.md` — 버전 정책
- `docs/PROJECT_STRUCTURE.md` — 폴더 구조 설명

이 7개가 기본 문서 세트다.

## 필요할 때만 보는 파일

- `docs/AUTONOMOUS_AGENT_TARGET.md` — 앞으로 가고 싶은 목표
- `docs/AUTONOMOUS_AGENT_SPEC.md` — goal contract 세부 규칙

이 문서들은 보조 문서다. 기본 흐름을 이해한 뒤에 보면 된다.

## 지금 상태 판단

- `docs/`는 이미 꽤 작다.
- 제품 문서는 `docs/` 안에서 current truth와 bridge docs로 나뉜다.
- 그래서 지금 필요한 일은 삭제보다 “입구를 쉽게 만들기”다.

## 규칙

- 루트 `README.md`가 가장 먼저다.
- 현재 제품 설명이 필요하면 `project-charter`, `RUNBOOK`, `CAPABILITY_MATRIX`를 본다.
- 구조가 헷갈리면 `PROJECT_STRUCTURE.md`를 본다.
- 현재 설명과 bridge 문서가 다르면 현재 문서가 우선이다.
