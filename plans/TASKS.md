# AxonRunner Autonomous Transition Tasks

Source inputs:
- `docs/roadmap/03_AUTONOMOUS_ROADMAP.md`
- `docs/roadmap/04_PHASED_IMPLEMENTATION_PLAN.md`
- `docs/roadmap/05_A_TO_Z_TASKS.md`
- current repo truth surface and bridge docs as of 2026-03-13

Status values:
- `TODO`
- `DOING`
- `DONE`
- `BLOCKED`

Execution rule:
- do not start a row until all `Depends On` items are complete and the listed evidence can actually be produced

## Phase P0

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| AR-001 | DONE | Add the autonomous product spec bridge to the root truth surface. | `README.md`; `docs/project-charter.md`; `docs/CAPABILITY_MATRIX.md` | root docs redefine the transition target in goal/run terms without lying about current behavior | docs truth-surface review + release gate | - |
| AR-002 | DONE | Document the run contract. | `docs/AUTONOMOUS_AGENT_SPEC.md` | goal, done, budget, approval, terminal outcomes, and replay evidence schemas are documented | doc review | AR-001 |
| AR-003 | DONE | Update the release gate for the new truth surface. | `crates/apps/tests/release_security_gate.rs` | README/help/docs/charter mismatches are rejected by tests | `cargo test -p axonrunner_apps --test release_security_gate` | AR-001, AR-002 |

## Phase P1

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| AR-004 | DONE | Add goal/run domain types. | `crates/core/src/intent.rs`; `crates/core/src/state.rs`; `crates/core/src/event.rs` | `RunGoal`, `RunPhase`, `RunOutcome`, budget, and approval vocabulary exist in `core` | `cargo test -p axonrunner_core` | AR-001, AR-002 |
| AR-005 | DONE | Implement goal lifecycle reducer and projection behavior. | `crates/core/src/reducer.rs`; `crates/core/src/projection.rs` | plan, execute, verify, repair, and terminal transitions are projected from events | `cargo test -p axonrunner_core` | AR-004 |
| AR-006 | DONE | Isolate legacy fact intents behind a compatibility shim. | `crates/core/src/intent.rs`; `crates/apps/src/cli_command.rs` | legacy aliases still work without defining the new public truth | e2e migration tests | AR-004 |
| AR-007 | DONE | Add run invariants and lifecycle regression tests. | `crates/core/tests/state_invariants.rs`; `crates/core/tests/domain_flow.rs` | budget, approval, and terminal-state invariants are locked in tests | `cargo test -p axonrunner_core` | AR-004, AR-005, AR-006 |

## Phase P2

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| AR-008 | BLOCKED | Transition the CLI to a goal/run-first surface. | `crates/apps/src/cli_command.rs`; `crates/apps/src/cli_args.rs` | `run`, `resume`, `abort`, `status`, `replay`, `doctor` are the canonical public surface | `cargo test -p axonrunner_apps --test e2e_cli` | AR-004, AR-006, AR-007 |
| AR-009 | TODO | Rewrite help and display output around the new operator surface. | `crates/apps/src/display.rs`; `crates/apps/src/cli_command.rs` | operator-facing usage text is concise, accurate, and goal/run-centric | golden output tests | AR-008 |
| AR-010 | TODO | Demote legacy aliases to hidden or debug-only paths. | `crates/apps/src/cli_command.rs`; `README.md` | public docs stop presenting legacy commands as the main product | doc + help review | AR-008, AR-009 |

