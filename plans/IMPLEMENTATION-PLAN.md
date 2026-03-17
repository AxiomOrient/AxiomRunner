# IMPLEMENTATION-PLAN

## 기준

- 대상 커밋: `dc3665276ba3e6d1721ef676bbe300fc0f732632`
- 분석 범위:
  - `crates/apps/src/**`
  - `crates/core/src/**`
  - `crates/adapters/src/**`
  - `scripts/**`
  - `docs/**`
  - 루트 `README.md`, `CHANGELOG.md`
  - `crates/apps/tests/**`
- 방법: **실제 파일 정적 분석만 사용**. 실행 결과, 추측, 미래 가정은 포함하지 않음.

---

## 1. 최종 판단

AxiomRunner는 **single-agent / verifier-first / workspace-bound CLI runtime** 방향 자체는 맞다. 다만 현재 상태는 “최고 상태”가 아니다.

핵심 문제는 다음 4개 축으로 모인다.

1. **계약 불일치**
   - goal schema / workflow pack 문서 / generator / runtime evaluator가 서로 같은 의미 체계를 쓰지 않는다.
2. **런타임 커밋 불일치**
   - report / trace / snapshot / memory summary가 하나의 일관된 commit 경계 없이 흩어져 기록된다.
3. **string 기반 의미 체계 과다**
   - done condition evidence, policy reason, verification semantics가 문자열 접두사와 free-form text에 과도하게 의존한다.
4. **문서 truth 경로 과다**
   - `docs/`가 shipped truth라고 명시되어 있지만 bridge docs, playbook, generator, `specs/`가 함께 존재해 의미 체계를 분산시킨다.

따라서 이번 정리는 **기능 추가**가 아니라 아래 순서로 가야 한다.

- **정확성 우선 수리**
- **계약 축소 및 typed contract 도입**
- **문서/스크립트 축소**
- **구조 리팩터링**

또한 사용자 요구에 따라 다음은 명시적으로 **하지 않는다**.

- 호환성 유지용 shim
- migration layer
- orchestrator 방향 확장
- surface 확장

---

## 2. 파일 기반으로 확인된 핵심 문제

### 2.1 goal / done-condition / generator / runtime contract 불일치

#### 확인된 사실

- `RunGoal.validate()`는 `done_conditions[].evidence`가 **비어있지 않기만 하면 통과**시킨다.  
  - `crates/core/src/intent.rs:131-142`
- runtime done-condition evaluator는 현재 **오직** `"report artifact exists"`만 지원한다.  
  - `crates/apps/src/cli_runtime/lifecycle.rs:332-365`
- generator는 brief의 `acceptance[]` 텍스트를 그대로 `done_conditions[].evidence`로 넣는다.  
  - `scripts/generate_goal_stack.py:81-90`
- 문서는 done-condition evidence를 file existence / command / changed-path summary / replayable patch / report summary 등 **넓은 범위**로 설명한다.  
  - `docs/WORKFLOW_PACK_CONTRACT.md:44-71`
- RUNBOOK과 GOAL_STACK_PLAYBOOK은 generator를 **권장 경로**로 노출한다.  
  - `docs/RUNBOOK.md:52-62`
  - `docs/GOAL_STACK_PLAYBOOK.md:85-104`

#### 의미

현재는 **유효한 goal + 문서상 맞는 evidence + 공식 generator 출력**이 runtime에서 그대로 실패할 수 있다.

#### 계획 결정

- free-form evidence를 유지하지 않는다.
- `DoneConditionEvidence`를 typed vocabulary로 축소한다.
- initial v1 범위는 아래처럼 좁힌다.
  - `report_artifact_exists`
  - `file_exists`
  - `path_changed`
  - `command_exit_zero`
- generator는 이 typed vocabulary만 생성하게 바꾼다.
- 문서도 이 vocabulary만 truth로 남긴다.

---

### 2.2 `on-risk`는 현재 기능이 아니라 placeholder

#### 확인된 사실

- `RunApprovalMode::OnRisk`는 주석상 **현재 `Always`와 동일**하다.  
  - `crates/core/src/intent.rs:4-11`
- runtime도 `OnRisk | Always`를 동일하게 pre-execution approval로 처리한다.  
  - `crates/apps/src/cli_runtime/lifecycle.rs:494-498`
  - `crates/apps/src/cli_runtime/lifecycle.rs:545-548`
