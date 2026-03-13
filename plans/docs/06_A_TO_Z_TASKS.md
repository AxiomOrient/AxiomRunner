# 06. A-to-Z Tasks

상태:
- `TODO`
- `DOING`
- `DONE`
- `BLOCKED`

우선순위:
- `P0`
- `P1`
- `P2`

## A. Truth Lock

| ID | Pri | Status | Task | Files | Done Condition | Verification |
|---|---|---|---|---|---|---|
| AZ-001 | P0 | TODO | README에 `doctor`와 실제 command surface 반영 | `README.md` | README와 CLI surface 일치 | `cargo run -p axonrunner_apps -- --help` |
| AZ-002 | P0 | TODO | README의 잘못된 절대 링크 수정 | `README.md` | 문서 링크가 repo-relative로 동작 | 링크 클릭/manual |
| AZ-003 | P0 | TODO | CHANGELOG를 현재 retained surface로 갱신 | `CHANGELOG.md` | `agent` 언급 제거 | diff review |
| AZ-004 | P1 | TODO | `docs/0*.md`를 archive로 이동하고 현재 source of truth 표기 | `docs/*` | 현재 문서와 과거 blueprint 구분 명확 | tree review |
| AZ-005 | P1 | TODO | DEPLOYMENT를 현재 run/replay/doctor 중심으로 재작성 | `docs/DEPLOYMENT.md` | 운영 문서가 현재 제품면과 일치 | doc review |

## B. Schema Lock

| ID | Pri | Status | Task | Files | Done Condition | Verification |
|---|---|---|---|---|---|---|
| AZ-006 | P0 | TODO | mode canonical string 선정(`read_only` 권장) | `state_store.rs`, `display.rs`, `trace_store.rs` | 모든 표면에서 동일 string 사용 | `cargo test -p axonrunner_apps` |
| AZ-007 | P0 | TODO | legacy snapshot migration 추가 | `state_store.rs` | `readonly`와 신규 문자열 모두 로드 가능 | state store tests |
| AZ-008 | P1 | TODO | revision 의미(event-count) 문서화 | `README.md`, `replay.rs`, docs | operator가 해석 가능 | doc + golden output |
| AZ-009 | P1 | TODO | replay/status/doctor mode 표기 일관화 | `status.rs`, `doctor.rs`, `replay.rs`, `display.rs` | 출력 drift 제거 | e2e |

## C. Evidence Upgrade

| ID | Pri | Status | Task | Files | Done Condition | Verification |
|---|---|---|---|---|---|---|
| AZ-010 | P0 | TODO | patch artifact schema v2 정의 | `tool_write.rs`, `runtime_compose.rs`, `trace_store.rs` | metadata + explainable fields 포함 | unit tests |
| AZ-011 | P0 | TODO | write/replace에 bounded before/after excerpt 추가 | `tool_write.rs` | text mutation evidence 강화 | adapter tests |
| AZ-012 | P1 | TODO | 가능한 경우 unified diff 추가 | `tool_write.rs` | diff artifact 생성 | adapter tests |
| AZ-013 | P1 | TODO | remove_path evidence 추가 | `tool.rs`, `trace_store.rs` | 삭제 경로가 trace/report에 남음 | adapter/apps tests |
| AZ-014 | P1 | TODO | replay가 changed paths와 evidence를 요약 출력 | `replay.rs` | replay UX 개선 | e2e |
| AZ-015 | P1 | TODO | report 템플릿에서 plan/apply/verify/report 연결 강화 | `runtime_compose.rs` | report가 self-contained | golden review |

## D. `codek` Contract Lock

| ID | Pri | Status | Task | Files | Done Condition | Verification |
|---|---|---|---|---|---|---|
| AZ-016 | P0 | TODO | 현재 `codex-runtime` pin과 upstream 권장 버전 차이 문서화 | `README.md`, `docs/DEPLOYMENT.md`, `Cargo.toml` | 운영자가 버전 기대치를 이해 | doctor/manual |
| AZ-017 | P0 | TODO | `doctor`에 codex binary path/version/compatibility 출력 강화 | `doctor.rs`, `provider_codex_runtime.rs` | 설치 문제 진단 가능 | `doctor --json` |
| AZ-018 | P0 | TODO | unsupported version에 대한 blocked/degraded 정책 명시 | `provider_codex_runtime.rs`, docs | 상태 판단 기준 고정 | unit tests |
| AZ-019 | P1 | TODO | session/workspace reuse contract 문서화 | `provider_codex_runtime.rs`, docs | 런타임 의미 명확 | doc review |
| AZ-020 | P1 | TODO | 0.4.x 유지 vs 0.5.x 업그레이드 decision record 작성 | `docs/` | 결정과 이유 명시 | ADR review |

