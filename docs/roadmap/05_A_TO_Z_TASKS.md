# 05. A to Z Tasks

상태 값:
- `TODO`
- `DOING`
- `DONE`
- `BLOCKED`

우선순위 기준:
- `P0`: 제품 완성도와 안전성의 직접 blocker
- `P1`: 강한 품질/운영성 개선
- `P2`: 후순위 또는 polish

| ID | Phase | Priority | Status | Task | Files | Done Condition | Verification |
|---|---|---|---|---|---|---|---|
| AR-001 | P0 | P0 | TODO | autonomous product spec 추가 | `README.md; docs/project-charter.md; docs/CAPABILITY_MATRIX.md` | public surface를 goal/run 중심으로 재정의 | `docs truth-surface review + release gate` |
| AR-002 | P0 | P0 | TODO | run contract 문서화 | `docs/AUTONOMOUS_AGENT_SPEC.md` | goal/done/budget/approval schema가 문서화됨 | `doc review` |
| AR-003 | P0 | P1 | TODO | release gate를 새 surface에 맞게 수정 | `crates/apps/tests/release_security_gate.rs` | README/help/docs/charter 불일치 차단 | `cargo test -p axonrunner_apps --test release_security_gate` |
| AR-004 | P1 | P0 | TODO | goal/run domain type 추가 | `crates/core/src/intent.rs; crates/core/src/state.rs; crates/core/src/event.rs` | RunGoal/RunPhase/RunOutcome 도입 | `cargo test -p axonrunner_core` |
| AR-005 | P1 | P0 | TODO | goal lifecycle reducer/projection 구현 | `crates/core/src/reducer.rs; crates/core/src/projection.rs` | plan/execute/verify/repair state 전이가 표현됨 | `cargo test -p axonrunner_core` |
| AR-006 | P1 | P1 | TODO | legacy fact intent를 compatibility shim으로 격리 | `crates/core/src/intent.rs; crates/apps/src/cli_command.rs` | legacy alias가 public truth를 흐리지 않음 | `e2e migration tests` |
| AR-007 | P1 | P1 | TODO | run invariant 테스트 추가 | `crates/core/tests/state_invariants.rs; crates/core/tests/domain_flow.rs` | budget/approval/terminal state invariant 잠금 | `cargo test -p axonrunner_core` |
| AR-008 | P2 | P0 | TODO | CLI를 goal 중심으로 전환 | `crates/apps/src/cli_command.rs; crates/apps/src/cli_args.rs` | run/resume/abort/status/replay/doctor surface 고정 | `cargo test -p axonrunner_apps --test e2e_cli` |
| AR-009 | P2 | P1 | TODO | help/display 출력 재정리 | `crates/apps/src/display.rs; crates/apps/src/cli_command.rs` | operator-facing usage가 concise하고 정확함 | `golden output tests` |
| AR-010 | P2 | P1 | TODO | legacy alias를 hidden/debug path로 전환 | `crates/apps/src/cli_command.rs; README.md` | public docs에서 legacy 비중 축소 | `doc + help review` |
| AR-011 | P3 | P0 | TODO | planner 단계 구현 | `crates/apps/src/cli_runtime.rs; crates/apps/src/runtime_compose.rs` | goal→bounded plan 생성 | `planner unit tests` |
| AR-012 | P3 | P0 | TODO | executor 단계 구현 | `crates/apps/src/cli_runtime.rs; crates/apps/src/runtime_compose.rs` | 한 step씩 tool/provider 실행 | `e2e step execution tests` |
| AR-013 | P3 | P0 | TODO | verifier 단계 구현 | `crates/apps/src/cli_runtime.rs; crates/apps/src/runtime_compose.rs` | done_when / verification contract 실행 | `verification golden tests` |
| AR-014 | P3 | P0 | TODO | repair loop 구현 | `crates/apps/src/cli_runtime.rs` | verify 실패 시 repair step 재시도 | `fault injection tests` |
| AR-015 | P3 | P0 | TODO | terminal outcomes 정리 | `crates/apps/src/cli_runtime.rs; crates/apps/src/status.rs` | success/failed/blocked/approval_required/aborted 분리 | `status + replay tests` |
| AR-016 | P3 | P1 | TODO | run-id 및 step-id schema 고정 | `crates/apps/src/state_store.rs; crates/apps/src/trace_store.rs` | resume/replay 가능한 stable ids | `trace schema tests` |
| AR-017 | P4 | P0 | TODO | run-scoped workspace binding 강화 | `crates/adapters/src/provider_codex_runtime.rs; crates/apps/src/runtime_compose.rs` | provider cwd가 명시적 workspace/worktree에 묶임 | `provider integration tests` |
| AR-018 | P4 | P0 | TODO | codex compatibility probe 강화 | `crates/adapters/src/provider_codex_runtime.rs; crates/apps/src/doctor.rs` | doctor가 binary/version/compatibility를 일관되게 노출 | `doctor tests` |
| AR-019 | P4 | P1 | TODO | session reuse contract 테스트 추가 | `crates/adapters/src/provider_codex_runtime.rs` | cwd/model 변경 시 재사용 금지 보장 | `provider unit tests` |
| AR-020 | P4 | P1 | TODO | git worktree isolation 옵션 도입 | `crates/apps/src/runtime_compose.rs; crates/adapters/src/tool.rs` | run마다 reviewable diff isolation 가능 | `git repo e2e tests` |
| AR-021 | P5 | P0 | TODO | tool contract를 public schema로 잠금 | `crates/adapters/src/contracts.rs; docs/CAPABILITY_MATRIX.md` | tool input/output/evidence schema 명시 | `contract tests` |
| AR-022 | P5 | P0 | TODO | list/read/search/write/replace/remove/run-command 전체 완성도 점검 | `crates/adapters/src/tool.rs; crates/adapters/src/tool_workspace.rs` | essential tool surface end-to-end green | `cargo test -p axonrunner_adapters --test tool` |
| AR-023 | P5 | P0 | TODO | run_command verifier profiles 추가 | `crates/adapters/src/tool.rs; crates/apps/src/runtime_compose.rs` | build/test/lint runner contract 표준화 | `command verifier tests` |
| AR-024 | P5 | P1 | TODO | patch evidence를 digest+excerpt 수준으로 강화 | `crates/adapters/src/tool.rs; crates/apps/src/replay.rs` | operator가 diff 의미를 바로 읽을 수 있음 | `replay golden tests` |
| AR-025 | P5 | P1 | TODO | high-risk operations risk tier 정의 | `crates/adapters/src/tool.rs; docs/RUNBOOK.md` | remove/large replace/dangerous command 분류 | `risk policy tests` |
| AR-026 | P6 | P0 | TODO | working memory vs recall memory 분리 | `crates/apps/src/runtime_compose.rs; crates/adapters/src/memory.rs` | hot context와 durable recall이 분리됨 | `memory integration tests` |
| AR-027 | P6 | P1 | TODO | artifact index 추가 | `crates/apps/src/trace_store.rs; crates/apps/src/replay.rs` | plan/apply/verify/report/patch artifact 빠른 조회 | `replay tests` |
| AR-028 | P6 | P1 | TODO | context compaction 규칙 구현 | `crates/apps/src/cli_runtime.rs; crates/apps/src/runtime_compose.rs` | 긴 run에서도 context budget 유지 | `long-run simulation` |
| AR-029 | P6 | P1 | TODO | AGENTS.md ingestion 추가 | `crates/apps/src/runtime_compose.rs; crates/apps/src/doctor.rs` | durable repo guidance를 run context에 반영 | `fixture repo tests` |
| AR-030 | P7 | P0 | TODO | approval policy 구현 | `crates/apps/src/cli_command.rs; crates/apps/src/cli_runtime.rs` | never/on-risk/always 동작 | `approval e2e tests` |
| AR-031 | P7 | P0 | TODO | budget policy 구현 | `crates/apps/src/cli_runtime.rs; crates/core/src/state.rs` | step/time/token budget 강제 | `budget exhaustion tests` |
| AR-032 | P7 | P0 | TODO | resume semantics 구현 | `crates/apps/src/state_store.rs; crates/apps/src/cli_runtime.rs; crates/apps/src/status.rs` | 중단된 run 재개 가능 | `resume e2e tests` |
| AR-033 | P7 | P0 | TODO | abort semantics 구현 | `crates/apps/src/cli_runtime.rs; crates/apps/src/status.rs` | operator abort가 안전하게 종료됨 | `abort e2e tests` |
| AR-034 | P7 | P1 | TODO | rollback/checkpoint contract 추가 | `crates/apps/src/runtime_compose.rs; crates/apps/src/replay.rs` | 고위험 변경 전 복원 가능 지점 확보 | `checkpoint tests` |
| AR-035 | P8 | P0 | TODO | autonomy golden corpus 설계 | `crates/apps/tests/e2e_cli.rs; crates/adapters/tests/tool.rs` | 대표 작업군 시나리오 잠금 | `golden suite` |
| AR-036 | P8 | P0 | TODO | false-success / false-done metric 추가 | `crates/apps/tests/e2e_cli.rs; crates/apps/src/replay.rs` | 성공 판정 오류를 계량화 | `eval report` |
| AR-037 | P8 | P0 | TODO | blocked/degraded provider path eval 추가 | `crates/adapters/tests/*; crates/apps/tests/e2e_cli.rs` | substrate failure가 안전하게 노출 | `provider failure tests` |
| AR-038 | P8 | P1 | TODO | nightly dogfood workflow 추가 | `GitHub Actions; docs/RUNBOOK.md` | 실사용 회귀 점검 자동화 | `CI/nightly run` |
| AR-039 | P8 | P1 | TODO | release gate를 autonomy 기준으로 갱신 | `docs/CAPABILITY_MATRIX.md; release tests` | release blocker가 autonomy 중심으로 재정의 | `release gate tests` |
| AR-040 | P8 | P1 | TODO | example repos와 operator runbook 보강 | `docs/RUNBOOK.md; examples/*` | 실제 사용 진입장벽 축소 | `manual smoke + docs review` |