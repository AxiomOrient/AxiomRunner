# Project Structure

AxiomRunner 저장소는 `core`, `adapters`, `apps` 3개 크레이트를 중심으로 돌아간다.

## 한눈에 보기

```
crates/core      — goal/run 계약과 상태 primitive
crates/adapters  — provider/tool/memory substrate
crates/apps      — CLI entrypoint와 run orchestration
docs/            — 현재 제품 계약과 운영 문서
examples/        — representative verifier examples
scripts/         — 반복 실행 보조
```

## crates/core

goal/run 입력 계약과 상태 primitive를 소유한다.

| 파일 | 역할 |
|---|---|
| `intent.rs` | `RunGoal`, `RunBudget`, `DoneCondition`, `VerificationCheck`, `RunConstraintPolicyKey` |
| `decision.rs` | `DecisionOutcome` |
| `policy_codes.rs` | policy code enum |
| `state.rs` | `AgentState`, `ExecutionMode` |
| `validation.rs` | 입력 검증 helper |
| `lib.rs` | public re-export |

`RunConstraintPolicyKey`: `PathScope`, `DestructiveCommandClass`, `ExternalCommandClass`, `ApprovalEscalation` — 강제 적용 constraint label.
`RunConstraint::mode()`: `EnforcedSubset` 또는 `Advisory` 반환.

## crates/adapters

바깥 세계와 붙는 substrate다.

| 파일 | 역할 |
|---|---|
| `contracts.rs` | workflow pack, tool, provider 계약 shared types |
| `provider_codex_runtime.rs` | primary `codek` provider |
| `provider_openai.rs` | experimental opt-in provider |
| `provider_registry.rs` | provider 등록/해석 |
| `tool.rs` | 파일/명령 실행 표면, `RunCommandClass` enum |
| `tool_write.rs` | patch artifact / command artifact evidence |
| `tool_workspace.rs` | workspace boundary helper |
| `memory.rs`, `memory_markdown.rs`, `memory_sqlite.rs` | memory backend |

`RunCommandClass`: `WorkspaceLocal`, `Destructive`(`rm`/`mv`), `External`(나머지).
`classify_run_command_class(program)` 함수로 command를 분류하고 constraint enforcement에 사용한다.

## crates/apps

실제 제품 surface를 소유한다.

| 파일 | 역할 |
|---|---|
| `main.rs`, `lib.rs` | entrypoint와 exit code |
| `cli_args.rs`, `cli_command.rs` | CLI parsing |
| `config_loader.rs` | file/env/cli config merge |
| `goal_file.rs` | goal file load + workflow pack manifest binding |
| `cli_runtime.rs` | `run / resume / abort / status / doctor / health` lifecycle |
| `cli_runtime/lifecycle.rs` | verification / repair / finalize helper, `classify_goal_verifier_strength()` |
| `runtime_compose.rs` | provider/memory/tool orchestration, constraint policy, pure mapping functions |
| `runtime_compose/plan.rs` | default verifier derivation, run plan assembly |
| `runtime_compose/artifacts.rs` | plan/apply/verify/report/checkpoint/rollback artifact write |
| `state_store.rs` | runtime state snapshot persistence |
| `trace_store.rs` | append-only trace와 replay summary |
| `workspace_lock.rs` | single-writer lock |
| `operator_render.rs` | status/doctor/replay 출력 렌더링 |
| `status.rs`, `replay.rs`, `doctor.rs`, `display.rs` | operator-facing output |
| `async_runtime_host.rs` | async host setup |

### 내부 흐름

1. `main.rs` → CLI entrypoint 오픈
2. `cli_args.rs` + `cli_command.rs` → 명령 파싱
3. `config_loader.rs` → config 합침
4. `goal_file.rs` → goal/workflow pack 입력 고정
5. `cli_runtime.rs` → 명령별 실행 경로 선택, constraint policy 검사, approval gate
6. `runtime_compose.rs` → provider/memory/tool 실행과 artifact 생성 조립
7. `state_store.rs` + `trace_store.rs` + `workspace_lock.rs` → 상태/증거/lock 기록
8. `status` / `replay` / `doctor` → 같은 증거를 다시 읽어 operator에게 표시

### 현재 실행 성격

- accepted goal run도 현재는 `workflow_pack + verifier tool path` 중심으로 조립된다.
- `build_runtime_compose_plan()`은 현재 accepted run에 대해 `provider: None`, `tool: None`으로 시작하고,
  verifier command 경로는 별도 default/packs 규칙에서 도출된다.
- 즉 현재 제품은 general autonomous planning runtime보다 verifier-first goal runtime에 가깝다.

### 핵심 순수 함수

`runtime_compose.rs`의 pure mapping functions:
- `verifier_strength_label(verification_status: &str) -> &str` — strength 도출. 현재는 identity (state = strength).
- `run_reason_code(reason: &str) -> &str` — reason을 짧은 code로 매핑
- `run_reason_detail(reason: &str) -> &str` — reason detail 추출
- `run_phase_name(phase: RuntimeRunPhase) -> &str`
- `run_outcome_name(outcome: RuntimeRunOutcome) -> &str`

## codek Provider Contract

`provider=codek` substrate의 운영 규칙.

**Session Reuse**:
- `cwd`와 `model`이 모두 같은 경우에만 session을 재사용한다.
- `cwd` 또는 `model`이 달라지면 기존 session을 닫고 새 session을 만든다.
- 같은 provider 인스턴스 안에서 `ask`는 동시에 두 번 보내지 않는다 (per-session serialization).

**Workspace Binding**:
- `codek` provider의 `cwd`는 runtime tool workspace와 같은 경계에 묶인다.
- workspace가 결정되지 않으면 runtime init 단계에서 fail-closed로 멈춘다.
- 다른 임의 cwd로 흘러가는 숨은 fallback이 없다.

**Compatibility**:
- bundled crate pin: `codex-runtime 0.5.0`
- minimum supported Codex CLI: `0.104.0`
- `doctor --json` provider detail: `cli_bin`, `version`, `compatibility`, `min_supported` 노출

**Health States**:
- `ready`: binary probe와 handshake 통과
- `degraded`: binary는 보였지만 version parse 또는 shutdown path가 불완전
- `blocked`: binary missing, version minimum 미달, handshake 실패

## docs / examples / scripts

- `docs/` — 현재 제품 truth (이 디렉터리)
  - `project-charter.md` — 제품 정의, 아키텍처, retained surface, 원칙
  - `RUNBOOK.md` — 빌드, 실행, 운영, 버전 정책
  - `CAPABILITY_MATRIX.md` — 지원 범위, constraint enforcement, release blocker
  - `WORKFLOW_PACK_CONTRACT.md` — goal 스키마, pack 계약, verification/done 규칙
  - `AUTONOMOUS_AGENT_TARGET.md` — bridge target
  - `AUTONOMOUS_AGENT_SPEC.md` — bridge spec
  - `VERSIONING.md` — versioning / changelog / release gate 규칙
  - `PROJECT_STRUCTURE.md` — 이 파일
- `examples/` — representative app/server verifier examples
- `scripts/` — `nightly_dogfood.sh`, `generate_goal_stack.py` 같은 반복 실행 보조

중요: `docs/`와 루트 `README.md`가 shipped truth를 소유한다. 임시 메모나 작업 중 분석 문서보다 `docs/`가 우선이다.
`AUTONOMOUS_AGENT_TARGET`와 `AUTONOMOUS_AGENT_SPEC`은 bridge 문서이며 current truth를 덮어쓰지 않는다.

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
