# 06. Tasks Backlog

상태 규칙:

- `TODO | DOING | DONE | BLOCKED`
- `DONE`은 반드시 검증 명령과 증거를 남긴다.
- 아래 태스크는 **현재 Option C 완료 이후**의 제품화 백로그다.

## Cleanup Sync — 2026-03-12

| ID | 상태 | 작업 | 파일/모듈 | 선행 | 완료 기준 | 검증 |
|---|---|---|---|---|---|---|
| TASK-C0-001 | DONE | legacy `agent` CLI / adapter / OTP surface 제거 | `crates/apps/src/cli_command.rs`, `crates/apps/src/lib.rs`, `crates/apps/tests/e2e_cli.rs`, `crates/adapters/src/contracts.rs`, `crates/adapters/src/lib.rs` | 없음 | 기본 build에서 `agent`, `AgentAdapter`, `agent_registry`, OTP gate 참조가 사라짐 | `cargo check`; `cargo test -p axonrunner_apps --test e2e_cli`; `rg -n "AgentAdapter|build_contract_agent|AXONRUNNER_AGENT_|AXONRUNNER_OTP_" crates` |
| TASK-C0-002 | DONE | dead `endpoint` config surface 제거 | `crates/apps/src/config_loader.rs`, `crates/apps/src/cli_command.rs`, `crates/apps/src/cli_runtime.rs`, `crates/apps/tests/config_priority.rs` | 없음 | 기본 build에서 `endpoint` 설정/CLI/help/health 출력 참조가 사라짐 | `cargo check`; `cargo test -p axonrunner_apps --test config_priority`; `cargo test -p axonrunner_apps --test e2e_cli`; `cargo test -p axonrunner_apps --test release_security_gate`; `rg -n "endpoint" crates/apps README.md docs` |
| TASK-C0-003 | DONE | dead provider alias/adapter indirection 제거 | `crates/adapters/src/provider_registry.rs`, `crates/adapters/src/lib.rs` | 없음 | provider registry가 canonical id만 유지하고 `resolve_provider_adapter_id`가 사라짐 | `cargo check`; `cargo test -p axonrunner_adapters provider_registry --quiet`; `rg -n "resolve_provider_adapter_id|adapter_id:|aliases:|provider\\.openai|provider\\.mock-local" crates` |
| TASK-C0-004 | DONE | fail-fast provider misconfiguration regression 고정 | `crates/apps/tests/e2e_cli.rs`, `crates/apps/tests/common/mod.rs` | 없음 | 잘못된 runtime provider가 env/config file 어디서 와도 즉시 startup failure로 종료하고 테스트 유틸이 retained env surface만 정리함 | `cargo test -p axonrunner_apps --test e2e_cli`; `rg -n "AXONRUNNER_ENDPOINT|AXONRUNNER_CHANNEL_STORE_PATH|AXONRUNNER_DAEMON_HEALTH_PATH|AXONRUNNER_SERVICE_STATE_PATH|AXONRUNNER_ONBOARD_" crates/apps/tests` |

현재 backlog의 `agent_codek`/`agent_registry` 기준 행은 legacy 초안을 보존한 것이다. 이후 작업은 별도 agent 계층이 아니라 `run` 경로와 runtime/provider 계약 기준으로 해석한다.

## Phase P1 — `coclai -> codek` 전환

