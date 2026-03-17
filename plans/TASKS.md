# TASKS

## 우선순위 기준

- **P0**: 정확성/계약 불일치로 인해 현재 오동작 또는 false contract를 만드는 항목
- **P1**: 문서/스크립트/구조 정리 항목
- **P2**: 품질/성능/구조 개선 항목

---

## P0 — 즉시 수정해야 할 항목

### T-001. `on-risk` 제거

**파일**

- `crates/core/src/intent.rs`
- `crates/apps/src/cli_runtime/lifecycle.rs`
- `crates/apps/src/runtime_compose/plan.rs`
- `docs/WORKFLOW_PACK_CONTRACT.md`
- `docs/RUNBOOK.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/AUTONOMOUS_AGENT_SPEC.md`
- `docs/AUTONOMOUS_AGENT_TARGET.md`
- `examples/**`
- `scripts/generate_goal_stack.py`

**작업**

- `RunApprovalMode`를 `Never | Always`로 축소
- parsing/serialization/docs/examples/generator에서 `on-risk` 제거
- approval 문서와 tests를 동기화

**완료 조건**

- 코드상 `OnRisk`가 완전히 제거됨
- 문서 어디에도 runtime-supported mode처럼 남지 않음

---

### T-002. done-condition을 typed contract로 축소

**파일**

- `crates/core/src/intent.rs`
- `crates/apps/src/cli_runtime/lifecycle.rs`
- `docs/WORKFLOW_PACK_CONTRACT.md`
- `scripts/generate_goal_stack.py`
- 관련 tests

**작업**

- `DoneConditionEvidence` enum 도입
- `RunGoal.validate()`에서 evidence syntax까지 검증
- lifecycle evaluator를 enum 기반으로 전환
- v1 허용 vocabulary 확정
  - `report_artifact_exists`
  - `file_exists`
  - `path_changed`
  - `command_exit_zero`
- free-form evidence 제거

**완료 조건**

- validate 통과 goal은 runtime에서 unsupported evidence로 떨어지지 않음
- 문서 예시와 generator 출력이 동일 vocabulary를 사용함

---

### T-003. generator를 공식 경로에서 내리거나 고친다

**파일**

- `scripts/generate_goal_stack.py`
- `README.md`
- `docs/RUNBOOK.md`
- `docs/GOAL_STACK_PLAYBOOK.md`

**작업**

둘 중 하나를 선택한다.

#### 선택 A — 유지
- generator를 typed done-condition DSL에 맞게 수정
- output schema tests 추가
- docs에서 supported output만 약속

#### 선택 B — 축소(권장)
- 공식 onboarding 경로에서 제거
- dev helper로 격하 또는 이동
- README/RUNBOOK은 static example goal stack만 안내

**완료 조건**

- 공식 문서가 runtime과 맞지 않는 generator를 권장하지 않음

---

### T-004. step journal 하드코딩 제거

**파일**

- `crates/apps/src/cli_runtime/lifecycle.rs`
- 관련 tests

**작업**

- `build_step_journal()`을 `plan.steps.iter()` 기반으로 재작성
- verification/terminal phase mapping을 actual step 길이에 맞게 일반화

**완료 조건**

- 3 step, 4 step, N step plan 모두 journal이 정확히 생성됨

---

### T-005. reason protocol 구조화

**파일**

- `crates/apps/src/cli_runtime/lifecycle.rs`
- `crates/apps/src/runtime_compose.rs`
- report/replay/operator rendering 관련 코드

**작업**

- `RunReason { code, detail }` 도입
- `policy=<...>` / `blocked_by_policy=<...>` 같은 문자열 프로토콜 제거
- rendering layer에서만 문자열화

**완료 조건**

- reason_code / reason_detail derivation이 파싱에 의존하지 않음
- policy blocked case가 일관되게 출력됨

---

### T-006. trace latest / artifact mismatch 수정

**파일**

- `crates/apps/src/trace_store.rs`
- `crates/apps/src/replay.rs`
- 관련 tests

**작업**

- `artifact_index_for_intent()` / `artifact_index_for_run()`를 latest event 기준으로 변경
- `intent_count`를 unique intent 수로 변경
- replay summary 명칭과 계산 의미 일치화

**완료 조건**

- 동일 run/intent의 multiple events가 있어도 latest state와 artifact가 같은 event를 가리킴

---

### T-007. state snapshot fallback 규칙 수정

**파일**

- `crates/apps/src/state_store.rs`
- 관련 tests

**작업**

