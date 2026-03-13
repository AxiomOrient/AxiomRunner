# Tasks

범위:

- 현재 실행 루프는 `AZ-019 ~ AZ-020`, `AZ-027 ~ AZ-037`만 포함한다.
- `AZ-001 ~ AZ-018`, `AZ-021 ~ AZ-026`은 완료된 선행 조건으로 간주한다.

상태:

- `TODO`
- `DOING`
- `DONE`
- `BLOCKED`

우선순위:

- `P0`
- `P1`
- `P2`

## Critical Path

`AZ-019 ~ AZ-020` → `AZ-027 ~ AZ-030` → `AZ-034 ~ AZ-037`
→ `AZ-031` → `AZ-032 ~ AZ-033`

## Phase 1. Substrate Tail Lock

| TASK-ID | Pri | Status | Action | Done When | Evidence Required | Depends On | Verification Path |
|---|---|---|---|---|---|---|---|
| AZ-019 | P1 | DONE | session/workspace reuse contract를 문서화한다 | `provider_codex_runtime`의 session 재사용과 workspace 바인딩 의미를 operator가 해석할 수 있다 | `docs/CODEK_RUNTIME_CONTRACT.md` + doctor contract review | - | doc review + `cargo run -p axonrunner_apps -- --workspace="$PWD" doctor --json` |
| AZ-020 | P1 | DONE | `0.5.x` 업그레이드 decision record를 작성한다 | `0.5.0` 채택 이유, 비목표, rollback 조건이 남는다 | `docs/ADR_CODEX_RUNTIME_0_5.md` | AZ-019 | ADR review |

## Phase 2. Runtime Hardening Tail

| TASK-ID | Pri | Status | Action | Done When | Evidence Required | Depends On | Verification Path |
|---|---|---|---|---|---|---|---|
| AZ-027 | P1 | DONE | async runtime fallback 정책을 operator-visible로 노출한다 | fallback이 `doctor` 또는 operator-facing output/doc에서 보인다 | `async_runtime_host`/`doctor` diff + doctor output | AZ-020 | `cargo test -p axonrunner_apps` |
| AZ-028 | P1 | DONE | `batch --reset-state` semantics를 문서화한다 | reset 범위 오해가 README / runtime docs에서 제거된다 | README / DEPLOYMENT / RUNBOOK diff | AZ-027 | docs review + e2e/doc check |
| AZ-029 | P1 | DONE | `--reset-trace` / `--reset-artifacts` 분리 여부를 결정한다 | 새 flags 추가 또는 ADR/doc-only 결정이 명시된다 | `docs/RUNBOOK.md` doc-only decision | AZ-028 | ADR review or `cargo test -p axonrunner_apps` |
| AZ-030 | P1 | DONE | remove_path / run_command high-risk trace를 강화한다 | 위험 작업의 trace/report 설명력이 현재보다 높아진다 | command artifact + tests | AZ-027 | `cargo test -p axonrunner_adapters`, `cargo test -p axonrunner_apps` |

## Phase 3. Release Readiness

| TASK-ID | Pri | Status | Action | Done When | Evidence Required | Depends On | Verification Path |
|---|---|---|---|---|---|---|---|
| AZ-034 | P1 | DONE | install/build/doctor/run/replay runbook을 정리한다 | 신규 사용자가 현재 제품면 기준으로 따라갈 runbook이 있다 | `docs/RUNBOOK.md` | AZ-030 | clean-machine rehearsal note |
| AZ-035 | P1 | DONE | minimal example scenario를 추가한다 | README 또는 docs에 현재 제품과 맞는 최소 예시가 있다 | README quick-start and runbook example | AZ-034 | manual walkthrough |
| AZ-036 | P1 | DONE | release checklist를 작성한다 | release gate가 checklist 형태로 명시된다 | `docs/RELEASE_CHECKLIST.md` | AZ-034 | checklist review |
| AZ-037 | P2 | DONE | versioning/changelog policy를 문서화한다 | CHANGELOG 운영 규칙과 versioning rule이 고정된다 | `docs/VERSIONING.md` + CHANGELOG alignment | AZ-036 | doc review |

## Phase 4. Cleanup And Experimental Decision

| TASK-ID | Pri | Status | Action | Done When | Evidence Required | Depends On | Verification Path |
|---|---|---|---|---|---|---|---|
| AZ-031 | P2 | DONE | markdown memory header의 `ZeroClaw` 잔재를 제거한다 | branding residue가 제거된다 | `memory_markdown.rs` diff | AZ-037 | `cargo test -p axonrunner_adapters` |
| AZ-032 | P2 | DONE | experimental OpenAI provider의 fate를 결정한다 | 유지/격리/삭제 중 하나가 문서와 코드에서 명시된다 | capability matrix + deployment/readme wording | AZ-037 | ADR review |
| AZ-033 | P2 | DONE | 유지 시 blocking reqwest를 제거 또는 분리한다 | async bridge 일관성이 맞는다 | `provider_openai.rs` + adapter tests | AZ-032 | `cargo test -p axonrunner_adapters` |

## Completed Preconditions

- `AZ-001 ~ AZ-018`
- `AZ-021 ~ AZ-026`