## Phase P3

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| AR-011 | DONE | Implement the planner stage. | `crates/apps/src/cli_runtime.rs`; `crates/apps/src/runtime_compose.rs` | goals are turned into bounded execution plans with explicit step boundaries | planner unit tests | AR-005, AR-008 |
| AR-012 | DONE | Implement the executor stage. | `crates/apps/src/cli_runtime.rs`; `crates/apps/src/runtime_compose.rs` | steps execute provider and tool work one step at a time | e2e step execution tests | AR-011 |
| AR-013 | DONE | Implement the verifier stage. | `crates/apps/src/cli_runtime.rs`; `crates/apps/src/runtime_compose.rs` | `done_when` and verification contracts run separately from execution | verification golden tests | AR-011, AR-012 |
| AR-014 | DONE | Implement the repair loop. | `crates/apps/src/cli_runtime.rs` | verifier failures feed a bounded repair step instead of a false success | fault injection tests | AR-012, AR-013 |
| AR-015 | DONE | Normalize autonomous terminal outcomes. | `crates/apps/src/cli_runtime.rs`; `crates/apps/src/status.rs` | `success`, `failed`, `blocked`, `approval_required`, and `aborted` are distinct and replayable | status + replay tests | AR-011, AR-012, AR-013, AR-014 |
| AR-016 | DONE | Lock the run-id and step-id schema. | `crates/apps/src/state_store.rs`; `crates/apps/src/trace_store.rs` | stable ids support resume, replay, and artifact lookup | trace schema tests | AR-011, AR-015 |

## Phase P4

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| AR-017 | DONE | Harden run-scoped workspace binding. | `crates/adapters/src/provider_codex_runtime.rs`; `crates/apps/src/runtime_compose.rs` | provider cwd is bound to an explicit workspace or worktree per run | provider integration tests | AR-012 |
| AR-018 | DONE | Strengthen the Codex compatibility probe. | `crates/adapters/src/provider_codex_runtime.rs`; `crates/apps/src/doctor.rs` | `doctor` exposes binary, version, and compatibility consistently | doctor tests | AR-017 |
| AR-019 | DONE | Add session reuse contract tests. | `crates/adapters/src/provider_codex_runtime.rs` | changing `cwd` or `model` blocks reuse as documented | provider unit tests | AR-017 |
| AR-020 | BLOCKED | Add optional git worktree isolation. | `crates/apps/src/runtime_compose.rs`; `crates/adapters/src/tool.rs` | runs can opt into reviewable diff isolation | git repo e2e tests | AR-017 |

## Phase P5

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| AR-021 | DONE | Lock the tool contract as a public schema. | `crates/adapters/src/contracts.rs`; `docs/CAPABILITY_MATRIX.md` | tool input, output, and evidence schemas are explicit and versionable | contract tests | AR-013, AR-017 |
| AR-022 | DONE | Complete the essential tool surface. | `crates/adapters/src/tool.rs`; `crates/adapters/src/tool_workspace.rs` | list, read, search, write, replace, remove, and run-command pass end-to-end contract checks | `cargo test -p axonrunner_adapters --test tool` | AR-021 |
| AR-023 | DONE | Add `run_command` verifier profiles. | `crates/adapters/src/tool.rs`; `crates/apps/src/runtime_compose.rs` | build, test, lint, and file-assertion verification modes are standardized | command verifier tests | AR-013, AR-021, AR-022 |
| AR-024 | DONE | Improve patch evidence to digest + excerpt level. | `crates/adapters/src/tool.rs`; `crates/apps/src/replay.rs` | operator replay shows what changed without opening raw diffs first | replay golden tests | AR-022 |
| AR-025 | DONE | Define high-risk operation tiers. | `crates/adapters/src/tool.rs`; `docs/RUNBOOK.md` | `remove_path`, broad replace, and dangerous commands are risk-classified | risk policy tests | AR-022 |

## Phase P6

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| AR-026 | DONE | Split working memory from recall memory. | `crates/apps/src/runtime_compose.rs`; `crates/adapters/src/memory.rs` | hot context and durable recall are stored and loaded separately | memory integration tests | AR-013, AR-017 |
| AR-027 | DONE | Add an artifact index. | `crates/apps/src/trace_store.rs`; `crates/apps/src/replay.rs` | plan/apply/verify/report/patch artifacts can be located quickly from replay/status paths | replay tests | AR-016, AR-024 |
| AR-028 | DONE | Implement context compaction rules. | `crates/apps/src/cli_runtime.rs`; `crates/apps/src/runtime_compose.rs` | long runs keep only the needed hot context within budget | long-run simulation | AR-026 |
| AR-029 | DONE | Ingest `AGENTS.md` as durable repo guidance. | `crates/apps/src/runtime_compose.rs`; `crates/apps/src/doctor.rs` | repo-level instructions enter run context predictably and visibly | fixture repo tests | AR-026 |

