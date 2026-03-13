# 05. Roadmap

## North star

From:
- minimal event-sourced CLI runtime

To:
- single-agent autonomous developer harness for app/server implementation workflows

## Phase 0 — Truth Lock

### Goal
Prevent another contract split.

### Outcomes
- current runtime truth remains explicit
- next-product target is documented without ambiguity
- no hidden fallbacks or undocumented behavior changes

### Exit criteria
- README / charter / capability matrix / runbook / CLI help all agree
- `doctor` exposes substrate compatibility and fallback state clearly
- current release blockers are green

## Phase 1 — Goal/Run Contract

### Goal
Make `goal/run` the canonical public surface.

### Outcomes
- public goal schema exists
- `run <goal-file>` exists
- run lifecycle, terminal outcomes, budget, approval policy, done conditions are public contract
- legacy intent runtime becomes debug/migration only

### Exit criteria
- a goal file can be parsed, validated, persisted, replayed
- `status`, `replay`, `resume`, `abort` operate on run ids

## Phase 2 — Step Engine

### Goal
Turn the current shallow compose path into a real step executor.

### Outcomes
- plan is compiled into explicit steps
- each step has inputs, outputs, evidence, status, failure boundary
- runtime uses actual workspace tools, not only log append/report writing

### Exit criteria
- a goal run produces a step journal
- tool calls are driven by step specs
- changed paths and evidence are step-scoped

## Phase 3 — Verification + Repair

### Goal
Success means the requested work actually works.

### Outcomes
- verification checks are first-class
- build/test/lint/typecheck/file assertions can be attached to milestones and done conditions
- repair loops can retry failed verification with bounded budgets

### Exit criteria
- a run cannot report success without verification evidence
- repair attempts are traceable and bounded

## Phase 4 — Safety + Control

### Goal
Allow real autonomy without losing operator control.

### Outcomes
- risk-tiered approval gate
- resume/abort semantics
- budget exhaustion semantics
- explicit no-hidden-fallback policy
- worktree/workspace isolation contract

### Exit criteria
- destructive steps pause correctly under `on-risk`/`always`
- abort leaves a coherent trace and final report
- multi-run interference risk is documented or prevented

## Phase 5 — Developer Workflow Packs + Adapters

### Goal
Support spec-driven app/server automation through adapter-based workflows.

### Outcomes
- stable adapter SDK for developer workflows
- workflow pack format for recurring engineering procedures
- user-provided workflow/verifier/tool adapters plug into one orchestration contract

### Exit criteria
- planning doc -> workflow pack -> autonomous execution path exists
- at least one app workflow and one server workflow run end-to-end

## Phase 6 — Evals + Product Hardening

### Goal
Ship only what is proven.

### Outcomes
- golden repositories and eval suites
- scorecards for success rate, repair rate, regression rate, artifact quality
- release gates around autonomous runs

### Exit criteria
- representative app/server goals pass repeatedly
- failures are inspectable and actionable
- release decisions are driven by evals, not vibes

## Phase 7 — Dogfooding + Narrow Expansion

### Goal
Use AxonRunner to build AxonRunner-adjacent engineering work.

### Outcomes
- project maintenance, refactors, bugfixes, release prep run through the product
- only then add narrow new capabilities when real workflows demand them

### Exit criteria
- the agent is trusted for recurring internal engineering work
- additions are driven by failed real tasks, not framework ambition
