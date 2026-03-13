# 03. Gap Catalog

## P0

| ID | 문제 | 영향 | 파일 | 조치 |
|---|---|---|---|---|
| GAP-001 | README / CHANGELOG / CLI surface drift | 사용자가 실제 제품면을 오해할 수 있음 | `README.md`, `CHANGELOG.md`, `crates/apps/src/cli_command.rs` | truth lock |
| GAP-002 | `readonly` vs `read_only` mode naming drift | snapshot / display / trace 해석 혼선 | `state_store.rs`, `display.rs`, `trace_store.rs` | schema 통일 + migration |
| GAP-003 | `codek` substrate compatibility drift | 설치/운영 환경에서 예기치 않은 실패 가능 | `crates/adapters/Cargo.toml`, `provider_codex_runtime.rs`, docs | version pin + compatibility matrix + doctor |
| GAP-004 | patch evidence가 metadata 중심 | “무엇이 바뀌었는가” 설명력이 부족 | `tool_write.rs`, `runtime_compose.rs`, `trace_store.rs`, `replay.rs` | diff-grade evidence |
| GAP-005 | plans/TASKS와 adapter tool tests 구성 불일치 | 검증 신뢰도 저하 | `plans/TASKS.md`, `crates/adapters/tests/*` | 문서/테스트 정합화 |

## P1

| ID | 문제 | 영향 | 파일 | 조치 |
|---|---|---|---|---|
| GAP-006 | `runtime_compose`의 workspace/cwd fallback이 너무 관대 | 잘못된 cwd에서 provider 실행 가능성 | `runtime_compose.rs` | fail-closed |
| GAP-007 | `async_runtime_host` fallback 정책이 약간 암묵적 | 실패 원인 파악성 저하 | `async_runtime_host.rs` | operator-visible policy |
| GAP-008 | `batch --reset-state` 범위가 직관과 다를 수 있음 | state/trace/log 기대 차이 | `cli_runtime.rs`, `trace_store.rs`, docs | semantics 명시 or 확장 |
| GAP-009 | `revision`의 의미가 사용자에게 충분히 설명되지 않음 | replay/status 해석 혼동 | `reducer.rs`, README, doctor/replay docs | 문서화 |

## P2

| ID | 문제 | 영향 | 파일 | 조치 |
|---|---|---|---|---|
| GAP-010 | markdown memory header에 ZeroClaw 잔재 | 브랜딩/신뢰도 저하 | `memory_markdown.rs` | 문자열 수정 |
| GAP-011 | experimental OpenAI provider가 blocking client 기반 | 성능/구현 일관성 저하 | `provider_openai.rs` | 유지 시 하드닝 |
| GAP-012 | legacy blueprint docs가 root docs에 남아 있음 | 현재 제품 이해 방해 | `docs/0*.md` | archive 분리 |

## 우선순위 원칙

1. 사용자 진실 표면을 먼저 잠근다.
2. 상태/trace schema를 통일한다.
3. evidence와 replay 설명력을 올린다.
4. `codek` 운영 계약을 고정한다.
5. 그 뒤에만 부가 개선을 한다.