## E. Verification Lock

| ID | Pri | Status | Task | Files | Done Condition | Verification |
|---|---|---|---|---|---|---|
| AZ-021 | P0 | TODO | `plans/TASKS.md` verification 문구와 실제 테스트 트리 정렬 | `plans/TASKS.md`, `crates/*/tests/*` | 계획과 검증이 일치 | tree + cargo test |
| AZ-022 | P0 | TODO | adapter tool integration tests 존재 여부 확정 및 보강 | `crates/adapters/tests/*` | essential tool surface가 테스트됨 | `cargo test -p axonrunner_adapters` |
| AZ-023 | P1 | TODO | e2e corpus를 truth-surface 시나리오 이름으로 재정렬 | `crates/apps/tests/e2e_cli.rs` | 테스트가 제품 계약을 직접 말함 | cargo test |
| AZ-024 | P1 | TODO | blocked provider / workspace boundary / persisted control state golden 추가 | `e2e_cli.rs`, adapter tests | 핵심 계약 회귀 방지 | cargo test |
| AZ-025 | P1 | TODO | release gate에 doc drift 검사 추가 | `release_security_gate.rs` 또는 scripts | README/help/charter drift 감지 | cargo test |

## F. Runtime Hardening

| ID | Pri | Status | Task | Files | Done Condition | Verification |
|---|---|---|---|---|---|---|
| AZ-026 | P0 | TODO | workspace 미결정 시 fail-closed | `runtime_compose.rs` | `/` fallback 제거 | unit/e2e |
| AZ-027 | P1 | TODO | async runtime fallback 정책을 operator-visible로 변경 | `async_runtime_host.rs`, `doctor.rs` | fallback이 숨지 않음 | tests/manual |
| AZ-028 | P1 | TODO | `batch --reset-state` semantics 문서화 | `cli_runtime.rs`, README | reset 범위 오해 제거 | e2e/docs |
| AZ-029 | P1 | TODO | 필요 시 `--reset-trace` / `--reset-artifacts` 설계 검토 | `cli_command.rs`, `cli_runtime.rs` | 결정이 명확 | ADR/doc |
| AZ-030 | P1 | TODO | remove_path / run_command high-risk action trace 강화 | `tool.rs`, `trace_store.rs`, `report` | 위험 작업 설명력 향상 | adapter/apps tests |

## G. Minor Cleanup

| ID | Pri | Status | Task | Files | Done Condition | Verification |
|---|---|---|---|---|---|---|
| AZ-031 | P2 | TODO | markdown memory header의 `ZeroClaw` 잔재 제거 | `memory_markdown.rs` | 브랜딩 정리 | unit tests |
| AZ-032 | P2 | TODO | experimental OpenAI provider 유지 여부 결정 | `provider_openai.rs`, docs | 유지/격리/삭제 결정 | ADR |
| AZ-033 | P2 | TODO | 유지 시 blocking reqwest 제거 또는 분리 | `provider_openai.rs` | async bridge 일관성 | tests |

## H. Release Readiness

| ID | Pri | Status | Task | Files | Done Condition | Verification |
|---|---|---|---|---|---|---|
| AZ-034 | P1 | TODO | install/build/doctor/run/replay runbook 정리 | docs | 신규 사용자가 따라갈 수 있음 | clean-machine rehearsal |
| AZ-035 | P1 | TODO | minimal example scenario 추가 | README, examples or docs | 사용 예시가 실제 제품과 일치 | manual |
| AZ-036 | P1 | TODO | release checklist 작성 | docs/plans | 릴리즈 gate 명확 | checklist review |
| AZ-037 | P2 | TODO | versioning/changelog policy 문서화 | CHANGELOG, docs | 기록 체계 고정 | doc review |

## 추천 순서

1. AZ-001 ~ AZ-007
2. AZ-010 ~ AZ-018
3. AZ-021 ~ AZ-026
4. AZ-027 ~ AZ-037

## 종료 기준

다음 네 가지가 동시에 만족되면 이번 루프는 닫을 수 있다.

1. README / help / charter / doctor / changelog가 같은 제품을 말한다.
2. mode/state/trace schema drift가 없다.
3. replay/report가 changed paths와 evidence를 operator-grade로 보여준다.
4. `codek` 설치/버전/호환성 문제가 `doctor` 하나로 진단된다.
