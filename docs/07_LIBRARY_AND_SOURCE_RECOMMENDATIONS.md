# 07. Library and Source Recommendations

기준:

- product core를 더 정확하고 단순하게 만드는가
- 현재 AxonRunner 철학과 맞는가
- 복잡성 증가보다 품질 이득이 큰가
- 라이선스/유지보수/버전 pinning이 가능한가

## 1. 즉시 채택 추천

### 1.1 `codex-runtime`

**채택 수준:** Adopt now

**역할:** `coclai` 대체 backend substrate

**이유:**
- published crate가 존재하고, repo/릴리즈가 정리되어 있음
- `Workflow`, `Client/Session`, `AppServer`, hooks, web/artifact 모듈까지 갖춘 다층 API
- local Codex app-server에 대한 typed 접근 제공

**권장 사용법:**
- production canonical path는 `Client/Session`
- `Workflow`는 migration smoke용
- `AppServer`는 advanced compatibility용으로만 제한

**주의:**
- `codex app-server`는 공식 문서상 개발/디버깅용이고 변경 가능성이 있음
- 따라서 version pinning, compatibility guard, mock backend가 필요

### 1.2 `ignore`

**채택 수준:** Adopt now

**역할:** `.gitignore` 존중형 workspace walk

**이유:**
- 직접 디렉터리 순회를 구현하는 것보다 정확하고 빠름
- AxonRunner의 workspace inspect 품질을 즉시 끌어올림

### 1.3 `globset`

**채택 수준:** Adopt now

**역할:** include/exclude/path policy 매칭

**이유:**
- 여러 glob 규칙을 동시에 효율적으로 평가 가능
- product-level path policy에 적합

### 1.4 `tracing` + `tracing-subscriber`

**채택 수준:** Adopt now

**역할:** structured diagnostics

**이유:**
- AxonRunner는 이미 gate/diagnosis 문화가 강하다
- `run`, `doctor`, `replay`를 structured event로 남기기 좋다

### 1.5 `rusqlite_migration`

**채택 수준:** Adopt now

**역할:** trace SQLite schema migration 관리

**이유:**
- 이미 rusqlite를 사용 중이고, migration을 간단하게 붙일 수 있다
- 별도 DB stack 도입 없이 schema evolution을 다룰 수 있다

### 1.6 `miette`

**채택 수준:** Adopt now

**역할:** CLI/operator-facing diagnostic errors

**이유:**
- operator UX가 선명해짐
- doctor/config/replay 실패 메시지를 명확히 표현하기 좋다

### 1.7 `assert_cmd` + `predicates`

**채택 수준:** Adopt now (dev)

**역할:** CLI integration tests

**이유:**
- `run/doctor/replay` 제품 contract를 CLI 관점에서 검증 가능

### 1.8 `insta`

**채택 수준:** Adopt now (dev)

**역할:** final answer, doctor JSON, replay summary snapshot testing

**이유:**
- CLI/output contract drift를 빨리 잡아줌

### 1.9 `proptest`

**채택 수준:** Adopt now (dev, selective)

**역할:** core invariants/property tests

**이유:**
- path normalization, patch application, event reduction 같은 곳에 효과적

## 2. 조건부 채택 추천

### 2.1 `toml_edit`

**채택 수준:** Adopt later

**역할:** 설정 파일 수정 시 주석/순서 보존

**언제 필요한가:**
- AxonRunner가 config file writer/editor 역할까지 맡게 될 때

### 2.2 `axum`

**채택 수준:** Defer

**역할:** 향후 local inspector/replay web UI

**왜 defer인가:**
- v1 core product에 필요하지 않음
- 지금 붙이면 surface만 넓어진다

## 3. 외부 소스코드 차용 추천

### 3.1 `AxiomOrient/codek`

**차용 수준:** 적극 검토

**차용 대상:**
- backend/session wrapper patterns
- compatibility guard 개념
- hook integration patterns
- artifact helper 개념 (`validate_doc_patch`, `apply_doc_patch` 등은 참고 가치 있음)

**주의:**
- AxonRunner의 event/policy/trace contract를 codek API에 종속시키지 말 것

### 3.2 `zeroclaw-labs/zeroclaw`

**차용 수준:** 구조/문서 패턴만 제한 차용

**차용 대상:**
- docs hub structure
- trait-driven contracts
- prompt section builder 개념
- observer/trace 사고방식

**차용 금지에 가까운 것:**
- 넓은 운영체제형 surface 전체
- multi-channel/platform breadth
- hardware/browser/memory 확장 기본 탑재

**라이선스 주의:**
- dual MIT / Apache-2.0 이므로, 실제 코드 복사 시 notice/license 정렬이 필요

## 4. 추천하지 않는 추가

### 4.1 `sqlx`

**판단:** v1에는 비추천

이유:
- 현재 rusqlite + migration으로 충분
- async DB stack을 도입하면 복잡성만 늘기 쉽다

### 4.2 vector DB / embedding stack

**판단:** 비추천

이유:
- 현재 제품 문제는 retrieval이 아니라 task completion reliability다

### 4.3 browser automation stack

**판단:** 비추천

이유:
- 제품 핵심 경로가 아니다
- flakiness가 크다

### 4.4 MCP-first 설계

**판단:** 비추천

이유:
- product core가 안정되기 전 external tool protocol을 먼저 늘리면 실패 표면만 커진다

## 5. 개발 도구 추천

### 5.1 `cargo-deny`

**역할:** dependency policy / advisories / bans / licenses

### 5.2 `cargo-nextest`

**역할:** test runner 안정화 및 속도 향상

### 5.3 `cargo-mutants`

**역할:** test suite가 실제로 버그를 잡는지 검증

## 6. 최종 추천 세트

### 필수 채택 세트

- `codex-runtime`
- `ignore`
- `globset`
- `tracing`
- `tracing-subscriber`
- `rusqlite_migration`
- `miette`
- `assert_cmd`
- `predicates`
- `insta`
- `proptest`

### 보류 세트

- `toml_edit`
- `axum`

### 차용 참고 세트

- `AxiomOrient/codek`
- `zeroclaw-labs/zeroclaw`
