# 04. Phased Implementation Plan

## 설계 원칙

1. **single-agent first**
2. **goal-oriented public surface**
3. **event-sourced truth**
4. **workspace-bound action**
5. **verify-before-done**
6. **operator-visible control**
7. **eval-driven release**

---

## Phase P0 — 제품 계약 잠금

### 바꿀 것
- README를 “minimal runtime”에서 “autonomous workspace agent” 계약으로 승격
- `docs/`에 autonomous spec 추가
- `plans/`에 새 roadmap/task 세트 추가
- `release_security_gate`에 새 truth surface 반영

### 주 파일
- `README.md`
- `docs/project-charter.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/RUNBOOK.md`
- `plans/IMPLEMENTATION-PLAN.md`
- `plans/TASKS.md`
- `crates/apps/tests/release_security_gate.rs`

### 핵심 구현
- public CLI contract 정리
- legacy alias의 역할 재정의
- “done”의 정의 문서화

### 수락 기준
- README / charter / capability matrix / runbook / help가 동일 surface를 말한다.

---

## Phase P1 — Domain Remodel

### 바꿀 것
fact/state 중심 의미를 유지하되, 상위 domain을 **goal/run lifecycle**로 교체한다.

### 새 개념
- `RunGoal`
- `RunConstraint`
- `DoneCondition`
- `RunBudget`
- `RunApprovalMode`
- `RunPhase`
- `RunOutcome`
- `RunEvent`

### 주 파일
- `crates/core/src/intent.rs`
- `crates/core/src/event.rs`
- `crates/core/src/effect.rs`
- `crates/core/src/state.rs`
- `crates/core/src/reducer.rs`
- `crates/core/src/projection.rs`
- `crates/core/tests/*`

### 구현 메모
- 기존 fact mutation은 internal effect나 run state payload로 후퇴시킨다.
- 외부 사용자 언어는 `goal` 중심으로 바꾼다.
- compatibility가 필요하면 legacy parser만 둔다.

### 수락 기준
- `core`가 “현재 런이 어떤 상태인가”를 표현할 수 있다.
- state invariant가 run lifecycle 중심으로 다시 정의된다.

---

## Phase P2 — CLI Surface Transition

### 바꿀 것
현재 `run <intent-spec>`를 `run <goal-spec>`로 전환한다.

### 추천 public surface
- `run <goal>`
- `status [run-id|latest]`
- `replay <run-id|latest>`
- `doctor [--json]`
- `resume <run-id|latest>`
- `abort <run-id|latest>`

### legacy surface
- `read/write/remove/freeze/halt` 는 internal debugging 또는 migration alias로만 유지

### 주 파일
- `crates/apps/src/cli_args.rs`
- `crates/apps/src/cli_command.rs`
- `crates/apps/src/display.rs`
- `crates/apps/tests/e2e_cli.rs`

### 수락 기준
- help text와 e2e golden outputs가 새 surface를 기준으로 고정된다.

---

## Phase P3 — Autonomous Loop Harness

### 바꿀 것
현재 `cli_runtime`를 goal loop 중심으로 재구성한다.

### 루프 상태
- `Planning`
- `ExecutingStep`
- `Verifying`
- `Repairing`
- `WaitingApproval`
- `Blocked`
- `Completed`
- `Failed`
- `Aborted`

### 주 파일
- `crates/apps/src/cli_runtime.rs`
- `crates/apps/src/runtime_compose.rs`
- `crates/apps/src/replay.rs`
- `crates/apps/src/status.rs`
- `crates/apps/src/trace_store.rs`

### 구현 메모
- 한 step씩 실행하고 매 step마다 trace를 append
- `done_when` 체크를 verifier가 수행
- verifier fail이면 repair loop
- budget 초과 시 blocked or failed로 종료
- 모든 종료에는 explicit reason이 있어야 함

### 수락 기준
- run이 중간 과정을 추적 가능한 상태로 남긴다.
- false success가 없다.

---

## Phase P4 — codek / codex-runtime Substrate Hardening

### 바꿀 것
`codex-runtime`를 단순 prompt call이 아니라 **run-scoped workspace execution substrate**로 사용한다.

