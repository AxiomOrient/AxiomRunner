# Docs

현재 shipped truth는 이 `docs/` 디렉터리와 루트 `README.md`가 소유한다.
임시 분석 메모, review note, ad-hoc 정리 문서는 제품 truth가 아니다.
두 문서가 다르면 `docs/`가 우선이다.
bridge 문서는 현재 truth를 보조하지만, 이를 덮어쓰지 않는다.

`docs/notes/`는 roadmap, audit, review log, process note를 두는 곳이다.
유지할 가치는 있지만 current truth로 잠그면 안 되는 문서는 그쪽으로 보낸다.
`archive/specs/`는 이전 secondary spec 보관 위치다. shipped truth가 아니다.

## 읽는 순서

1. `README.md` — `run <goal-file>` 중심 제품 입구
2. `docs/project-charter.md` — 제품 정의, 아키텍처, retained surface, 원칙
3. `docs/RUNBOOK.md` — 빌드, 실행, 운영, 복구 절차
4. `docs/CAPABILITY_MATRIX.md` — 지원 범위, constraint 강제 여부, release blocker
5. `docs/WORKFLOW_PACK_CONTRACT.md` — goal 스키마, pack 계약, verification/done 규칙
6. `docs/PROJECT_STRUCTURE.md` — 크레이트 구조, 주요 파일, 실행 흐름, codek 계약
7. `docs/GOAL_STACK_PLAYBOOK.md` — brief -> atomic goals 생성 방식
8. `docs/notes/README.md` — roadmap/audit/process note 입구
9. `docs/AUTONOMOUS_AGENT_BRIDGE.md` — bridge note
10. `docs/VERSIONING.md` — versioning / changelog / release gate 규칙

## 규칙

- `project-charter`, `RUNBOOK`, `CAPABILITY_MATRIX`가 현재 제품 surface를 정의한다.
- `WORKFLOW_PACK_CONTRACT`가 goal/pack/verification 계약 본문이다. transition mirror 없음.
- `PROJECT_STRUCTURE`가 crate 경계와 provider 계약을 소유한다.
- `docs/notes/`는 참고 메모다. shipped truth가 아니며 release 기준이 아니다.
- `archive/specs/`는 보관용 spec이다. shipped truth가 아니다.
- `AUTONOMOUS_AGENT_BRIDGE`는 bridge 문서다.
- bridge 문서와 current truth가 다르면 current truth가 release 기준이다.
- transition mirror는 두지 않는다.
