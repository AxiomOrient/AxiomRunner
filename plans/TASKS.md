# TASKS

## 사용 규칙
- 상태값: `todo | doing | blocked | done`
- 우선순위: `P0 | P1 | P2 | P3`
- 분류: `release | architecture | observability | docs | optional`
- blocker: `yes | no`

## 실행 결과

- 현재 cycle 기준으로 active task는 모두 완료했다.
- 핵심 수정은 누락된 current/bridge 문서 복구와 tool contract 문서 토큰 보강이다.
- 후속 수정으로 스펙 대비 부족했던 `doctor` 필드, `replay next_action`, `plan artifact` 명시 필드, `approval_escalation` 정책 가시성을 구현 쪽에 반영했다.
- 최종 검증 증거:
  - `cargo test -p axiomrunner_apps`
  - `cargo test -p axiomrunner_adapters`

---

## P0 — must-fix before release

| ID | 상태 | 우선순위 | 분류 | blocker | 작업 | 왜 중요한가 | 줄이는 위험 | 수정 대상 | 완료 증거 | 검증 |
|---|---|---:|---|---|---|---|---|---|---|---|
| T-001 | done | P0 | release | yes | retained CLI surface truth matrix 고정 | public surface drift는 제품 의미 drift다 | CLI/docs/test 불일치 | `README.md`, `docs/project-charter.md`, `docs/CAPABILITY_MATRIX.md`, `docs/RUNBOOK.md`, `crates/apps/src/cli_command.rs`, `crates/apps/tests/release_security_gate.rs` | retained surface string과 command set이 한 벌로 잠김 | `cargo test -p axiomrunner_apps --test release_security_gate` |
| T-002 | done | P0 | release | yes | identity truth lock (`AxiomRunner` / `axiomrunner_apps` / `AXIOMRUNNER_*`) | 이름 drift는 operator/build/env drift를 부른다 | packaging/CI/env 혼선 | workspace + crate manifests, docs, scripts, release gate | legacy token 금지 + naming truth gate 통과 | `cargo test -p axiomrunner_apps --test release_security_gate` |
| T-003 | done | P0 | release | yes | verifier fallback semantics 고정 | verify-before-done이 제품 핵심이다 | false success | `runtime_compose/plan.rs`, `cli_runtime/lifecycle.rs`, `operator_render.rs`, `e2e_cli.rs`, `autonomous_eval_corpus.rs` | `weak/unresolved/pack_required`가 모두 blocked | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-004 | done | P0 | release | yes | placeholder verifier 금지 gate 강화 | placeholder probe가 success 신호로 읽히면 안 된다 | 형식적 green | `release_security_gate.rs`, examples/packs | placeholder verifier 없는 상태 고정 | `cargo test -p axiomrunner_apps --test release_security_gate` |
| T-005 | done | P0 | release | yes | `path_scope` enforcement path 고정 | constraints가 실제 policy를 바꿔야 한다 | safety expectation mismatch | `core/intent.rs`, `apps/runtime_compose.rs`, `e2e_cli.rs` | verifier command가 scope 밖이면 blocked_by_policy | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-006 | done | P0 | release | yes | `destructive_commands=deny` enforcement | destructive verifier 차단은 safety 핵심 | destructive command 실행 리스크 | `apps/runtime_compose.rs`, `e2e_cli.rs`, `operator_render.rs` | policy code / reason visible | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-007 | done | P0 | release | yes | `external_commands=deny` enforcement | 외부 command class 차단이 workspace-bounded 의미를 지킨다 | unbounded execution | `apps/runtime_compose.rs`, `e2e_cli.rs`, `operator_render.rs` | external command blocked | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-008 | done | P0 | release | yes | `approval_escalation=required` + high-risk verifier path 고정 | on-risk approval semantics를 실제 risk path에 연결해야 한다 | unsafe mutation without approval | `runtime_compose.rs`, `cli_runtime/lifecycle.rs`, `e2e_cli.rs` | pre-execution approval required visibility | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-009 | done | P0 | release | yes | done condition truth lock 강화 | success는 verification뿐 아니라 done evidence까지 닫아야 한다 | false done | `cli_runtime/lifecycle.rs`, `trace_store.rs`, `e2e_cli.rs` | `false_done_intents` 계산과 report/replay visibility 고정 | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-010 | done | P0 | release | yes | report write failure -> run failure path 잠금 | artifact 없이는 operator trust가 무너진다 | silent incomplete run | `cli_runtime.rs`, `runtime_compose/artifacts.rs`, `e2e_cli.rs` | report failure가 `failed`로 승격 | `cargo test -p axiomrunner_apps --test e2e_cli` |

