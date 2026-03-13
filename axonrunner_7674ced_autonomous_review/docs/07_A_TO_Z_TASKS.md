# 07. A-to-Z Tasks

Tasks are ordered by product dependency, not by implementation convenience.

## P0

### AZ-001 — Lock current truth surface across README/charter/capability matrix/runbook/help
- Done when: done when all public docs and CLI outputs agree on current contract
- Proof: docs + golden text tests

### AZ-002 — Decide async runtime fallback policy
- Done when: done when fallback is either removed or explicitly surfaced as blocked/degraded contract
- Proof: doctor + init tests

### AZ-003 — Create product-owned goal schema spec
- Done when: done when objective/workspace/constraints/done_conditions/verification/budget/approval are versioned
- Proof: schema tests

### AZ-004 — Implement goal file parser
- Done when: done when goal yaml/json/md loads into validated RunGoal input
- Proof: parser tests

### AZ-005 — Add canonical CLI commands run/resume/abort around goal files
- Done when: done when help/status/replay support run ids
- Proof: e2e CLI tests

### AZ-006 — Create run-id based persistence path
- Done when: done when runs survive process restarts and are addressable by id
- Proof: state/trace tests

### AZ-007 — Promote RunGoal/RunEvent/RunStatus path into apps runtime
- Done when: done when autonomous run path uses core run domain directly
- Proof: core/apps integration tests

## P1

### AZ-008 — Define explicit step model
- Done when: done when each run compiles to inspectable steps with ids/types/status
- Proof: planner tests

### AZ-009 — Persist step journal into trace
- Done when: done when replay can render per-step timeline
- Proof: trace tests

### AZ-010 — Replace shallow tool phase with step executor
- Done when: done when runtime uses tool adapter operations beyond log/report append
- Proof: e2e step tests

### AZ-011 — Implement inspect/read/search step handlers
- Done when: done when planner can gather workspace evidence before mutation
- Proof: tool integration tests

### AZ-012 — Implement mutate/write/replace/remove step handlers with path expectations
- Done when: done when mutation steps emit bounded evidence and changed-path summaries
- Proof: mutation tests

### AZ-013 — Implement verification-command steps
- Done when: done when cargo/npm/python build/test/lint/typecheck can be attached to milestones
- Proof: command profile tests

### AZ-014 — Upgrade report artifacts to step-aware plan/apply/verify/report documents
- Done when: done when reports show milestone/step/evidence/failure boundary
- Proof: artifact golden tests

### AZ-015 — Implement terminal done-condition evaluation
- Done when: done when success requires all declared checks to pass
- Proof: goal-run e2e tests

### AZ-016 — Implement per-milestone verification
- Done when: done when milestone advancement is impossible without passing checks
- Proof: milestone tests

### AZ-017 — Implement repair classification
- Done when: done when verification failures map to retryable/non-retryable classes
- Proof: repair tests

### AZ-018 — Implement bounded repair loop
- Done when: done when repair attempts consume explicit budget and stop deterministically
- Proof: repair budget tests

### AZ-019 — Persist repair attempts and reasons
- Done when: done when replay/report show repair history
- Proof: replay/report tests

### AZ-020 — Support operator-readable changed-path and patch preview summaries
- Done when: done when replay/report show what changed without raw diff overload
- Proof: artifact tests

### AZ-021 — Wire tool risk tiers into approval gate
- Done when: done when risky steps pause before execution under on-risk/always
- Proof: approval e2e tests

### AZ-022 — Implement persisted pending-approval state
- Done when: done when status/replay show waiting approval with step context
- Proof: state/replay tests

### AZ-023 — Implement resume after approval
- Done when: done when approved runs continue from paused step, not restart whole run
- Proof: resume tests

### AZ-024 — Implement abort semantics
- Done when: done when abort leaves coherent terminal outcome and report
- Proof: abort tests

### AZ-025 — Define budget exhaustion semantics
- Done when: done when step/time/token limits yield budget_exhausted terminal outcome
- Proof: budget tests

### AZ-026 — Expose budget and approval state in doctor/status/replay
- Done when: done when operator can inspect run control state easily
- Proof: golden output tests

## P2

### AZ-027 — Define execution isolation strategy (single-writer lockfile or worktree-per-run)
- Done when: done when concurrency contract is explicit and enforced
- Proof: design doc + tests

### AZ-028 — Protect state snapshot against concurrent writers
- Done when: done when multi-process writes are prevented or serialized
- Proof: race tests

### AZ-029 — Protect trace log append path
- Done when: done when events.jsonl cannot be corrupted by multi-process append
- Proof: trace concurrency tests

### AZ-030 — Protect memory backends or document single-writer mode
- Done when: done when markdown/sqlite behavior is deterministic under chosen contract
- Proof: memory concurrency tests

### AZ-031 — Serialize or isolate provider session usage
- Done when: done when active session cannot be asked concurrently unsafely
- Proof: provider concurrency tests

### AZ-032 — Remove or surface hidden async-host fallback
- Done when: done when configured runtime init failure is operator-visible and policy-consistent
- Proof: doctor/init tests

### AZ-033 — Tighten replace_in_file semantics with expected_count/max_count
- Done when: done when risky broad replacements are impossible by accident
- Proof: tool tests

### AZ-034 — Surface unreadable/skipped files in search/list operations
- Done when: done when search results report skips/errors deterministically
- Proof: tool tests

### AZ-035 — Add worktree adapter or scm adapter for isolated coding runs
- Done when: done when runs can operate in ephemeral worktrees and leave reviewable diffs
- Proof: integration tests

### AZ-036 — Create verifier profiles for Rust/Node/Python projects
- Done when: done when common dev stacks can be verified with minimal configuration
- Proof: profile tests

### AZ-037 — Define workflow-pack contract
- Done when: done when user-provided workflow packs have stable schema and allowed-tool/verifier metadata
- Proof: schema tests

### AZ-038 — Implement adapter SDK for workflow packs
- Done when: done when external workflow adapters plug into one orchestrator contract
- Proof: SDK samples

### AZ-039 — Support spec-stack inputs (SPEC/PLAN/IMPLEMENT/STATUS/AGENTS)
- Done when: done when runs can ingest durable project memory files as source of truth
- Proof: e2e goal-package tests

### AZ-040 — Create app workflow pack
- Done when: done when an app scaffold/change goal can run end-to-end
- Proof: golden repo eval

### AZ-041 — Create server/API workflow pack
- Done when: done when a server feature goal can run end-to-end
- Proof: golden repo eval

## P3

### AZ-042 — Build autonomous eval corpus
- Done when: done when representative repos/goals exist for repeated scoring
- Proof: eval dataset

### AZ-043 — Add trace/artifact scoring checks
- Done when: done when runs are scored on outcome plus evidence quality
- Proof: eval harness

### AZ-044 — Add release gate for autonomous runs
- Done when: done when releases are blocked by failing representative runs
- Proof: CI gate

### AZ-045 — Dogfood maintenance workflow on AxonRunner itself
- Done when: done when a recurring internal engineering workflow completes through the product
- Proof: dogfood report

### AZ-046 — Dogfood spec-to-feature workflow
- Done when: done when a feature derived from a planning doc is executed end-to-end
- Proof: dogfood report

### AZ-047 — Trim legacy intent path to debug/migration mode
- Done when: done when goal-run is primary and legacy path no longer drives product decisions
- Proof: CLI/docs update

### AZ-048 — Write product release checklist for autonomous mode
- Done when: done when operator can deploy/use/recover confidently
- Proof: runbook