| ID | 상태 | 작업 | 파일/모듈 | 선행 | 완료 기준 | 검증 |
|---|---|---|---|---|---|---|
| TASK-P1-001 | DONE | `coclai` path dependency 제거 | `crates/adapters/Cargo.toml` | 없음 | `../../../coclai` 경로가 완전히 사라짐 | `rg -n "coclai" crates` |
| TASK-P1-002 | DONE | `codek` dependency 도입 (`package = codex-runtime`) | `crates/adapters/Cargo.toml` | P1-001 | lockfile가 갱신되고 compile graph가 고정됨 | `cargo tree -p axonrunner_adapters` |
| TASK-P1-003 | DONE | legacy `agent_coclai.rs` 제거 | `crates/adapters/src/agent_coclai.rs` | P1-001 | build path에서 참조가 사라짐 | `rg -n "agent_coclai" crates` |
| TASK-P1-004 | BLOCKED | standalone `agent_codek.rs` 추가 | `crates/adapters/src/agent_codek.rs` | P1-002 | 별도 agent 계층 대신 runtime/provider direct path 채택 여부가 확정돼야 함 | architecture sync |
| TASK-P1-005 | DONE | legacy `AgentAdapter` registry 제거 | `crates/adapters/src/contracts.rs`, `crates/adapters/src/lib.rs` | P1-003 | 기본 build에서 `AgentAdapter`/`agent_registry`가 사라짐 | `rg -n "AgentAdapter|agent_registry" crates` |
| TASK-P1-006 | TODO | schema에 backend config 추가 | `crates/schema/src/config.rs` | P1-004 | `backend.kind`, `model`, `timeout` 등이 타입화됨 | `cargo test -p axonrunner_schema --quiet` |
| TASK-P1-007 | TODO | `doctor`에 `codex` binary 존재 probe 추가 | `crates/apps/src/doctor.rs` | P1-004 | doctor가 binary absence를 명시적으로 보고 | `cargo test -p axonrunner_apps doctor --quiet` |
| TASK-P1-008 | DONE | `run` 경로에서 `codek` backend selection 연결 | `crates/apps/src/runtime_compose.rs`, `crates/adapters/src/provider_codex_runtime.rs`, `crates/adapters/src/provider_registry.rs` | P1-005, P1-006 | 실제 `run`이 codek backend로 세션 생성 시도 | `cargo test -p axonrunner_apps --test e2e_cli`; `cargo test -p axonrunner_adapters provider_registry --quiet` |
| TASK-P1-009 | TODO | `MockBackend`를 first-class backend로 유지 | `crates/adapters/src/agent_mock.rs` 또는 신규 | P1-005 | mock로 golden tests 구동 가능 | `cargo test --workspace --features mock-backend` |
| TASK-P1-010 | TODO | README/DEPLOYMENT에서 `coclai` 문구 제거 | `README.md`, `docs/DEPLOYMENT.md` | P1-008 | 사용자 문서에 backend truth가 반영됨 | docs snapshot test |

## Phase P2 — Canonical run path 고정

| ID | 상태 | 작업 | 파일/모듈 | 선행 | 완료 기준 | 검증 |
|---|---|---|---|---|---|---|
| TASK-P2-001 | DONE | `run`을 single canonical path로 고정 | `crates/apps/src/cli_command.rs`, `crates/apps/tests/e2e_cli.rs` | P1 완료 | canonical `run <intent-spec>`가 생기고 기존 단축 명령이 모두 동일 `run` 경로로 정규화됨 | `cargo test -p axonrunner_apps --test e2e_cli` |
| TASK-P2-002 | TODO | `run` state machine 도입 | `crates/apps/src/agent_loop.rs` 또는 runtime split | P2-001 | inspect/plan/apply/verify/finalize 상태가 모델링됨 | state transition tests |
| TASK-P2-003 | TODO | hidden fallback 제거 검증 추가 | runtime compose 전반 | P2-001 | 잘못된 backend/config에서 즉시 실패 | fail-fast regression |
| TASK-P2-004 | TODO | final answer contract 고정 | display/output layer | P2-002 | 변경/명령/검증결과/남은 리스크를 항상 출력 | snapshot tests |
| TASK-P2-005 | TODO | exit code contract 정의 | CLI layer | P2-001 | success / verification fail / config fail 분리 | e2e CLI tests |

## Phase P3 — Workspace and patching hardening