---

## P1 — architecture cleanup

| ID | 상태 | 우선순위 | 분류 | blocker | 작업 | 왜 중요한가 | 줄이는 위험 | 수정 대상 | 완료 증거 | 검증 |
|---|---|---:|---|---|---|---|---|---|---|---|
| T-011 | done | P1 | architecture | no | pending run snapshot round-trip lock | status/doctor/resume 의미가 snapshot과 같아야 한다 | control state drift | `state_store.rs`, `status.rs`, `doctor.rs`, `e2e_cli.rs` | snapshot->load->status/doctor 동일 필드 | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-012 | done | P1 | architecture | no | status / doctor / replay / report vocabulary 정규화 | operator output drift 방지 | support/debug confusion | `operator_render.rs`, `doctor.rs`, `status.rs`, report writer | 동일 key vocabulary | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-013 | done | P1 | architecture | no | workflow pack source of truth 단일화 유지 | 중복 계약 문서 drift 방지 | docs drift | `docs/README.md`, `docs/WORKFLOW_PACK_CONTRACT.md`, `release_security_gate.rs` | authoritative source 1개만 존재 | `cargo test -p axiomrunner_apps --test release_security_gate` |
| T-014 | done | P1 | architecture | no | reason_code / reason_detail normalization 강화 | report/replay analytics를 안정화 | parsing ambiguity | `runtime_compose.rs`, `operator_render.rs`, tests | reason code/detail stable | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-015 | done | P1 | architecture | no | trace corruption/read-while-write matrix 강화 | append-only trace 신뢰성 확보 | corrupted replay | `trace_store.rs`, `fault_path_suite.rs` | partial-line recover / malformed fail 고정 | `cargo test -p axiomrunner_apps --test fault_path_suite` |

---

## P1 — operator experience / observability

| ID | 상태 | 우선순위 | 분류 | blocker | 작업 | 왜 중요한가 | 줄이는 위험 | 수정 대상 | 완료 증거 | 검증 |
|---|---|---:|---|---|---|---|---|---|---|---|
| T-016 | done | P1 | observability | no | replay changed_paths / patch_artifact readability 개선 | operator가 무엇이 바뀌었는지 바로 이해해야 한다 | post-run ambiguity | `operator_render.rs`, `runtime_compose/artifacts.rs` | replay/report가 동일 changed-path/evidence semantics 제공 | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-017 | done | P1 | observability | no | checkpoint / rollback metadata contract 강화 | isolated worktree 운영을 닫는다 | recovery failure | `runtime_compose/artifacts.rs`, `operator_render.rs`, `e2e_cli.rs`, `RUNBOOK.md` | metadata fields + replay/report docs 일치 | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-018 | done | P1 | observability | no | doctor lock/path/provider detail 강화 | run 전 readiness 판단의 정확성 향상 | bad runtime diagnosis | `doctor.rs`, `operator_render.rs`, `e2e_cli.rs` | doctor가 lock/path/provider detail 모두 노출 | `cargo test -p axiomrunner_apps --test e2e_cli` |
| T-019 | done | P1 | observability | no | nightly summary metrics를 release truth로 승격 | CI green보다 운영 green이 중요 | hidden regressions | `nightly_dogfood_contract.rs`, `scripts/nightly_dogfood.sh`, `RUNBOOK.md` | nightly summary quality metrics 0 lock | `cargo test -p axiomrunner_apps --test nightly_dogfood_contract` |
| T-020 | done | P1 | observability | no | representative examples와 fixtures 동기화 | examples가 실제 operator asset이어야 한다 | docs/example drift | `examples/*`, fixtures, `README.md`, `autonomous_eval_corpus.rs` | example goal/pack이 tests와 같은 contract를 말함 | `cargo test -p axiomrunner_apps --test autonomous_eval_corpus` |

