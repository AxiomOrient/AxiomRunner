# AxonRunner Autonomous Transition Implementation Plan

## Scope Contract

- Request: turn the `docs/roadmap/` bundle into a build-ready implementation plan for migrating AxonRunner from an `intent-spec` minimal runtime to a goal/run autonomous workspace agent.
- Target scope: repo-wide changes across `README.md`, `docs/`, `crates/core`, `crates/apps`, `crates/adapters`, regression tests, release gates, and later-stage examples.
- Repository baseline:
  - Current truth surface is still `run <intent-spec>`, `batch`, `doctor`, `replay`, `status`, `health`, `help`, plus legacy aliases.
  - `docs/roadmap/` and `docs/AUTONOMOUS_AGENT_TARGET.md` describe the future transition target, not the current product contract.
  - The current worktree does not yet contain `plans/IMPLEMENTATION-PLAN.md` or `plans/TASKS.md`, so this plan becomes the first canonical execution ledger for the transition.
- Out of scope:
  - multi-agent orchestration
  - browser or channel integrations
  - daemon, gateway, or cron-first surfaces
  - skills marketplace or broad MCP sprawl
- Constraints:
  - preserve event-sourced truth, workspace boundary, and explicit failure semantics throughout the transition
  - do not claim the autonomous product contract before planner, verifier, repair, approval, budget, and replay evidence actually exist
  - prefer additive migration steps and compatibility shims over a big-bang rewrite
  - keep docs, help text, release gate tests, and runtime behavior aligned at every phase boundary
- Done conditions:
  - docs, help, and release gates converge on the same goal/run public contract
  - `core` models run lifecycle and terminal outcomes, not only fact mutation
  - `apps` executes a bounded `plan -> act -> verify -> repair -> report` loop with explicit stop reasons
  - `adapters` provide bounded workspace execution, deterministic artifacts, verifier evidence, and safety policies
  - release decisions use autonomy-specific evidence instead of intent-runtime-only confidence

## Design Summary

AxonRunner should migrate in three coupled layers. `core` changes first to introduce the run lifecycle, invariants, and event vocabulary that the rest of the system can trust. `apps` then moves the public CLI and runtime harness from one-shot intent execution to a bounded autonomous loop. `adapters` harden provider, tool, and memory behavior so the loop can operate on a real workspace without hidden fallback or unverifiable success.

The migration should stay dual-track until the new truth is real. Current docs and tests still lock the minimal runtime surface, while `docs/roadmap/` and the bridge docs describe the target state. That means P0 is not editorial cleanup; it is the contract lock that prevents false claims while the code remains intent-centric. The plan therefore treats doc alignment, release gates, and CLI/help outputs as core product work.

Implementation order follows dependency rather than ownership. Contract lock must land before public-surface changes. Domain remodel must land before the autonomous loop can be honest. Loop harness and substrate hardening must exist before approval, budget, resume, and eval signals are meaningful. Context engineering and dogfooding remain later phases because they amplify a stable loop instead of substituting for one.

Temporary bridge docs already exist in `docs/AUTONOMOUS_AGENT_TARGET.md` and `docs/DOCS_ALIGNMENT.md`. They should be used to reduce reader confusion while P0 is in flight, but the final product truth must move back into the canonical root docs and release checks once the autonomous surface is implemented.

## Expanded Atomic Path

1. `scout-boundaries` — reconcile roadmap intent with the current repo truth surface and bridge docs.
2. `plan-what-it-does` — define the bounded contract, constraints, non-goals, and observable done conditions for the transition.
3. `plan-how-to-build` — order the migration around domain-first data changes, loop scaffolding, and safety/eval gates.
4. `plan-task-breakdown` — convert the roadmap backlog into dependency-ordered execution rows under `plans/TASKS.md`.

## Phase Plan