## Phase P7

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| AR-030 | TODO | Implement approval policy. | `crates/apps/src/cli_command.rs`; `crates/apps/src/cli_runtime.rs` | `never`, `on-risk`, and `always` modes behave as declared | approval e2e tests | AR-015, AR-025 |
| AR-031 | TODO | Implement budget policy. | `crates/apps/src/cli_runtime.rs`; `crates/core/src/state.rs` | step, time, and token budgets are enforced by runtime and state | budget exhaustion tests | AR-005, AR-015 |
| AR-032 | TODO | Implement resume semantics. | `crates/apps/src/state_store.rs`; `crates/apps/src/cli_runtime.rs`; `crates/apps/src/status.rs` | interrupted runs can resume from stable ids and stored state | resume e2e tests | AR-016, AR-031 |
| AR-033 | TODO | Implement abort semantics. | `crates/apps/src/cli_runtime.rs`; `crates/apps/src/status.rs` | operator abort exits safely and reports a terminal outcome | abort e2e tests | AR-015, AR-031 |
| AR-034 | TODO | Add rollback and checkpoint semantics. | `crates/apps/src/runtime_compose.rs`; `crates/apps/src/replay.rs` | high-risk work can capture and restore safe checkpoints | checkpoint tests | AR-020, AR-030, AR-033 |

## Phase P8

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| AR-035 | TODO | Design the autonomy golden corpus. | `crates/apps/tests/e2e_cli.rs`; `crates/adapters/tests/tool.rs` | representative long-horizon tasks are locked as regression scenarios | golden suite | AR-015, AR-022, AR-030 |
| AR-036 | TODO | Add false-success and false-done metrics. | `crates/apps/tests/e2e_cli.rs`; `crates/apps/src/replay.rs` | success classification errors are measurable and reported | eval report | AR-013, AR-015, AR-035 |
| AR-037 | TODO | Add blocked/degraded provider-path evals. | `crates/adapters/tests/*`; `crates/apps/tests/e2e_cli.rs` | substrate failures surface as safe, explicit outcomes | provider failure tests | AR-018, AR-035 |
| AR-038 | TODO | Add nightly dogfood workflow. | `GitHub Actions`; `docs/RUNBOOK.md` | recurring real-use regression runs exist and are reviewable | CI/nightly run | AR-035, AR-037 |
| AR-039 | TODO | Update the release gate to autonomy criteria. | `docs/CAPABILITY_MATRIX.md`; release tests | release blockers are defined by autonomy evidence instead of intent-runtime-only checks | release gate tests | AR-036, AR-037, AR-038 |
| AR-040 | TODO | Add example repos and strengthen the operator runbook. | `docs/RUNBOOK.md`; `examples/*` | onboarding and real-use docs match the shipped autonomous surface | manual smoke + docs review | AR-039 |

## Immediate Build Slice

The first execution slice should close the minimum transition foundation before any broad runtime rewrite starts:

1. `AR-001` through `AR-003`
2. `AR-004` through `AR-007`
3. `AR-011`, `AR-015`, `AR-017`, `AR-018`, `AR-030`, `AR-031`

That slice locks the product contract, the core run model, the first honest loop skeleton, the provider/workspace substrate, and the operator-control primitives that keep autonomy from becoming a false promise.

## Progress Log

