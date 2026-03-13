# AxonRunner Documentation-Driven Improvement Plan

## Scope Contract

- Request: review the current root docs and the autonomous review bundle, then turn that evidence into a repo-improvement plan.
- Target scope: `README.md`, `docs/*.md`, `plans/*.md`, `axonrunner_7674ced_autonomous_review/docs/*.md`, and the follow-on code areas named by those docs in `crates/core`, `crates/apps`, and `crates/adapters`.
- Repository baseline:
  - Current shipped truth is still the minimal `intent-spec` CLI surface, confirmed by `README.md`, `docs/project-charter.md`, `docs/CAPABILITY_MATRIX.md`, `docs/RUNBOOK.md`, CLI help output, and `cargo test -p axonrunner_apps --test release_security_gate`.
  - Autonomous transition analysis now lives in `axonrunner_7674ced_autonomous_review/docs/`, not in `docs/roadmap/`.
  - `docs/AUTONOMOUS_AGENT_TARGET.md`, `docs/DOCS_ALIGNMENT.md`, and the old planning ledger still point at deleted `docs/roadmap/*`, so the reading path and plan baseline are stale.
  - The review bundle converges on the same remaining product gaps: truthful `goal/run` contract, real step engine, stronger verification, approval enforcement, and concurrency/isolation hardening.
- Out of scope:
  - implementing the runtime changes in this planning pass
  - widening the product into channels, gateways, multi-agent orchestration, or marketplace features
  - rewriting root docs to claim `goal/run` behavior before the CLI actually ships it
- Constraints:
  - keep current-truth docs honest until public behavior changes
  - declare one canonical home for transition analysis and one canonical task ledger
  - reuse the existing run-domain scaffolding instead of inventing a second goal schema
  - end every phase with operator-visible evidence such as docs, tests, or replayable artifacts
- Done conditions:
  - root docs and planning docs point to living transition references
  - the repo has one explicit reading path for current truth versus next-product transition material
  - remaining project work is ordered from doc truth repair to runtime hardening
  - `plans/TASKS.md` matches the current repo topology and the real dependency path

## Document Review Summary

### Healthy parts

- Root truth docs are mostly aligned on the retained CLI surface.
- The autonomous review bundle is coherent and already narrows the next product around one agent, one workspace, one evidence trail.
- The release gate test already protects the current truth surface from drifting into false marketing.

### Gaps that matter now

- Dead `docs/roadmap/*` references remain in root transition docs and in the planning ledger.
- Transition analysis is split between root bridge docs and the external review bundle without one declared canonical owner.
- The current plan/task ledger still assumes the deleted roadmap tree is the active source set.
- The release gate protects truth tokens, but the document topology itself is not yet guarded.

## Complexity Map

### Essential complexity

- The repo must keep two truths separate until the product actually changes: current `intent-spec` reality and next `goal/run` target.
- The transition crosses `core`, `apps`, and `adapters`, so the plan has to respect domain, CLI, and substrate dependencies.
- Success must stay evidence-driven; docs, tests, status, replay, and risk policy all have to move together.

### Accidental complexity

- Deleted `docs/roadmap/*` paths are still treated as live sources.
- Transition material has two homes without a declared primary index.
- Planning artifacts drifted away from the current document tree.
- Topology drift can recur because the current checks do not validate transition-reference integrity.

### Simplification candidates

1. Declare one canonical transition-doc home and update every reference to that location.
2. Add a lightweight docs-topology check so dead roadmap references fail quickly.
3. Reset the plan/task ledger around the current baseline instead of keeping stale source paths alive.

## Design Summary

The improvement path should start with documentation truth repair, not more runtime surface. Right now the repo already has a usable minimal runtime, a clear bridge into the next product, and a substantial review bundle. The immediate problem is that those assets are arranged in a way that still makes readers and future execution plans look at deleted paths.

After the doc topology is repaired, the project should promote the autonomous contract in dependency order. First freeze the goal package and run identity contract using the run-domain scaffolding that already exists in `core`. Then move the public CLI only when `apps` can ingest a goal file, persist run ids, and report status/replay honestly. Only after that should the shallow tool phase be replaced by a real step engine, followed by stronger verification, approval control, concurrency hardening, and release evidence.

This keeps the plan aligned with the review bundle while avoiding two mistakes: pretending the autonomous product already exists, or restarting work that the current baseline already solved.

## Expanded Atomic Path

