# AxonRunner Post-Board Tasks

상태 값:

- `TODO`
- `DOING`
- `DONE`
- `BLOCKED`

## A. Patch Evidence Hardening

| ID | Status | Task | Files | Done Condition | Verification |
| --- | --- | --- | --- | --- | --- |
| PB-001 | DONE | file write / replace 결과에 before/after digest 추가 | `crates/adapters/src/tool.rs`, `crates/adapters/src/tool_write.rs`, `crates/adapters/src/contracts.rs` | text mutation 결과가 digest를 남긴다 | `cargo test -p axonrunner_adapters --test tool` |
| PB-002 | DONE | patch artifact metadata를 trace event에 연결 | `crates/apps/src/trace_store.rs`, `crates/apps/src/replay.rs`, `crates/apps/tests/e2e_cli.rs` | replay가 patch evidence 위치를 보여준다 | `cargo test -p axonrunner_apps --test e2e_cli` |

## B. Trace Store Formalization

| ID | Status | Task | Files | Done Condition | Verification |
| --- | --- | --- | --- | --- | --- |
| PB-003 | DONE | trace append/read API를 storage facade로 분리 | `crates/apps/src/trace_store.rs`, `crates/apps/src/lib.rs` | CLI가 concrete file layout에 직접 의존하지 않는다 | `cargo test -p axonrunner_apps` |
| PB-004 | DONE | trace schema versioning rule과 compatibility test 추가 | `crates/apps/src/trace_store.rs`, `crates/apps/tests/e2e_cli.rs` | additive schema change가 legacy trace를 깨지 않는다 | `cargo test -p axonrunner_apps` |

## C. Validation Corpus Expansion

| ID | Status | Task | Files | Done Condition | Verification |
| --- | --- | --- | --- | --- | --- |
| PB-005 | DONE | doctor/replay contract golden scenario 추가 | `crates/apps/tests/e2e_cli.rs` | operator-facing text/json outputs가 stable 하다 | `cargo test -p axonrunner_apps --test e2e_cli` |
| PB-006 | DONE | mock-local / blocked-codek / persisted-state 시나리오를 corpus 관점으로 재정렬 | `crates/apps/tests/e2e_cli.rs`, `crates/adapters/tests/tool.rs`, `crates/core/tests/*` | 핵심 계약이 시나리오 이름으로 직접 보인다 | `cargo test -p axonrunner_apps && cargo test -p axonrunner_adapters && cargo test -p axonrunner_core` |

## D. Runtime Contract Cleanup

| ID | Status | Task | Files | Done Condition | Verification |
| --- | --- | --- | --- | --- | --- |
| PB-007 | DONE | command allowlist를 runtime config surface와 연결 | `crates/apps/src/config_loader.rs`, `crates/apps/src/runtime_compose.rs`, `crates/adapters/src/contracts.rs` | allowlist가 env/internal constant가 아니라 공식 surface가 된다 | `cargo test -p axonrunner_apps` |
| PB-008 | DONE | bounded substring/regex search semantics 분리 | `crates/adapters/src/tool.rs`, `crates/adapters/src/tool_workspace.rs`, `crates/adapters/src/contracts.rs`, `crates/adapters/tests/tool.rs` | search request가 명시적 mode를 가진다 | `cargo test -p axonrunner_adapters --test tool` |
| PB-009 | DONE | macOS/Linux shell-less execution contract를 더 강하게 잠금 | `crates/adapters/src/tool.rs`, `crates/adapters/tests/tool.rs` | wrapper가 shell interpreter 의존 없이 contract를 증명한다 | `cargo test -p axonrunner_adapters --test tool` |

## Exit Rule

이 문서의 작업은 `issue/`의 연장이 아니다.
새 작업은 반드시 `PB-*` 형식으로만 추가하고, AX 보드와 섞지 않는다.