| ID | 상태 | 작업 | 파일/모듈 | 선행 | 완료 기준 | 검증 |
|---|---|---|---|---|---|---|
| TASK-P3-001 | TODO | file inventory를 `.gitignore` 존중형으로 교체 | 새 `workspace/scan.rs` 또는 기존 tool/fs | P2 완료 | ignore rules 반영 | fixture tests |
| TASK-P3-002 | TODO | substring/regex search API 분리 | `workspace/search.rs` | P3-001 | 대규모 파일에서도 bounded search | search tests |
| TASK-P3-003 | TODO | path normalization 공통화 | workspace policy module | P3-001 | symlink/`..`/absolute path 우회 차단 | security tests |
| TASK-P3-004 | TODO | atomic patch writer 구현 | `workspace/write.rs`, `patch.rs` | P3-003 | temp write + rename + diff artifact | write tests |
| TASK-P3-005 | TODO | line-ending/preserve-encoding 정책 정의 | write pipeline | P3-004 | 불필요한 전체 파일 churn 방지 | golden diff tests |
| TASK-P3-006 | TODO | patch artifact table 추가 | trace/sqlite | P3-004 | 각 write에 before/after digest 기록 | DB tests |

## Phase P4 — Command verification hardening

| ID | 상태 | 작업 | 파일/모듈 | 선행 | 완료 기준 | 검증 |
|---|---|---|---|---|---|---|
| TASK-P4-001 | TODO | allowlist command spec 정의 | schema/runtime config | P2 완료 | 허용 실행파일 목록이 타입화됨 | schema tests |
| TASK-P4-002 | TODO | `shell = false` 강제 래퍼 구현 | command/exec | P4-001 | 모든 command path가 동일 wrapper 사용 | unit tests |
| TASK-P4-003 | TODO | stdout/stderr truncation 정책 구현 | command/capture | P4-002 | 로그 폭주 방지 | capture tests |
| TASK-P4-004 | TODO | timeout/cancel 정책 구현 | command/exec | P4-002 | hung process가 bounded failure로 종료 | timeout tests |
| TASK-P4-005 | TODO | verification summary event 도입 | core + runtime | P4-002 | 검증 결과가 final answer와 trace에 반영 | replay tests |

## Phase P5 — Trace, replay, doctor

| ID | 상태 | 작업 | 파일/모듈 | 선행 | 완료 기준 | 검증 |
|---|---|---|---|---|---|---|
| TASK-P5-001 | TODO | `runs/events/commands/file_patches/final_reports` schema 추가 | trace/sqlite/migrations | P2 완료 | migration 적용 가능 | DB migration tests |
| TASK-P5-002 | TODO | event append API 구현 | trace/event_log.rs | P5-001 | 모든 단계가 append-only로 기록 | event log tests |
| TASK-P5-003 | TODO | replay loader 구현 | replay.rs | P5-002 | run-id로 세션 요약 가능 | replay tests |
| TASK-P5-004 | TODO | `doctor` 환경 진단 확장 | doctor | P1-007 | backend/config/workspace/permissions 진단 | doctor integration tests |
| TASK-P5-005 | TODO | `doctor --json` 지원 | CLI + doctor | P5-004 | machine-readable diagnostics 제공 | snapshot tests |

## Phase P6 — codek hook bridge

| ID | 상태 | 작업 | 파일/모듈 | 선행 | 완료 기준 | 검증 |
|---|---|---|---|---|---|---|
| TASK-P6-001 | TODO | pre/post run hook 연결 | runtime/provider backend bridge | P1 완료 | run 시작/종료가 event에 기록 | backend tests |
| TASK-P6-002 | TODO | pre/post turn hook 연결 | runtime/provider backend bridge | P6-001 | turn lifecycle 추적 가능 | backend tests |
| TASK-P6-003 | TODO | pre/post tool use hook 연결 | runtime/provider backend bridge | P6-002 | tool 요청과 결과가 trace에 반영 | backend tests |
| TASK-P6-004 | TODO | prompt digest logging | prompt + trace | P6-002 | 민감정보 없이 prompt version 추적 가능 | trace tests |
| TASK-P6-005 | TODO | compatibility guard 도입 | backend init | P6-001 | protocol drift 시 명시적 실패 | compatibility tests |

## Phase P7 — Product surface reduction