- 문서는 `on-risk`를 실제 high-risk 구분 모드처럼 설명한다.  
  - `docs/WORKFLOW_PACK_CONTRACT.md:83-91`
  - `docs/AUTONOMOUS_AGENT_SPEC.md` 관련 approval section

#### 의미

표면 계약이 실제 동작보다 강하다. 이는 operator 기대를 오도한다.

#### 계획 결정

호환성 요구가 없으므로 **가장 깔끔한 선택은 `on-risk`를 지금 제거**하는 것이다.

- `RunApprovalMode`를 `Never | Always`로 축소
- pack / docs / examples / generator에서 `on-risk` 제거
- 나중에 risk classifier가 실제 구현되면 새 기능으로 재도입

대안으로 risk classifier를 바로 구현할 수도 있으나, 현재 코드/문서 상태를 기준으로는 **제거가 더 정확하고 더 작다**.

---

### 2.3 step journal이 plan 길이와 불일치

#### 확인된 사실

- `build_step_journal()`은 `plan.steps[0]`, `[1]`, `[2]`만 사용한다.  
  - `crates/apps/src/cli_runtime/lifecycle.rs:586-630`
- planner는 goal/verification 구성에 따라 3 step 이상 계획할 수 있다.  
  - `crates/apps/src/runtime_compose/plan.rs` 전체 계획 생성 로직

#### 의미

실제 plan이 4 step 이상일 때 replay/status/report의 step journal이 **잘리거나 왜곡**된다.

#### 계획 결정

- step journal을 `plan.steps.iter()` 기반으로 전면 재작성
- terminal summary step을 “세 번째 step”에 덮어쓰지 말고, 실제 마지막 단계 또는 별도 terminal record로 표현
- step count를 tests에서 고정 검증

---

### 2.4 trace / report / snapshot / memory summary commit 경계 부재

#### 확인된 사실

- `execute_intent()`는 report 작성 후 trace를 append하고, 그 뒤에 memory summary와 snapshot persist를 수행한다.  
  - `crates/apps/src/cli_runtime.rs:293-389`
- `remember_run_summary()` 실패 시 snapshot은 복원하지만, 이미 기록된 report/trace side effect는 되돌리지 못한다.  
  - `crates/apps/src/cli_runtime.rs:340-367`
- `resume` / `abort`도 report → trace → snapshot 순으로 진행한다.  
  - `crates/apps/src/cli_runtime.rs:438-487`
  - `crates/apps/src/cli_runtime.rs:532-553`
- `resume` / `abort`에는 `remember_run_summary()`가 없다.  
  - 같은 파일 구간 확인

#### 의미

현재는 하나의 run finalization이 **원자적이지 않다**. 일부 파일만 남고 일부는 실패하는 partial commit이 가능하다.

#### 계획 결정

런타임을 `RunCommit` 중심으로 재편한다.

### 설계 원칙

- commit-critical state는 아래 3개다.
  1. state snapshot
  2. trace event
  3. report/rollback artifacts
- memory summary는 retained surface가 아니므로 **commit blocker에서 제외**한다.
  - 실패 시 run 자체를 실패로 되돌리지 말고 warning/health로 승격
- `run / resume / abort`는 공통 finalization pipeline을 사용한다.

### 구현 방향

- `crates/apps/src/run_commit.rs` 신설
- `PreparedRunCommit` 구조체 도입
  - finalized record
  - trace payload
  - report payload
  - rollback payload
  - pending snapshot payload
- sequence
  1. 모든 payload를 메모리에서 준비
  2. report/rollback temp write
  3. snapshot temp write
  4. trace append
  5. temp rename / finalize
  6. memory summary best-effort
- failure 시 snapshot rollback이 아니라 **prepared commit 자체를 버리고 trace를 남기지 않는 방식**으로 정리

---

### 2.5 reason code protocol 불일치

#### 확인된 사실

- blocked policy outcome은 `policy=<code>` 형식으로 reason을 만든다.  
  - `crates/apps/src/cli_runtime/lifecycle.rs:486-490`
- `run_reason_code()` / `run_reason_detail()`은 `blocked_by_policy=<code>`를 기대한다.  
  - `crates/apps/src/runtime_compose.rs:1473-1535`