| Phase | Goal | Primary Files | Entry Criteria | Exit Evidence |
|---|---|---|---|---|
| P0 | Lock the autonomous transition contract without pretending the product is already there. | `README.md`, `docs/project-charter.md`, `docs/CAPABILITY_MATRIX.md`, `docs/RUNBOOK.md`, `docs/AUTONOMOUS_AGENT_SPEC.md`, `crates/apps/tests/release_security_gate.rs` | current truth and target truth are both visible and can be separated cleanly | root docs, help, and release gate all describe the same transition-aware public surface |
| P1 | Introduce a run-oriented domain model under the current intent runtime. | `crates/core/src/{intent,event,effect,state,reducer,projection}.rs`, `crates/core/tests/*` | P0 contract exists and the target lifecycle is named | `core` expresses `RunGoal`, `RunPhase`, `RunOutcome`, budget/approval semantics, and lifecycle invariants |
| P2 | Move the CLI surface from intent-first to goal/run-first without losing migration safety. | `crates/apps/src/{cli_args,cli_command,display}.rs`, `crates/apps/tests/e2e_cli.rs`, `README.md` | P1 types and compatibility story exist | help text and golden CLI outputs lock `run`, `resume`, `abort`, `status`, `replay`, `doctor` as the public truth |
| P3 | Build the single-agent autonomous loop harness. | `crates/apps/src/{cli_runtime,runtime_compose,replay,status,trace_store,state_store}.rs` | P2 gives the loop a truthful public entrypoint | every run records planning, execution, verification, repair, and terminal outcome evidence |
| P4 | Harden `codek` / `codex-runtime` into a run-scoped execution substrate. | `crates/adapters/src/provider_codex_runtime.rs`, `crates/apps/src/{runtime_compose,doctor}.rs`, `docs/CODEK_RUNTIME_CONTRACT.md` | P3 loop can execute steps and report failure boundaries | provider workspace binding, compatibility probe, and optional isolation are explicit and testable |
| P5 | Finish the tool and verifier contract so "done" means verified, not merely changed. | `crates/adapters/src/{contracts,tool,tool_workspace}.rs`, `crates/apps/src/{runtime_compose,replay}.rs`, `docs/CAPABILITY_MATRIX.md`, `docs/RUNBOOK.md` | P3/P4 can execute against a workspace | tool input/output schema, verifier profiles, patch evidence, and risk tiers are stable and operator-readable |
| P6 | Add context discipline and durable memory boundaries for long-horizon runs. | `crates/apps/src/{cli_runtime,runtime_compose,trace_store,state_store}.rs`, `crates/adapters/src/{memory,memory_sqlite,memory_markdown}.rs` | loop, trace, and verifier artifacts already exist | hot context, recall memory, artifact index, compaction, and `AGENTS.md` ingestion are explicit and replayable |
| P7 | Add operator control for approval, budget, resume, abort, and rollback. | `crates/apps/src/{cli_command,cli_runtime,state_store,status,replay}.rs`, `docs/RUNBOOK.md`, `crates/core/src/state.rs` | loop and substrate already expose stable ids and failure modes | risky work can be paused, resumed, aborted, or rolled back with explicit evidence |
| P8 | Gate releases on autonomy evidence and real usage, not aspiration. | `crates/apps/tests/e2e_cli.rs`, `crates/apps/tests/release_security_gate.rs`, `crates/adapters/tests/*`, `crates/core/tests/*`, `docs/CAPABILITY_MATRIX.md`, `docs/RUNBOOK.md`, `examples/*` | P0-P7 are materially implemented | autonomy corpus, false-success metrics, provider-failure evals, nightly dogfood, and release gate criteria are in place |

## Execution Strategy

Build in four waves so each slice closes with usable evidence instead of broad partial work.

1. Wave A: P0 + P1
   - lock the contract and land the run domain model
   - do not expose a new public CLI until the underlying state machine exists
2. Wave B: P2 + P3 + P4
   - switch the public entry surface, then immediately back it with a traceable loop and hardened provider/workspace substrate
3. Wave C: P5 + P7
   - finish verifier quality and operator-control semantics before claiming safe autonomous execution
4. Wave D: P6 + P8
   - optimize long-horizon behavior and release confidence only after the core loop is stable

## Open Decisions

- Legacy public surface demotion
  - Option A: keep `batch` plus legacy aliases visible until `run <goal>` is production-ready
  - Option B: demote them as soon as P2 lands
  - Recommended: Option A, because current docs/tests still lock those paths and early demotion would create contract churn without autonomy evidence.
- Event schema migration shape
  - Option A: add `RunEvent` and run lifecycle projections alongside the current intent event path, then phase out legacy projections later
  - Option B: replace the current event schema in place
  - Recommended: Option A, because replay compatibility and transition safety matter more than immediate schema purity.
- Workspace isolation default
  - Option A: ship in-place workspace execution first and add optional worktree isolation in P4/P7
  - Option B: require worktree isolation before any write-capable autonomous loop ships
  - Recommended: Option A first, but only if approval, checkpoint, and failure reporting are present before high-risk write operations leave experimental scope.
