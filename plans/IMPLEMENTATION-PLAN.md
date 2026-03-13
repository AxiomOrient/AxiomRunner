# Implementation Plan

## Planning Goal

완료된 1차 execution evidence를 전제로, AxonRunner의 deferred tail
(`AZ-019 ~ AZ-020`, `AZ-027 ~ AZ-037`)을 다음 active scope로 승격해
운영 꼬리와 release-readiness를 닫는 bounded delivery로 정리한다.

## Target Scope

- 입력 근거: `plans/docs/01_REVIEW_REPORT.md` ~ `plans/docs/07_SELF_REVIEW.md`,
  `plans/data/tasks.csv`, `plans/data/tasks.json`, 현재 `plans/TASKS.md`
- 제품 truth surface: `README.md`, `CHANGELOG.md`, `docs/project-charter.md`,
  `docs/DEPLOYMENT.md`, `docs/CAPABILITY_MATRIX.md`, CLI `--help`, `doctor --json`
- 구현 표면: deferred tail에 직접 걸리는 파일
  `crates/adapters/src/provider_codex_runtime.rs`,
  `crates/apps/src/async_runtime_host.rs`,
  `crates/apps/src/cli_runtime.rs`,
  `crates/apps/src/cli_command.rs`,
  `crates/adapters/src/tool.rs`,
  `crates/apps/src/trace_store.rs`,
  `crates/adapters/src/memory_markdown.rs`,
  `crates/adapters/src/provider_openai.rs`, docs
- 실행 산출물: `plans/IMPLEMENTATION-PLAN.md`, `plans/TASKS.md`

## Done Condition

1. `provider_codex_runtime` 문서/contract에 session reuse와 workspace binding semantics가 남는다.
2. async runtime fallback, reset semantics, high-risk tool traces가 operator-visible contract로 잠긴다.
3. release-readiness 문서(runbook, example, checklist, versioning/changelog policy)가 현재 제품면과 일치한다.
4. `ZeroClaw` 잔재 제거와 experimental OpenAI provider fate가 명시적으로 정리된다.

## Constraints

- planning-only 범위로 제한한다. 이 문서는 구현이나 코드 리뷰 verdict를 대신하지 않는다.
- 현재 실행 루프는 `AZ-019 ~ AZ-020`, `AZ-027 ~ AZ-037`만 포함한다.
- `AZ-001 ~ AZ-018`, `AZ-021 ~ AZ-026`은 완료된 선행 조건이며 재실행 범위에 포함하지 않는다.
- task id는 review bundle의 `AZ-*` 식별자를 그대로 유지한다.
- `plans/docs/*`는 review input이고, canonical execution artifact는 루트 `plans/*.md`다.
- `AZ-029`는 실제 CLI surface 추가와 ADR/doc-only 결정 사이의 선택지를 열어 둔다.
- `AZ-032`는 정책 결정(유지/격리/삭제)이 선행되어야 하며, 이 결정이 `AZ-033`의 필요 여부를 바꾼다.

## Critical Path

`AZ-019 ~ AZ-020` → `AZ-027 ~ AZ-030` → `AZ-034 ~ AZ-037`
→ `AZ-031` → `AZ-032 ~ AZ-033`

## Decision Gates

| Gate Name | Check | Pass Condition | On Fail |
|---|---|---|---|
| Substrate Tail Gate | session/workspace reuse contract + upgrade record | `provider_codex_runtime` 운영 의미와 `0.5.x` 결정이 문서로 잠긴다 | runtime/release tail로 넘어가지 않는다 |
| Runtime Hardening Gate | async fallback / reset semantics / high-risk traces | operator가 fallback, reset, risky operations를 문서와 출력으로 해석할 수 있다 | release-readiness 문서를 쓰지 않는다 |
| Release Readiness Gate | runbook / example / checklist / versioning policy | 새 사용자가 install/build/doctor/run/replay까지 따라갈 문서 surface가 갖춰진다 | cleanup/experimental decision을 뒤로 미룬다 |
| Cleanup Decision Gate | branding residue + OpenAI provider decision | `ZeroClaw` 잔재가 제거되고 experimental provider fate가 문서와 코드에 맞게 정리된다 | tail loop를 pass로 닫지 않는다 |

## Phase Plan

### Phase 1. Substrate Tail Lock

대상 태스크: `AZ-019 ~ AZ-020`

- `provider_codex_runtime`의 session/workspace reuse contract를 문서화한다.
- `0.5.x` 채택 이유와 non-goal, rollback 조건을 ADR 성격 문서로 남긴다.

종료 기준:

- `codek` substrate 운영 의미와 upgrade rationale이 operator-facing 문서에 남는다.

### Phase 2. Runtime Hardening Tail

대상 태스크: `AZ-027 ~ AZ-030`

- async runtime fallback 정책을 operator-visible surface로 올린다.
- `batch --reset-state` semantics와 reset surface 분리 여부를 명시한다.
- remove_path / run_command high-risk trace를 더 강화한다.

종료 기준:

- fallback, reset, risky operation semantics가 더 이상 암묵적이지 않다.

### Phase 3. Release Readiness

대상 태스크: `AZ-034 ~ AZ-037`

- install/build/doctor/run/replay runbook을 정리한다.
- minimal example, release checklist, versioning/changelog policy를 만든다.

종료 기준:

- release-facing docs만 읽고도 현재 제품면을 따라갈 수 있다.

### Phase 4. Cleanup And Experimental Decision

대상 태스크: `AZ-031 ~ AZ-033`

- `ZeroClaw` 잔재를 제거한다.
- experimental OpenAI provider 유지 여부를 결정한다.
- 유지 시 blocking reqwest 제거 또는 분리 방향까지 닫는다.

종료 기준:

- 브랜드 잔재와 experimental provider fate가 열린 꼬리로 남지 않는다.

## Verification Strategy

### Substrate / Runtime Surface

- `cargo run -p axonrunner_apps -- --workspace="$PWD" doctor --json`
- `cargo test -p axonrunner_adapters`
- `cargo test -p axonrunner_apps`

### Release Docs Surface

- manual cross-check:
  `README.md`, `CHANGELOG.md`, `docs/project-charter.md`,
  `docs/DEPLOYMENT.md`, `docs/CAPABILITY_MATRIX.md`, new release docs
- `crates/apps/tests/release_security_gate.rs`

### Full Contract Surface

- `cargo test --workspace`

## Open Edges

- `AZ-029`는 실제로 새 CLI flags를 추가할지, ADR 수준 문서화로 닫을지 구현 단계에서 결정이 필요하다.
- `AZ-032`는 provider를 유지/격리/삭제 중 하나로 선택해야 하며, 이 결정이 `AZ-033`의 필요 여부를 바꾼다.