1. `scout-boundaries` — reconcile current truth docs, review-bundle docs, and the real repo tree.
2. `plan-what-it-does` — lock the bounded improvement goal, constraints, and done conditions.
3. `plan-how-to-build` — order the work from doc truth repair to runtime/product hardening.
4. `plan-task-breakdown` — convert the path into execution-ready task rows under `plans/TASKS.md`.

## Phase Plan

| Phase | Goal | Primary Files | Entry Criteria | Exit Evidence |
|---|---|---|---|---|
| P0 | Repair the document topology so every root reference points at living transition material. | `README.md`, `docs/AUTONOMOUS_AGENT_TARGET.md`, `docs/DOCS_ALIGNMENT.md`, `plans/IMPLEMENTATION-PLAN.md`, `plans/TASKS.md` | current truth docs remain valid and the review bundle exists | dead roadmap references are removed or redirected, and one reading path is documented |
| P1 | Freeze the canonical goal package contract by reusing existing run-domain types instead of inventing new ones. | `docs/AUTONOMOUS_AGENT_SPEC.md`, `crates/core/src/{intent,validation,state}.rs`, parser fixtures | P0 gives the repo one honest transition-doc baseline | goal input shape, run ids, done conditions, budget, and approval rules are versioned and testable |
| P2 | Promote a truthful `goal/run` public surface only after ingestion and persistence exist. | `crates/apps/src/{cli_command,cli_args,display,status}.rs`, `README.md`, `docs/CAPABILITY_MATRIX.md`, `docs/RUNBOOK.md` | P1 schema and persistence boundary exist | `run <goal-file>`, `status`, `replay`, `resume`, and `abort` agree across help, docs, and e2e tests |
| P3 | Replace the shallow compose path with a real step engine plus completion-grade verification. | `crates/apps/src/{cli_runtime,runtime_compose,trace_store,replay}.rs`, `crates/adapters/src/{tool,contracts}.rs` | P2 makes the public entrypoint honest | runs produce explicit step journals, verification evidence, bounded repairs, and terminal outcomes tied to declared checks |
| P4 | Add operator control and safety hardening around risky work and multi-run interference. | `crates/apps/src/{cli_runtime,status,replay,state_store}.rs`, `crates/adapters/src/{provider_codex_runtime,memory,tool}.rs`, `docs/RUNBOOK.md` | P3 can execute and verify real work | approval, resume, abort, budget exhaustion, and concurrency/isolation rules are operator-visible and tested |
| P5 | Move release confidence from aspiration to evidence. | `crates/apps/tests/*`, `crates/adapters/tests/*`, `docs/CAPABILITY_MATRIX.md`, `docs/RUNBOOK.md`, example or fixture repos | P4 safety and control semantics are stable | representative autonomous runs, eval suites, and release gates prove the shipped product behavior |

## Critical Path

1. P0 document topology repair
2. P1 goal package contract freeze
3. P2 truthful `goal/run` CLI promotion
4. P3 step engine plus verification
5. P4 approval and isolation hardening
6. P5 eval-backed release gates

## Decision Gates

| Gate | Check | Pass Condition | On Fail |
|---|---|---|---|
| DG-1 | Transition-doc home | one canonical transition-doc location and reading path are declared | stop surface edits and resolve doc ownership first |
| DG-2 | Goal package contract | one goal input schema maps cleanly onto current run-domain types | keep CLI truth on `intent-spec` until schema is fixed |
| DG-3 | Public CLI truth flip | goal-file ingestion, run-id persistence, status, and replay all work together | keep legacy surface canonical and continue implementation behind the bridge docs |
| DG-4 | Safe autonomy gate | verification, approval, budget, and isolation semantics all have passing evidence | do not call the product autonomous in release docs |
| DG-5 | Release evidence gate | eval corpus and release checks prove repeated autonomous success | keep autonomous work experimental |

## Open Decisions

- Canonical transition-doc home
  - Option A: keep `axonrunner_7674ced_autonomous_review/docs/` as the source of transition analysis and update root docs to point there.
  - Option B: move that bundle under a canonical path such as `docs/transition/` and treat the review folder as archived input.
  - Recommended: Option B, because it restores one obvious docs tree and removes the need for root docs to point outside `docs/`.
- Concurrency model
  - Option A: declare single-writer mode first and enforce it with lockfiles.
  - Option B: move directly to worktree-per-run isolation.
  - Recommended: Option A first, then add worktree isolation when the autonomous loop is already stable.