### 주 파일
- `crates/adapters/src/provider_codex_runtime.rs`
- `crates/adapters/src/provider_registry.rs`
- `docs/CODEK_RUNTIME_CONTRACT.md`
- `crates/apps/src/doctor.rs`

### 구현 메모
- run마다 worktree/cwd를 명확히 결정
- session reuse는 현재 계약(`cwd`, `model`) 유지
- version/compatibility를 `doctor`와 trace에 기록
- binary blocked 시 early fail
- optional Git worktree support 추가

### 수락 기준
- provider layer가 autonomous run의 신뢰도를 떨어뜨리지 않는다.
- blocked/degraded/ready 의미가 실행/문서/doctor에서 같다.

---

## Phase P5 — Tool Surface & Verifier Completion

### 바꿀 것
tool contract를 “기록용 보조”가 아니라 “작업 완료용 실행면”으로 완성한다.

### 주 파일
- `crates/adapters/src/contracts.rs`
- `crates/adapters/src/tool.rs`
- `crates/adapters/src/tool_workspace.rs`
- `crates/adapters/tests/tool.rs`
- `crates/apps/src/runtime_compose.rs`

### 구현 메모
- 각 tool call에 deterministic artifact 남기기
- `run_command`는 allowlist + timeout + cwd + output truncation 보장
- verifier는 build/test/lint/file assertions를 표준화
- patch evidence는 operator가 읽을 수 있는 형태까지 강화

### 수락 기준
- “변경”과 “검증”이 모두 structured artifact를 남긴다.

---

## Phase P6 — Context & Memory Engineering

### 바꿀 것
run이 길어질수록 context를 더 엄격히 다룬다.

### 주 파일
- `crates/apps/src/runtime_compose.rs`
- `crates/apps/src/trace_store.rs`
- `crates/apps/src/state_store.rs`
- `crates/adapters/src/memory.rs`
- `crates/adapters/src/memory_sqlite.rs`
- `crates/adapters/src/memory_markdown.rs`

### 구현 메모
- working memory와 recall memory를 분리
- latest plan summary / latest failures / selected files만 hot context로 유지
- artifact index를 만들어 replay와 doctor가 빠르게 요약 가능하게 함
- `AGENTS.md`를 durable instruction source로 로드

### 수락 기준
- 긴 run에서도 prompt size가 통제된다.
- 재시작 후에도 필요한 맥락이 유지된다.

---

## Phase P7 — Safety / Approval / Resume

### 바꿀 것
자율 실행에 필요한 operator control을 명시적으로 제공한다.

### 주 파일
- `crates/apps/src/cli_command.rs`
- `crates/apps/src/cli_runtime.rs`
- `crates/apps/src/state_store.rs`
- `crates/apps/src/status.rs`
- `crates/apps/src/replay.rs`
- `docs/RUNBOOK.md`

### 구현 메모
- approval policy
- resume token or run-id
- abort semantics
- rollback/checkpoint
- risk tiering (`remove_path`, broad replace, dangerous commands)

### 수락 기준
- operator가 언제든 멈추고, 보고, 재개할 수 있다.

---

## Phase P8 — Eval / Release Gate / Dogfood

### 바꿀 것
CI와 nightly를 autonomy 중심으로 재구성한다.

### 주 파일
- `crates/apps/tests/e2e_cli.rs`
- `crates/apps/tests/release_security_gate.rs`
- `crates/core/tests/*`
- `crates/adapters/tests/*`
- `docs/CAPABILITY_MATRIX.md`
- `docs/RUNBOOK.md`
- `plans/TASKS.md`

### 평가 항목
- task success rate
- false-success rate
- false-done rate
- blocked path correctness
- replay completeness
- artifact completeness
- unsafe command rejection
- boundary escape rejection

### 수락 기준
- release 여부가 autonomy eval로 결정된다.

---

## 가장 중요한 아키텍처 결론

AxonRunner는 아래처럼 남는 것이 맞다.

- `core` = run domain + policy + event sourcing
- `apps` = CLI + loop harness + replay/status/doctor
- `adapters` = codek provider + memory + workspace tools

즉, **작은 OS가 아니라 작은 autonomous harness**가 되어야 한다.