---

## P2 — docs truth lock

| ID | 상태 | 우선순위 | 분류 | blocker | 작업 | 왜 중요한가 | 줄이는 위험 | 수정 대상 | 완료 증거 | 검증 |
|---|---|---:|---|---|---|---|---|---|---|---|
| T-021 | done | P2 | docs | no | current truth docs와 bridge docs 경계 문장 강화 | target 문서가 current truth를 덮어쓰지 않게 함 | release misunderstanding | `docs/README.md`, `AUTONOMOUS_AGENT_TARGET.md`, `AUTONOMOUS_AGENT_SPEC.md` | current truth 우선 규칙 명시 | `cargo test -p axiomrunner_apps --test release_security_gate` |
| T-022 | done | P2 | docs | no | CAPABILITY_MATRIX release blocker 항목과 test matrix 동기화 | 문서 blocker와 실제 gate를 맞춤 | paper contract only | `docs/CAPABILITY_MATRIX.md`, `release_security_gate.rs` | blocker 항목마다 대응 test 존재 | gate suite |
| T-023 | done | P2 | docs | no | RUNBOOK recovery 절차 상세화 | blocked/failed/rollback operator playbook 고도화 | operator error | `docs/RUNBOOK.md` | rollback/checkpoint/lock/manual cleanup 문서화 | docs review + e2e reference |
| T-024 | done | P2 | docs | no | PROJECT_STRUCTURE를 현재 코드와 지속 정합 | 구조 문서 drift 방지 | onboarding confusion | `docs/PROJECT_STRUCTURE.md` | modules/files/current ownership 일치 | release gate docs check |

---

## P3 — later / optional

| ID | 상태 | 우선순위 | 분류 | blocker | 작업 | 왜 중요한가 | 줄이는 위험 | 수정 대상 | 완료 증거 | 검증 |
|---|---|---:|---|---|---|---|---|---|---|---|
| T-025 | done | P3 | optional | no | representative domain pack 확대 | 도메인 coverage를 넓힌다 | pack_required run 감소 | examples/packs | 4개 대표 pack 이상 안정 운영 | eval/nightly |
| T-026 | done | P3 | optional | no | additional provider compat hardening | substrate 선택지를 늘린다 | provider monoculture | adapters providers | experimental remains isolated | provider tests |
| T-027 | done | P3 | optional | no | richer replay diff/indexing | 큰 run의 replay usability 향상 | replay scale issues | trace/replay modules | bounded index strategy | future test |

---

## 자기 피드백

- 가장 큰 실제 문제는 코드 미구현보다 **문서 truth 누락**이었다.
- `release_security_gate`와 `adapters` 계약 테스트가 이미 강해서, 비어 있던 bridge/versioning 문서를 채우자 전체 계약이 바로 닫혔다.
- 추가 개선으로 `docs/CAPABILITY_MATRIX.md`에 tool contract 핵심 토큰을 보강해 adapters 문서 테스트까지 맞췄다.
- 그 다음 cycle에서는 스펙이 요구한 operator/evidence 계약이 실제 출력에 없던 부분을 구현으로 끌어올렸다.
- 즉 이번 작업의 개선 포인트는 **문서-테스트-코드의 단일 truth 회복 + 스펙 부족 구현 보강**이었다.

## 완료 순서

1. 누락된 `docs/AUTONOMOUS_AGENT_TARGET.md`, `docs/AUTONOMOUS_AGENT_SPEC.md`, `docs/VERSIONING.md` 복구
2. `README.md`, `docs/README.md`, `docs/PROJECT_STRUCTURE.md` 문서 입구 정리
3. `docs/CAPABILITY_MATRIX.md` tool contract 토큰 보강
4. `cargo test -p axiomrunner_apps`
5. `cargo test -p axiomrunner_adapters`

---

## 명시적으로 지금 하지 않을 일

- public command 추가
- daemon/service/gateway 재도입
- multi-agent 도입
- channel integration
- marketplace / integrations catalog
- generalized memory platform 확장
- release 전 broad provider 확대