- temp fallback은 primary `NotFound`일 때만 허용
- parse error / read error는 hard fail
- `is_missing_snapshot_error()` 제거
- `io::ErrorKind::NotFound` 사용

**완료 조건**

- corrupt primary snapshot이 temp snapshot으로 조용히 가려지지 않음

---

### T-008. patch artifact path root 수정

**파일**

- `crates/adapters/src/tool.rs`
- `crates/adapters/src/tool_write.rs`
- artifact/report related tests

**작업**

- writer에 실제 workspace root 전달
- artifact root는 저장 위치 용도, workspace root는 relativization 용도로 분리
- artifact schema에 relative path invariant 추가

**완료 조건**

- patch/command/report가 항상 workspace-relative target path를 기록함

---

### T-009. `run / resume / abort` finalization 공통화

**파일**

- `crates/apps/src/cli_runtime.rs`
- 신규 `crates/apps/src/run_commit.rs`
- 관련 tests

**작업**

- 공통 finalization pipeline 추출
- report / rollback / trace / snapshot 순서를 하나의 API로 통일
- partial commit을 막는 prepared commit 단계 도입

**완료 조건**

- 세 명령 모두 같은 commit 경계를 사용함
- 실패 시 orphan trace/report가 남지 않음

---

### T-010. memory summary를 commit blocker에서 제외

**파일**

- `crates/apps/src/cli_runtime.rs`
- compose_state memory summary 경로
- doctor/health 관련 코드

**작업**

- `remember_run_summary()` 실패가 run commit 자체를 invalidate하지 않게 변경
- 실패는 warning / health signal / operator-visible auxiliary issue로 노출

**완료 조건**

- retained surface truth(run/report/trace/snapshot)가 memory adapter 실패 때문에 partial failure 상태로 꼬이지 않음

---

## P1 — 문서 / 스크립트 / 구조 정리

### T-011. docs truth 재정렬

**파일**

- `docs/README.md`
- `README.md`
- `docs/project-charter.md`
- `docs/RUNBOOK.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/WORKFLOW_PACK_CONTRACT.md`
- `docs/PROJECT_STRUCTURE.md`
- `docs/VERSIONING.md`
- `CHANGELOG.md`

**작업**

- 현재 retained surface, approval modes, pack shape, generator status를 코드와 일치시킴
- truth 문서 간 중복 문구 최소화
- definition-of-product는 charter에, 운영은 RUNBOOK에, contract는 WORKFLOW_PACK_CONTRACT에만 두기

**완료 조건**

- 같은 의미가 여러 문서에서 다르게 서술되지 않음

---

### T-012. GOAL_STACK_PLAYBOOK 정리

**파일**

- `docs/GOAL_STACK_PLAYBOOK.md`

**작업**

- 절대 로컬 경로 제거
- generator가 retained path가 아니면 해당 문서 톤을 dev helper 수준으로 낮춤
- brief → atomic goal 방법은 유지하되 supported DSL만 설명

**완료 조건**

- 로컬 환경 전용 경로나 broken contract 예시가 남지 않음

---

### T-013. bridge docs 축소 또는 통합

**파일**

- `docs/AUTONOMOUS_AGENT_TARGET.md`
- `docs/AUTONOMOUS_AGENT_SPEC.md`
- `README.md`
- release truth tests

**작업**

- 둘 다 유지할 필요가 없다면 1개 bridge note로 통합
- 유지한다면 current truth를 반복 설명하지 말고 backlog/status note로 축소

**완료 조건**

- bridge 문서가 current truth를 재정의하지 않음
- README 링크와 tests가 새 구조를 반영함

---

### T-014. `specs/` 제거 또는 archive 이동

**파일**

- `specs/**`
- `docs/README.md` (필요 시)
- release truth tests (필요 시)

**작업**

- shipped truth가 아닌 secondary specs를 release path 밖으로 이동
- `archive/specs/`로 옮기거나 삭제

**완료 조건**

- product truth가 `docs/` + root README로 수렴됨

---

### T-015. script 최소화

**파일**

- `scripts/`
- `README.md`
- `docs/RUNBOOK.md`
- `docs/PROJECT_STRUCTURE.md`

**작업**

- retained ops script는 `nightly_dogfood.sh`만 남김
- generator는 dev helper면 위치/문서 모두 격하

**완료 조건**

- scripts 디렉터리가 제품 핵심 runtime이 아닌 보조 용도만 가짐

---

### T-016. workflow pack contract 축소

**파일**

