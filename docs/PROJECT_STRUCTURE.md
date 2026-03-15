# Project Structure

AxiomRunner 저장소는 `core`, `adapters`, `apps` 3개 크레이트를 중심으로 돌아간다.
현재 제품은 `goal-file` 중심 CLI runtime이므로, 구조 설명도 그 계약을 기준으로 읽는 게 맞다.

## 한눈에 보기

- `crates/core` — goal/run 계약과 상태 primitive
- `crates/adapters` — provider/tool/memory substrate
- `crates/apps` — CLI entrypoint와 실제 run orchestration
- `docs` — 현재 제품 계약과 운영 문서
- `examples` — representative verifier examples
- `scripts` — 반복 실행 보조
- `plans` — review/plan/task 같은 작업 산출물
- `target` — 빌드 결과물

## crates/core

현재 `core`는 작은 계약 층이다. 예전 event/reducer 모델을 설명하는 문서는 더 이상 맞지 않는다.

주요 파일:

- `intent.rs` — `RunGoal`, `RunBudget`, `DoneCondition`, `VerificationCheck`
- `decision.rs` — `DecisionOutcome`
- `policy_codes.rs` — policy code enum
- `state.rs` — `AgentState`, `ExecutionMode`
- `validation.rs` — 입력 검증 helper
- `lib.rs` — public re-export

핵심 역할:

- goal/run 입력 계약 정의
- 상태 primitive 정의
- apps/adapters가 공유하는 최소 타입 제공

## crates/adapters

`adapters`는 바깥 세계와 붙는 substrate다.

주요 파일:

- `contracts.rs` — workflow pack, tool, provider 계약
- `provider_codex_runtime.rs` — primary `codek` provider
- `provider_openai.rs` — experimental opt-in provider
- `provider_registry.rs` — provider 등록/해석
- `tool.rs` — 파일/명령 실행 표면
- `tool_write.rs` — patch artifact / command artifact evidence
- `tool_workspace.rs` — workspace boundary helper
- `memory.rs`, `memory_markdown.rs`, `memory_sqlite.rs` — memory backend

핵심 역할:

- provider health와 request execution
- tool boundary / allowlist / evidence
- workflow pack contract shared types

## crates/apps

`apps`는 실제 제품 surface를 가진다.

주요 파일:

- `main.rs`, `lib.rs` — entrypoint와 exit code
- `cli_args.rs`, `cli_command.rs` — CLI parsing
- `config_loader.rs` — file/env/cli config merge
- `goal_file.rs` — goal file load + workflow pack manifest binding
- `cli_runtime.rs` — `run / resume / abort / status / doctor / health`
- `cli_runtime/lifecycle.rs` — verification / repair / finalize helper
- `runtime_compose.rs` — provider/memory/tool orchestration
- `runtime_compose/plan.rs` — default verifier derivation / run plan assembly
- `runtime_compose/artifacts.rs` — plan/apply/verify/report/checkpoint/rollback artifact write
- `state_store.rs` — runtime state snapshot persistence
- `trace_store.rs` — append-only trace and replay summary
- `workspace_lock.rs` — single-writer lock
- `status.rs`, `replay.rs`, `doctor.rs`, `operator_render.rs`, `display.rs` — operator-facing output
- `async_runtime_host.rs` — async host setup

### apps 내부 흐름

1. `main.rs` / `lib.rs`가 CLI entrypoint를 연다.
2. `cli_args.rs`와 `cli_command.rs`가 명령을 파싱한다.
3. `config_loader.rs`가 config를 합친다.
4. `goal_file.rs`가 goal/workflow pack 입력을 고정한다.
5. `cli_runtime.rs`가 명령별 실행 경로를 선택한다.
6. `runtime_compose.rs`가 provider/memory/tool 실행과 artifact 생성을 조립한다.
7. `state_store.rs`, `trace_store.rs`, `workspace_lock.rs`가 상태/증거/lock을 남긴다.
8. `status`, `replay`, `doctor` 경로가 같은 증거를 다시 읽어 보여준다.

## docs / examples / scripts / plans

- `docs` — 현재 제품 truth
  - `project-charter.md`
  - `CAPABILITY_MATRIX.md`
  - `RUNBOOK.md`
  - `CODEK_RUNTIME_CONTRACT.md`
  - `WORKFLOW_PACK_CONTRACT.md`
  - `VERSIONING.md`
  - `PROJECT_STRUCTURE.md`
- `examples` — representative app/server verifier examples
- `scripts` — `nightly_dogfood.sh` 같은 반복 실행 보조
- `plans` — review bundle, implementation plan, task ledger 같은 작업 산출물

중요:

- `docs/`와 루트 `README.md`가 현재 shipped truth를 소유한다.
- `plans/`는 작업에 도움을 주는 산출물이지, 기본 제품 truth가 아니다.

## 구조를 읽는 순서

1. `README.md`
2. `docs/project-charter.md`
3. `docs/RUNBOOK.md`
4. `docs/CAPABILITY_MATRIX.md`
5. `crates/apps/src/cli_command.rs`
6. `crates/apps/src/cli_runtime.rs`
7. `crates/apps/src/runtime_compose.rs`
8. `crates/apps/src/state_store.rs`
9. `crates/apps/src/trace_store.rs`

이 순서로 보면 제품 표면 → 실행 경로 → 상태/증거 저장 순서가 자연스럽게 이어진다.