- AR-001 — DONE — `README.md`, `docs/project-charter.md`, and `docs/CAPABILITY_MATRIX.md` now reference the transition bridge and target contract without changing the current truth surface.
- AR-002 — DONE — `docs/AUTONOMOUS_AGENT_SPEC.md` was added with goal, done condition, budget, approval, terminal outcome, and replay evidence sections.
- AR-003 — DONE — `cargo test -p axonrunner_apps --test release_security_gate` passed after adding transition-doc assertions.
- AR-004 — DONE — `RunGoal`, `RunBudget`, `RunApprovalMode`, `RunPhase`, `RunOutcome`, `RunStatus`, and `RunEvent` were added to `axonrunner_core`, and `cargo test -p axonrunner_core` passed.
- AR-005 — DONE — separate run lifecycle reducer/projection paths were added so planning, execution, verification, repair, approval wait, and terminal outcomes can be projected without disturbing fact-state reduction.
- AR-006 — DONE — legacy intent execution now flows through an explicit `RunTemplate::LegacyIntent(...)` compatibility shim, and `cargo test -p axonrunner_apps --test e2e_cli` passed.
- AR-007 — DONE — run lifecycle regression tests now cover approval wait and budget exhaustion terminal behavior, and `cargo test -p axonrunner_core` passed.
- AR-008 — BLOCKED — the current runtime does not yet implement planner/executor/verifier/terminal-outcome semantics strongly enough to make `run <goal>`, `resume`, and `abort` honest public truth.
- AR-011 — DONE — bounded planner artifacts now exist via `RuntimeRunPlan`, with unit tests for write/control plans.
- AR-012 — DONE — executor flow now carries explicit provider/memory/tool step state, provider output, and per-run execution records, verified by `cargo test -p axonrunner_apps --test e2e_cli`.
- AR-013 — DONE — verifier logic now evaluates execution separately from application, writes richer verify artifacts, and emits trace verification summaries.
- AR-014 — DONE — a bounded repair loop retries failed tool steps, with a fault-injection unit test proving repair can recover a failed tool stage.
- AR-015 — DONE — status/replay now surface terminal outcomes and run phases, with success, blocked, approval-required, and aborted representation covered by tests.
- AR-016 — DONE — persisted state now tracks `next_run_seq`, traces carry stable `run_id` plus deterministic `step_ids`, and replay/status output exposes them.
- AR-017 — DONE — runtime execution now records the canonical provider workspace binding in apply/report artifacts and trace run summaries, with tests locking canonical workspace binding.
- AR-018 — DONE — Codex probe tests now cover blocked old versions and degraded unknown versions, and doctor e2e asserts binary/version/compatibility visibility.
- AR-019 — DONE — the session reuse contract is now locked by a dedicated test proving reuse only happens when both `cwd` and `model` match.
- AR-020 — BLOCKED — current runtime still uses one workspace root for execution, tool writes, trace, and artifacts, so safe per-run git worktree isolation needs an execution-workspace vs artifact-workspace split first.
- AR-021 — DONE — the tool request/output/evidence schema is now explicit in `docs/CAPABILITY_MATRIX.md` and locked by dedicated adapter contract tests.
- AR-022 — DONE — the essential tool surface remains fully green end-to-end under `cargo test -p axonrunner_adapters --test tool`.
- AR-023 — DONE — `run_command` now emits standard build/test/lint/generic profiles, and apps/adapters tests lock the shared profile naming contract.
- AR-024 — DONE — replay tests now require digest and excerpt-level patch evidence so operators can read change meaning directly from replay output.
- AR-025 — DONE — high-risk tool tiers are now defined in code and `docs/RUNBOOK.md`, with tests proving remove and dangerous commands classify as high risk.
- AR-026 — DONE — runtime now writes run summaries into a separate `recall:` memory namespace while existing direct fact values stay in working memory, and adapter/apps tests lock the split helpers plus persisted recall entries.
- AR-027 — DONE — trace now builds an explicit artifact index, and replay exposes that index alongside the latest artifact paths.
- AR-028 — DONE — hot context summaries are now compacted to a bounded size before being stored in recall memory, with long-summary tests locking the cap.
- AR-029 — DONE — runtime now searches parent workspaces for `AGENTS.md`, surfaces that path in tool health detail, and carries the guidance location into stored run summaries.
