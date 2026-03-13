# 05. Implementation Plan

## Phase 0. Truth Lock

### 목표
README / CHANGELOG / help / charter / deployment / doctor를 하나의 제품면으로 맞춘다.

### 작업
- README에 `doctor` 포함
- README 링크 타깃 수정
- CHANGELOG의 `agent` 제거
- `docs/0*.md`를 archive로 이동하고 현재 source of truth를 분명히 표시
- `doctor` 출력에 현재 provider/version/workspace/state/trace/artifact 정보를 일관되게 노출

### 완료 조건
- 사용자가 README, `--help`, `doctor`, charter를 보면 같은 제품을 이해한다.

## Phase 1. Schema Lock

### 목표
state / display / trace / replay에서 mode naming과 revision 의미를 일관되게 만든다.

### 작업
- `readonly` / `read_only` 중 canonical string 하나로 통일
- snapshot migration 허용
- replay/status/doctor 표시 통일
- revision이 event-count라는 점을 operator-facing 문서에 기록

### 완료 조건
- 신규/기존 snapshot 모두 읽힌다.
- mode 문자열이 어디서나 동일하다.

## Phase 2. Evidence Upgrade

### 목표
자동화 결과를 “metadata”가 아니라 “변경 근거” 수준으로 보여 준다.

### 작업
- write/replace/remove에 대해 patch evidence schema 강화
- 가능한 경우 unified diff 또는 bounded before/after excerpt 추가
- replay/report가 changed paths와 evidence를 직접 요약
- artifact naming과 retention 정책 문서화

### 완료 조건
- operator가 replay만 보고도 어떤 파일이 어떻게 바뀌었는지 1차 판단이 가능하다.

## Phase 3. `codek` Contract Lock

### 목표
`codek`/`codex-runtime`를 제품 운영 계약으로 끌어올린다.

### 작업
- 현재 pinned version과 upstream 권장 버전 차이를 문서화
- compatibility matrix 작성
- `doctor`에 codex binary path / version / compatibility 표시
- unsupported version에서 blocked/degraded 정책을 명시
- session/workspace contract를 문서화

### 완료 조건
- 설치 환경 문제를 `doctor` 하나로 식별할 수 있다.

## Phase 4. Verification Lock

### 목표
계획 문서와 실제 검증 표면을 1:1로 맞춘다.

### 작업
- `plans/TASKS.md` verification 문구와 실제 테스트 파일 구성 정렬
- adapter tool integration tests 존재 여부 확정
- e2e golden corpus를 truth surface / schema / evidence / blocked provider 시나리오로 재구성
- release gate에 문서 drift 검사 추가

### 완료 조건
- “문서상 done”과 “실제 검증 가능”이 동일해진다.

## Phase 5. Runtime Hardening

### 목표
실패 정책과 workspace 경계를 더 단단히 한다.

### 작업
- workspace 미결정 시 fail-closed
- async runtime fallback 정책을 operator-visible로 정리
- `batch --reset-state` 의미를 명시 또는 `--reset-trace`, `--reset-artifacts` 추가 검토
- tool high-risk operations(remove_path, run_command)에 대한 UX/trace 강화

### 완료 조건
- 실패/복구/초기화 semantics가 명확해진다.

## Phase 6. Release Readiness

### 목표
작은 제품이지만 운영 가능한 상태로 잠근다.

### 작업
- install/run/replay/doctor runbook 정리
- release checklist 작성
- example scenario 추가
- changelog / versioning policy 정리

### 완료 조건
- 새 사용자가 clone/build/doctor/run/replay까지 막힘 없이 갈 수 있다.
