# 06. Phased Implementation Plan

## Phase 0 — Truth Lock

### Objective
Stabilize what exists and make the next transition safe.

### Implement
- add explicit documentation that current truth is `intent-spec`, target truth is `goal/run`
- expose async-host init mode clearly in `doctor`
- define a single policy on fallback behavior
- confirm all current commands and outputs are intentionally supported

### Deliverables
- synchronized README / charter / capability matrix / runbook
- release gate checks for doc drift
- issue list for known non-goal/autonomous gaps

---

## Phase 1 — Canonical Goal Schema

### Objective
Replace legacy intent as the public mental model.

### Implement
- design `goal.yaml` / `goal.json` / `GOAL.md` ingestion layer
- validate objective / workspace / constraints / done conditions / verification checks / budgets / approval mode
- persist `RunGoal` and create run records through `core`
- add CLI:
  - `run <goal-file>`
  - `resume [run-id|latest]`
  - `abort [run-id|latest]`

### Deliverables
- parsing + validation layer
- goal-run creation path
- run id lookup and status lifecycle

---

## Phase 2 — Step Journal + Planner

### Objective
Turn runs into explicit, inspectable sequences.

### Implement
- compile goal packages into milestone/step plans
- represent steps explicitly in app layer
- persist step journal entries to trace
- step status taxonomy: planned / running / verified / repaired / blocked / failed / skipped

### Deliverables
- runtime step planner
- persisted step journal
- `replay` support for step-level summaries

---

## Phase 3 — Real Tool Execution

### Objective
Use the existing tool adapter as the real execution substrate.

### Implement
- route step types to concrete tool requests
- add “expected changes” / “allowed paths” metadata to mutation steps
- enrich step evidence with changed-path summary and bounded patch preview
- support scoped command execution profiles (build/test/lint/typecheck/generic)

### Deliverables
- step executor
- richer mutation evidence
- command profile support in run records

---

## Phase 4 — Verification Contract

### Objective
Make “done” externally checkable.

### Implement
- support verification checks from the goal package:
  - command checks
  - file existence checks
  - file content assertions
  - changed-path assertions
- milestone verification after each checkpoint
- terminal verification before success

### Deliverables
- verifier adapter or verifier module
- per-step and per-milestone verification records
- `success` only when all required evidence exists

---

## Phase 5 — Repair Loop

### Objective
Bounded autonomous recovery.

### Implement
- detect verification failures and map them to repairable/non-repairable classes
- feed back verification evidence to provider/planner
- retry with budget accounting
- persist repair attempts and reasons

### Deliverables
- repair state transitions
- bounded retry policy
- budget consumption tracking

---

## Phase 6 — Approval + Safety Gate

### Objective
Safe autonomy for real development work.

### Implement
- connect `classify_tool_request_risk()` to execution gating
- pause before high-risk operations when approval mode requires it
- persist pending approval step and request reason
- allow `resume` after approval
- make `abort` deterministic

### Deliverables
- real approval gate
- persisted pending-approval state
- operator-visible approval requests

---

## Phase 7 — Concurrency / Isolation Hardening

### Objective
Stop multi-run interference from undermining trust.

### Implement
- decide and document execution model:
  - single-writer only, or
  - advisory lockfile, or
  - one-worktree-per-run isolation
- lock or isolate state snapshot, trace log, memory, and append artifacts
- serialize provider session usage or explicitly declare provider-per-run isolation

### Deliverables
- lock/isolation design
- race-focused tests
- clear operator guarantees

---

## Phase 8 — Workflow Packs + Adapter SDK

### Objective
Support user-supplied developer workflows without fragmenting product truth.

### Implement
- define a workflow-pack contract:
  - input schema
  - planner hints
  - verifier hooks
  - allowed tools
  - risk profile
- keep orchestration inside AxonRunner
- expose adapters only for deterministic capability boundaries

### Deliverables
- adapter SDK docs
- at least two workflow packs:
  - app implementation flow
  - server/API implementation flow

---

## Phase 9 — Evals + Release Gate

### Objective
Make autonomous quality measurable.

### Implement
- golden repositories
- scored end-to-end runs
- trace/artifact scoring
- regression dashboards or at least machine-readable reports

### Deliverables
- eval harness
- release thresholds
- reproducible run corpus

---

## Phase 10 — Dogfood

### Objective
Use the product for real engineering work.

### Implement
- maintenance workflow
- refactor workflow
- release prep workflow
- spec-to-feature workflow

### Deliverables
- documented dogfood runs
- failure taxonomy based on real use
- narrow next-surface decisions from real workload evidence