#### 의미

operator-facing reason_code / reason_detail 파생이 일관되지 않다.

#### 계획 결정

문자열 접두사 프로토콜을 폐기한다.

- `RunReason { code, detail }` 구조체 도입
- record 저장 시 structured value를 저장
- 문자열 rendering은 가장 마지막 presentation layer에서만 수행

---

### 2.6 trace 최신 상태와 artifact index가 서로 다른 event를 볼 수 있음

#### 확인된 사실

- latest event lookup은 `.rfind()`를 사용한다.  
  - `crates/apps/src/trace_store.rs:291-305`
- artifact index lookup은 `.find()`를 사용한다.  
  - `crates/apps/src/trace_store.rs:316-335`
- replay summary의 `intent_count`는 unique intent 수가 아니라 `events.len()`이다.  
  - `crates/apps/src/trace_store.rs:417-450`

#### 의미

resume/abort/rewrite가 생기면 **최신 run 상태와 오래된 artifact가 섞일 수 있고**, summary 숫자도 operator 의미와 어긋난다.

#### 계획 결정

- `artifact_index_for_*`를 latest event 기준으로 통일
- replay summary는 unique `intent_id` 기준으로 계산
- trace query API를 `latest_by_intent`, `latest_by_run`, `history_by_run`처럼 분리

---

### 2.7 state snapshot fallback 규칙이 위험함

#### 확인된 사실

- primary snapshot read/parse 실패 뒤에도 temp snapshot fallback을 시도한다.  
  - `crates/apps/src/state_store.rs:73-90`
- missing-file 판별은 `ErrorKind`가 아니라 문자열 포함 검사다.  
  - `crates/apps/src/state_store.rs:151-159`

#### 의미

primary snapshot이 **깨졌는데도** stale temp snapshot이 조용히 채택될 수 있다.

#### 계획 결정

- temp fallback은 **primary가 `NotFound`일 때만** 허용
- parse error는 즉시 hard failure
- missing 판별은 `io::ErrorKind::NotFound` 기반으로 변경

---

### 2.8 artifact path root가 잘못 전달됨

#### 확인된 사실

- `tool.rs`는 patch/command artifact writer에 `workspace_root: &self.artifact_root`를 넘긴다.  
  - `crates/adapters/src/tool.rs:331-338`
  - `crates/adapters/src/tool.rs:454-462`
  - `crates/adapters/src/tool.rs:559-565`
- `tool_write.rs`는 `target_path.strip_prefix(workspace_root)`로 relative path를 만들려 한다.  
  - `crates/adapters/src/tool_write.rs:153-160`

#### 의미

artifact root와 실제 workspace root가 다를 때 patch artifact의 `target_path`가 절대경로로 새어나갈 수 있다.

#### 계획 결정

- `artifact_root`와 `workspace_root`를 분리해 전달
- artifact schema에서 둘을 명시적으로 분리
- report/replay는 항상 workspace-relative path만 사용

---

### 2.9 workflow pack contract가 실제 enforcement보다 넓다

#### 확인된 사실

- pack struct는 `description`, `planner_hints`, `risk_policy.max_mutating_steps` 등을 가진다.  
  - `crates/adapters/src/contracts.rs:277-287`
- validate는 `description`, `planner_hints`, `max_mutating_steps`를 실질적으로 검증하지 않는다.  
  - `crates/adapters/src/contracts.rs:290-326`
  - `crates/adapters/src/contracts.rs:368-372`
- default plan은 `risk_policy.max_mutating_steps`를 별도 의미가 아니라 goal budget `max_steps` 복사값으로 채운다.  
  - `crates/apps/src/runtime_compose/plan.rs:258-261`
- verifier command parsing은 `split_whitespace()` 기반이고, shell 프로그램 `sh`/`bash`를 허용 대상으로 본다.  
  - `crates/apps/src/runtime_compose/plan.rs:371-406`
  - `crates/apps/src/runtime_compose/plan.rs:455-469`
- 그러나 실제 tool execution layer는 shell program을 deny한다.  
  - `crates/adapters/src/tool.rs` command deny path

#### 의미

workflow pack surface가 “존재는 하지만 실제 의미가 잠기지 않은 필드들”을 많이 포함한다.

