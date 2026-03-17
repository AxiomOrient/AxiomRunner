# Autonomous Agent Bridge

이 문서는 current truth를 다시 정의하지 않는 짧은 bridge note다.

현재 release truth:

- `README.md`
- `docs/project-charter.md`
- `docs/RUNBOOK.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/WORKFLOW_PACK_CONTRACT.md`
- `docs/PROJECT_STRUCTURE.md`

bridge 범위:

- 제품 방향을 넓히지 않는다.
- 새 플랫폼 기능을 약속하지 않는다.
- 남은 backlog를 hardening 항목으로만 요약한다.

현재 bridge backlog:

- retained CLI surface 유지: `run <goal>`, `resume`, `abort`
- docs truth lock 유지
- verifier / approval / budget / blocked reason 일관성 유지
- workspace safety와 rollback evidence 유지
- goal / approval / trace vocabulary 유지
- example pack / release gate / nightly dogfood hardening 유지

bridge 문서와 current truth가 다르면 current truth가 우선이다.