- `crates/adapters/src/contracts.rs`
- `crates/apps/src/runtime_compose/plan.rs`
- `docs/WORKFLOW_PACK_CONTRACT.md`
- examples pack files

**작업**

- 아래 필드는 제거 또는 비-authoritative로 격하
  - `planner_hints`
  - `risk_policy.max_mutating_steps`
- `command_example`는 structured command로 교체
- validate가 실제 required fields만 검증하도록 수정

**완료 조건**

- pack 문서가 runtime enforcement 범위를 넘어서 약속하지 않음

---

### T-017. `WorkflowPackContract` 이동

**파일**

- `crates/adapters/src/contracts.rs`
- `crates/core/src/**`
- 호출부 전반
- `docs/PROJECT_STRUCTURE.md`

**작업**

- `WorkflowPackContract`와 관련 타입을 `core`로 이동
- adapter crate는 provider/tool/memory substrate만 남김

**완료 조건**

- pack contract ownership이 문서 정의와 일치함

---

## P2 — 품질 / 성능 / 추가 리팩터링

### T-018. `cli_runtime.rs` 분해

**파일**

- `crates/apps/src/cli_runtime.rs`
- 신규 모듈

**작업**

- orchestration / commit / control / rendering 책임 분리

**완료 조건**

- `cli_runtime.rs`가 entry wiring 수준으로 축소됨

---

### T-019. storage 모듈화

**파일**

- `trace_store.rs`
- `state_store.rs`
- artifact 관련 write path

**작업**

- 저장소 규약을 `storage/` 하위로 모듈화
- trace/state/artifact invariants를 같은 계층에서 관리

**완료 조건**

- 저장 규약이 파일별 ad-hoc 로직이 아니라 하나의 storage layer로 정리됨

---

### T-020. `workspace_lock` portability 정리

**파일**

- `crates/apps/src/workspace_lock.rs`
- docs

**작업**

둘 중 하나 선택

- cross-platform stale PID 확인 구현
- 또는 stale recovery를 Unix-only로 문서화

**완료 조건**

- 현재 구현 가능한 보장만 문서화됨

---

### T-021. `provider_openai` timeout fallback 보정

**파일**

- `crates/adapters/src/provider_openai.rs`

**작업**

- `reqwest::Client::builder()` 실패 시 silent `Client::new()` fallback 대신
  - build error를 명시적으로 surface 하거나
  - timeout이 유지되는 fallback 전략으로 변경

**완료 조건**

- timeout contract가 builder failure에서 사라지지 않음

---

### T-022. `memory_markdown` rewrite 비용 완화

**파일**

- `crates/adapters/src/memory_markdown.rs`

**작업**

- store/delete마다 전체 파일 rewrite하는 구조를 완화
- retained surface가 아니라면 perf 투자 우선순위는 낮게 둠

**완료 조건**

- 대규모 memory data에서 불필요한 full rewrite 비용이 줄어듦

---

### T-023. verifier command parsing 구조화

**파일**

- `crates/apps/src/runtime_compose/plan.rs`
- pack schema

**작업**

- `split_whitespace()` 제거
- structured `{ program, args[] }` contract 사용
- parser에서 shell 허용하고 executor에서 차단하는 모순 제거

**완료 조건**

- verifier command contract와 executor policy가 같은 모델을 사용함

---

## 삭제/정리 후보 요약

### 바로 정리 가능

- `on-risk`
- `planner_hints`
- `risk_policy.max_mutating_steps`
- broken generator 공식 노출
- GOAL_STACK_PLAYBOOK의 절대 로컬 경로
- `specs/`

### 코드 이동 권장

- `WorkflowPackContract` → `crates/core`
- commit/storage logic → `crates/apps/src/storage/*`, `run_commit.rs`

### 남겨야 하는 것

- retained CLI surface
- `scripts/nightly_dogfood.sh`
- examples
- release truth docs

---

## 검증 체크리스트

- [ ] validate 통과 goal이 runtime unsupported evidence로 실패하지 않는다
- [ ] `run / resume / abort`가 같은 commit pipeline을 사용한다
- [ ] trace latest lookup과 artifact lookup이 같은 event를 가리킨다
- [ ] snapshot corrupt 시 temp fallback이 일어나지 않는다
- [ ] patch/report artifact path가 workspace-relative다
- [ ] docs truth와 code contract가 동일하다
- [ ] generator가 남아 있다면 supported DSL만 생성한다
- [ ] `specs/`가 release truth 경로 밖으로 정리되었다
- [ ] AxiomRunner를 orchestrator로 오해하게 만드는 문구가 남아 있지 않다

