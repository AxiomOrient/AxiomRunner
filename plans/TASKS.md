# AxonRunner Project Improvement Tasks

Source inputs:
- `README.md`
- `docs/project-charter.md`
- `docs/AUTONOMOUS_AGENT_TARGET.md`
- `docs/AUTONOMOUS_AGENT_SPEC.md`
- `docs/CAPABILITY_MATRIX.md`
- `docs/DOCS_ALIGNMENT.md`
- `docs/RUNBOOK.md`
- `axonrunner_7674ced_autonomous_review/docs/01_REVIEW_REPORT.md`
- `axonrunner_7674ced_autonomous_review/docs/03_PRODUCT_QUALITY_FINDINGS.md`
- `axonrunner_7674ced_autonomous_review/docs/04_NEXT_PRODUCT_DEFINITION.md`
- `axonrunner_7674ced_autonomous_review/docs/05_ROADMAP.md`
- `axonrunner_7674ced_autonomous_review/docs/06_PHASED_IMPLEMENTATION_PLAN.md`
- `axonrunner_7674ced_autonomous_review/docs/07_A_TO_Z_TASKS.md`
- CLI help output from `cargo run -p axonrunner_apps -- --help`
- release evidence from `cargo test -p axonrunner_apps --test release_security_gate`

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
| IMP-001 | DONE | Refresh the implementation plan and task ledger against the current document tree. | `plans/IMPLEMENTATION-PLAN.md`; `plans/TASKS.md` | plan docs reference live source paths and the current dependency story | document review of root docs plus review bundle | - |
| IMP-002 | DONE | Choose one canonical home for transition analysis and reading order. | `README.md`; `docs/AUTONOMOUS_AGENT_TARGET.md`; `docs/DOCS_ALIGNMENT.md`; transition-doc folder choice | root docs tell readers where current truth ends and where next-product analysis lives | doc diff plus manual read-through | IMP-001 |
| IMP-003 | DONE | Remove or redirect dead `docs/roadmap/*` references and add a topology guard. | root docs; planning docs; release gate or docs-check script/test | stale roadmap paths no longer appear as live references, and drift is checked automatically | `rg -n "docs/roadmap"` returns only intentional archival mentions; docs guard passes | IMP-002 |

## Phase P1

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| IMP-004 | DONE | Freeze the canonical goal package contract using existing run-domain scaffolding. | `docs/AUTONOMOUS_AGENT_SPEC.md`; `crates/core/src/{intent,validation,state}.rs` | one goal input shape defines objective, workspace, constraints, done conditions, verification, budget, and approval | schema/validation tests plus spec review | IMP-003 |
| IMP-005 | DONE | Add goal-file ingestion and run-id persistence. | `crates/apps/src/{cli_command,cli_args}.rs`; `crates/core`; parser fixtures | a goal file loads into a persisted run record that `status` and `replay` can address by run id | e2e CLI tests for parse, create, status, replay | IMP-004 |

## Phase P2

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| IMP-006 | DONE | Promote a truthful `goal/run` CLI surface and demote legacy intent commands. | `crates/apps/src/{cli_command,display,status}.rs`; `README.md`; `docs/CAPABILITY_MATRIX.md`; `docs/RUNBOOK.md` | help, docs, and runtime all agree on `run <goal-file>`, `status`, `replay`, `resume`, and `abort` | e2e CLI tests plus release-gate doc checks | IMP-005 |

## Phase P3

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| IMP-007 | DONE | Replace the shallow runtime path with an explicit step journal and executor. | `crates/apps/src/{cli_runtime,runtime_compose,trace_store,replay}.rs`; `crates/adapters/src/tool.rs` | each run records inspect, mutate, verify, and repair steps with ids, evidence, and failure boundaries | planner and step-execution tests | IMP-005 |
| IMP-008 | DONE | Make verification completion-driven instead of artifact-presence-driven. | `crates/apps/src/{cli_runtime,runtime_compose}.rs`; `crates/adapters/src/{tool,contracts}.rs` | success requires declared file, command, and changed-path checks, with bounded repair attempts on failure | verification and repair regression tests | IMP-007 |

## Phase P4

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| IMP-009 | DONE | Enforce approval, resume, abort, and budget controls end to end. | `crates/apps/src/{cli_runtime,status,replay,state_store}.rs`; `crates/core/src/state.rs` | risky steps can pause before execution, resume from pending approval, abort cleanly, and stop on budget exhaustion | approval, resume, abort, and budget e2e tests | IMP-008 |
| IMP-010 | DONE | Define and implement the concurrency or isolation contract. | `crates/apps/src/{state_store,trace_store}.rs`; `crates/adapters/src/{provider_codex_runtime,memory,tool}.rs`; `docs/RUNBOOK.md` | multi-run interference policy is explicit and enforced by locks or isolation | race or isolation tests plus runbook review | IMP-009 |

## Phase P5

| ID | Status | Goal | Scope | Done When | Evidence Required | Depends On |
|---|---|---|---|---|---|---|
| IMP-011 | DONE | Define the workflow-pack and adapter contract without splitting product truth. | `docs/`; `crates/adapters`; SDK or contract docs | user-provided workflow packs plug into one orchestration contract with allowed tools and verifier rules | contract docs plus sample adapter tests | IMP-010 |
| IMP-012 | DONE | Add autonomous evals and release gates based on real runs. | `crates/apps/tests/*`; `crates/adapters/tests/*`; `docs/CAPABILITY_MATRIX.md`; `docs/RUNBOOK.md`; fixture repos | release criteria depend on representative autonomous success, failure visibility, and replay quality | eval suite plus updated release gate | IMP-008, IMP-009, IMP-010 |
| IMP-013 | DONE | Remove panic-prone legacy helpers and leftover dead-code warnings from the apps runtime. | `crates/apps/src/{cli_command,runtime_compose}.rs`; `plans/TASKS.md` | no panic-only legacy accessor remains and the runtime no longer warns about unused legacy phase or verifier helpers | apps test suite plus eval corpus | IMP-012 |
| IMP-014 | DONE | Remove the synthetic goal-file intent shim from the apps runtime. | `crates/apps/src/{cli_command,cli_runtime}.rs`; `plans/TASKS.md` | goal-file runs no longer flow through a fake legacy intent conversion path | apps test suite plus eval corpus | IMP-013 |