| ID | 상태 | 작업 | 파일/모듈 | 선행 | 완료 기준 | 검증 |
|---|---|---|---|---|---|---|
| TASK-P7-001 | TODO | `channel*`를 `experimental` feature 뒤로 이동 | apps/adapters channel modules | P2 완료 | 기본 build에서 채널 제외 | `cargo test --workspace --no-default-features` |
| TASK-P7-002 | TODO | `gateway*`를 experimental로 이동 | apps gateway | P7-001 | 제품 도움말에서 제거 | help snapshot |
| TASK-P7-003 | TODO | `daemon/service/cron/metrics_http` 격리 | apps ops modules | P7-001 | run/doctor/replay만 남음 | CLI tests |
| TASK-P7-004 | TODO | `tool_browser/composio/delegate` 실험 격리 | adapters tools | P7-001 | 제품 tool surface 정리 | tool list tests |
| TASK-P7-005 | TODO | `memory_axiomme/context_axiomme/hybrid` 격리 | adapters memory/context | P7-001 | 기본 제품에 외부 RAG 의존성 없음 | cargo tree diff |

## Phase P8 — Tests and release gates

| ID | 상태 | 작업 | 파일/모듈 | 선행 | 완료 기준 | 검증 |
|---|---|---|---|---|---|---|
| TASK-P8-001 | TODO | golden task corpus 생성 | `tests/golden/*` | P2~P6 | 대표 작업 10종 이상 | corpus runner |
| TASK-P8-002 | TODO | mock backend snapshot suite 구축 | tests | P1-009, P8-001 | deterministic snapshot pass | `cargo insta test` 또는 equivalent |
| TASK-P8-003 | TODO | codek backend smoke suite 구축 | tests/live-smoke | P6 완료 | 실제 backend 연결 smoke 가능 | opt-in tests |
| TASK-P8-004 | TODO | contract gate 스크립트 추가 | scripts | P8-001 | 핵심 경로 자동 검증 | shellcheck + dry run |
| TASK-P8-005 | TODO | `cargo deny`와 라이선스 정책 도입 | repo root | P1 완료 | dependency policy 위반 차단 | `cargo deny check` |
| TASK-P8-006 | TODO | `cargo nextest` 도입 | local scripts | P8-001 | local test runtime 안정화 | local gate pass |
| TASK-P8-007 | TODO | mutation test baseline 도입 | dev docs/local optional | P8-001 | 핵심 로직 weak test 탐지 | `cargo mutants` report |

## Phase P9 — Documentation and release candidate

| ID | 상태 | 작업 | 파일/모듈 | 선행 | 완료 기준 | 검증 |
|---|---|---|---|---|---|---|
| TASK-P9-001 | TODO | README를 product truth 기준으로 재작성 | `README.md` | P7 완료 | run/doctor/replay 중심 문서 | docs snapshot |
| TASK-P9-002 | TODO | DEPLOYMENT를 local product 기준으로 축소 | `docs/DEPLOYMENT.md` | P7 완료 | non-core ops 제거 | docs review |
| TASK-P9-003 | TODO | operator runbook 작성 | new docs | P8 완료 | doctor/replay/failure triage 절차 정리 | doc lint |
| TASK-P9-004 | TODO | release rehearsal 수행 | scripts + docs | P8 완료 | RC gate report 생성 | rehearsal report |
| TASK-P9-005 | TODO | rollback rehearsal 수행 | scripts + docs | P8 완료 | rollback trace 검증 | rollback report |
| TASK-P9-006 | TODO | v1.0 RC tag 조건 확정 | release docs | P9-004, P9-005 | tag checklist green | release checklist |

## Recommended next task

가장 먼저 시작할 태스크는 다음 순서가 맞다.

1. `TASK-P1-001`
2. `TASK-P3-001`
3. `TASK-P4-001`
4. `TASK-P5-001`
5. `TASK-P2-003`

즉, backend source와 `run` canonical path를 고정한 뒤 workspace hardening, command hardening, hidden fallback 검증 순으로 잠근다.