#### 계획 결정

**pack contract를 축소**한다.

초기 retained pack surface:

- `pack_id`
- `version`
- `entry_goal`
- `allowed_tools[]`
- `verifier_rules[]`
- `approval_mode` (단, `never|always`만)

정리 대상:

- `planner_hints` → 제거
- `risk_policy.max_mutating_steps` → 제거
- free-form `command_example` → structured `{ program, args[] }`로 변경

추가 구조 변경:

- `WorkflowPackContract`는 adapter 계층이 아니라 **kernel contract**이므로 `crates/core`로 이동

이 이동은 문서의 ownership 정의와 더 잘 맞는다.

---

### 2.10 문서/스크립트 surface가 과도함

#### 확인된 사실

- `docs/README.md`는 shipped truth를 `docs/`와 루트 `README.md`라고 명시한다.  
  - `docs/README.md:1-28`
- 같은 문서는 `AUTONOMOUS_AGENT_TARGET`, `AUTONOMOUS_AGENT_SPEC`을 bridge docs라고 명시한다.  
  - `docs/README.md:16-18`, `21-28`
- GOAL_STACK_PLAYBOOK은 generator를 소개하면서 **절대 로컬 경로** 링크를 포함한다.  
  - `docs/GOAL_STACK_PLAYBOOK.md:85-104`
- RUNBOOK은 현재 broken generator를 onboarding 권장 경로로 노출한다.  
  - `docs/RUNBOOK.md:52-62`
- `specs/` 디렉터리는 별도 대형 spec 집합으로 존재하지만, shipped truth 정의에는 포함되지 않는다.  
  - 루트 트리 확인 + `docs/README.md`

#### 의미

문서 표면이 “현재 truth / bridge / secondary spec / generator playbook”로 분산되어 있다.

#### 계획 결정

### 유지

- 루트 `README.md`
- `docs/README.md`
- `docs/project-charter.md`
- `docs/RUNBOOK.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/WORKFLOW_PACK_CONTRACT.md`
- `docs/PROJECT_STRUCTURE.md`
- `docs/VERSIONING.md`
- `CHANGELOG.md`
- `examples/**`

### 수정

- `docs/GOAL_STACK_PLAYBOOK.md`
  - 절대 로컬 경로 제거
  - generator가 typed evidence를 생성하기 전까지 “권장 경로”에서 제외
- `README.md`, `RUNBOOK.md`
  - generator 권장 표현 축소 또는 제거

### 축소/정리

- `docs/AUTONOMOUS_AGENT_TARGET.md`
- `docs/AUTONOMOUS_AGENT_SPEC.md`

둘 다 bridge 문서이므로 유지가 필요하다면 **한 장짜리 backlog/bridge note**로 합친다.
단, 현재 release test가 이 파일들을 잠그고 있으면 같은 changeset에서 test와 README 링크까지 같이 정리한다.

### 제거/아카이브

- `specs/` 전체

`specs/`는 shipped truth가 아니라 secondary spec set이므로, 유지 시 의미 중복만 늘린다. 남긴다면 `archive/specs/`로 이동해 release truth 바깥으로 명확히 격리한다.

---

## 3. 제품/구조 재설계 제안

### 3.1 제품 정체성

유지할 정체성:

- single-agent
- verifier-first
- local workspace-bound
- retained CLI only
- operator-visible replay/report/status truth

버릴 방향:

- orchestrator
- multi-agent
- generalized autonomy platform
- broad planner framework
- memory platformization

이 판단은 `docs/project-charter.md`의 current product/non-goals와 일치한다.

---

### 3.2 구조 리팩터링 제안

#### A. `WorkflowPackContract`를 `adapters`에서 `core`로 이동

이유:

- workflow pack은 provider/tool substrate가 아니라 product contract다.
- 현재 문서는 kernel이 goal/run/done 의미를 소유한다고 명시한다.
- 따라서 pack contract를 adapters에 두는 것은 ownership 표현이 어긋난다.

#### B. `cli_runtime.rs` 분해

현재 `cli_runtime.rs`는 너무 많은 책임을 가진다.

권장 분해:

- `run_session.rs` — execute / resume / abort orchestration entry
- `run_commit.rs` — prepared commit / atomic finalize
- `pending_run.rs` — waiting_approval control state
- `report_pipeline.rs` — report/rollback artifact rendering and write