## Immediate Build Slice

Start with the shortest path that removes current planning drift and unlocks truthful product work:

1. `IMP-002`
2. `IMP-003`
3. `IMP-004`
4. `IMP-005`
5. `IMP-006`

## Decision Gates

| Gate | Check | Passes When | On Fail |
|---|---|---|---|
| DG-1 | Canonical transition-doc home | one live location is declared and root docs point to it | stop and resolve doc ownership first |
| DG-2 | Goal package contract | goal-file schema maps cleanly onto existing run-domain types | keep current CLI truth unchanged |
| DG-3 | Truthful public CLI | help, docs, parse path, status, and replay all agree | keep `goal/run` behind bridge docs only |
| DG-4 | Safe autonomy | verification, approval, budget, and isolation semantics all have passing evidence | do not market or release autonomous mode |
| DG-5 | Eval-backed release | repeated autonomous runs pass representative scenarios | keep autonomy experimental |

## Progress Log

- IMP-002 — DONE — added `docs/transition/README.md` as the canonical transition-doc entrypoint and updated `README.md`, `docs/AUTONOMOUS_AGENT_TARGET.md`, and `docs/DOCS_ALIGNMENT.md` to point at it.
- IMP-003 — DONE — removed live `docs/roadmap/*` references from root docs and extended `crates/apps/tests/release_security_gate.rs` so root transition references must point at `docs/transition/README.md`; `cargo test -p axonrunner_apps --test release_security_gate` passed.
- IMP-004 — DONE — documented the canonical `RunGoal` field mapping in `docs/AUTONOMOUS_AGENT_SPEC.md`, added `RunGoal::validate()` plus budget validation in `crates/core/src/intent.rs`, and verified the contract with `cargo test -p axonrunner_core`.
- IMP-005 — DONE — added JSON goal-file ingestion, `RunGoal` validation at intake, blocked-but-persisted goal runs, and `run_id` lookup support for `status`/`replay`; `cargo test -p axonrunner_apps --test e2e_cli` and `cargo test -p axonrunner_apps --test release_security_gate` passed.
- IMP-006 — DONE — promoted the public help and root docs to the `goal-file` surface, added visible `resume`/`abort` commands, and demoted legacy intent paths into compatibility sections; `cargo test -p axonrunner_apps --test e2e_cli`, `cargo test -p axonrunner_apps --test release_security_gate`, and `cargo test -p axonrunner_core` passed.
- IMP-007 — DONE — added explicit step journals to runtime records and trace replay, including per-step status, evidence, and failure fields; `cargo test -p axonrunner_apps --test e2e_cli`, `cargo test -p axonrunner_apps --test release_security_gate`, and `cargo test -p axonrunner_core` passed.
- IMP-008 — DONE — changed runtime verification from artifact-only checks to completion checks against actual state, mode, and changed paths for current executable runs; `cargo test -p axonrunner_apps --test e2e_cli`, `cargo test -p axonrunner_apps --test release_security_gate`, and `cargo test -p axonrunner_core` passed.
- IMP-009 — DONE — added hidden `resume`/`abort` command routing, pending goal-run persistence in `state_store`, approval-required resume, clean abort, and step-budget exhaustion handling; `cargo test -p axonrunner_apps --test e2e_cli`, `cargo test -p axonrunner_apps --test release_security_gate`, and `cargo test -p axonrunner_core` passed.
- IMP-010 — DONE — enforced a single-writer workspace lock for mutating commands with `.axonrunner/runtime.lock`, documented the policy in `docs/RUNBOOK.md`, and verified that mutating commands block while `status` still reads; `cargo test -p axonrunner_apps --test e2e_cli`, `cargo test -p axonrunner_apps --test release_security_gate`, and `cargo test -p axonrunner_core` passed.
- IMP-011 — DONE — added a workflow-pack contract document plus explicit adapter contract types and tests so packs can narrow tools and verifier rules without redefining run lifecycle semantics; `cargo test -p axonrunner_adapters`, `cargo test -p axonrunner_apps --test release_security_gate`, and `cargo test -p axonrunner_apps --test e2e_cli` passed.
- IMP-012 — DONE — added a representative autonomous eval corpus with fixture goal files and updated release docs so release evidence now depends on real runs, replay quality, and adapter coverage; `cargo test -p axonrunner_apps --test autonomous_eval_corpus`, `cargo test -p axonrunner_apps --test release_security_gate`, `cargo test -p axonrunner_adapters`, and `cargo test -p axonrunner_apps --test e2e_cli` passed.
- IMP-013 — DONE — removed the panic-only `legacy_intent()` accessor, deleted the unused verifier helper, and made planning/repairing phase strings part of real report artifacts so the apps runtime no longer carries those dead-code warnings as leftover legacy noise.
- IMP-014 — DONE — removed the fake `Intent::read(\"__goal_file__\")` shim so goal-file runs no longer pretend to be legacy intent conversions before execution.