#### C. storage 계층 명시화

`trace_store.rs`, `state_store.rs`, artifact write path는 같은 저장 규약 문제를 공유한다.

권장:

- `crates/apps/src/storage/trace.rs`
- `crates/apps/src/storage/state.rs`
- `crates/apps/src/storage/artifacts.rs`
- `crates/apps/src/storage/txn.rs`

#### D. stringly typed semantics 제거

core에 아래 타입을 도입한다.

- `DoneConditionEvidence`
- `RunReason`
- `VerificationStatus`
- `VerifierStrength`

현재는 status/strength/reason/evidence가 대부분 문자열 조합이라 contract drift가 쉽게 발생한다.

---

## 4. 스크립트 최소화 계획

### 유지

- `scripts/nightly_dogfood.sh`
  - RUNBOOK, README, nightly contract test와 직접 연결됨

### 축소 또는 제거

- `scripts/generate_goal_stack.py`

현재 상태에서는 공식 onboarding 경로로 두기 어렵다.

선택지는 2개다.

#### 선택지 1 — 유지

- typed done-condition DSL에 맞게 전면 수정
- docs에서 “supported evidence vocabulary만 생성”한다고 명시
- output schema를 tests로 잠금

#### 선택지 2 — 축소(권장)

- `scripts/`에서 제거하고 `examples/dev/` 또는 `tools/dev/`로 이동
- README/RUNBOOK의 공식 onboarding 경로에서 제외
- static example goal stack만 유지

**현재 요구사항(스크립트 최소화)** 기준으로는 **선택지 2가 더 적합**하다.

---

## 5. 단계별 실행 순서

## Phase 0 — truth 정리와 범위 고정

- `on-risk` 제거 결정
- pack surface 축소 결정
- generator retained 여부 결정
- bridge docs / specs 정리 방침 확정

출력:

- 수정된 `docs/project-charter.md`
- 수정된 `docs/WORKFLOW_PACK_CONTRACT.md`
- 수정된 `docs/README.md`

## Phase 1 — correctness blockers 수리

- done-condition typed contract 도입
- lifecycle evaluator 수정
- step journal 일반화
- policy reason struct 도입
- trace latest/artifact mismatch 수정
- state fallback 수정
- patch target path root 수정

## Phase 2 — runtime commit 재구성

- `RunCommit` 도입
- `run/resume/abort` 공통 finalization pipeline 도입
- memory summary를 best-effort로 격하

## Phase 3 — docs / scripts / repo cleanup

- generator 문서 경로 정리
- GOAL_STACK_PLAYBOOK 절대 경로 제거
- `specs/` archive/remove
- bridge docs 축소 또는 통합
- release truth tests 동기화

## Phase 4 — 구조 리팩터링

- `WorkflowPackContract` 이동
- `cli_runtime.rs` 분해
- storage 모듈화

---

## 6. 완료 기준

다음이 모두 만족되어야 이번 정리가 완료된 것으로 본다.

1. **문서와 런타임이 같은 contract를 말한다.**
2. **generator가 남아 있다면 runtime이 실제로 이해하는 goal만 생성한다.**
3. **`run / resume / abort`가 같은 commit 규약으로 끝난다.**
4. **trace, report, snapshot이 partial commit 없이 정합성을 유지한다.**
5. **workflow pack surface가 실제 enforcement 범위를 넘어서 약속하지 않는다.**
6. **bridge / legacy / secondary spec가 release truth를 흐리지 않는다.**
7. **AxiomRunner는 single-agent verifier-first kernel로 더 명확해지고, orchestrator 방향 흔적이 남지 않는다.**

---

## 7. 이번 정리에서 남겨도 되는 것 / 남기면 안 되는 것

### 남겨도 되는 것

- verifier-first 방향
- workspace-bound safety
- retained CLI surface
- examples 기반 onboarding
- nightly dogfood evidence

### 남기면 안 되는 것

- fake mode (`on-risk`)
- free-form done-condition evidence
- adapter 계층에 놓인 pack contract
- 문자열 접두사 기반 reason protocol
- 공식 onboarding으로 노출되는 broken generator
- shipped truth 밖에서 product를 다시 정의하는 secondary spec set

