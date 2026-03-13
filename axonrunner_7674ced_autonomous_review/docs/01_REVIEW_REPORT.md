# 01. Review Report

## Executive verdict

AxonRunner is now a **coherent minimal runtime**, not a scattered platform.
The current implementation is materially better than the earlier post-board state.

What is already good:

- narrow product surface (`run`, `batch`, `doctor`, `replay`, `status`, `health`, `help`, plus thin legacy aliases)
- event-sourced core remains small and intelligible
- provider failure propagates as process failure instead of looking like success
- state snapshot compatibility for legacy `readonly` is fixed
- workspace tool surface is now broad enough for real engineering work (`list/read/search/write/replace/remove/run-command`)
- report/trace/replay/doctor artifacts exist and are tested
- `codek` / `codex-runtime` substrate contract is explicit and probed

What is **not** done yet:

- the autonomous product is still **not canonical**
- the current public contract is still legacy `intent-spec`, not `goal/run`
- the runtime loop is still too shallow for autonomous app/server development
- verification is mostly structural, not true milestone verification
- approval policy is modeled but not fully enforced in the execution loop
- concurrent process safety is not yet product-grade

## Product-level judgment

**Current state:** strong v1 runtime hardening baseline.

**Not yet:** a trustworthy autonomous developer agent that can take a planning/spec document, execute a multi-step workflow, verify milestone completion, repair failures, and continue automatically.

The repository now contains enough substrate to build that product without widening into ZeroClaw-style breadth. The next move is not more platform surface. The next move is to make the autonomous `goal/run` loop the product truth.

## The strongest parts

### 1. Product truth got narrower
The repository is now centered around three crates: `apps`, `core`, `adapters`. That is the right shape.

### 2. Core domain is small and durable
`core` still has the strongest design in the project:
- intent / decision / effect / event / reducer / projection / state boundaries remain explicit
- autonomous scaffolding (`RunGoal`, budget, done conditions, verification checks, run events/status) has been added without exploding the domain

### 3. Failure semantics improved
Provider failure now blocks memory/tool follow-on work and is surfaced as runtime failure with trace/report evidence.
That closes one of the most serious earlier product lies.

### 4. Tool contract is finally usable
The tool adapter now supports the essential file-system and command operations required for dev automation.
This is the minimum viable substrate for build/test/lint/edit/search workflows.

### 5. Observability is product-shaped
`doctor`, `trace_store`, `replay`, report artifacts, and changed-path evidence are all moving in the right direction.
This is exactly the kind of inspectability an autonomous harness needs.

## The biggest remaining problems

### P0-1. Autonomous contract is still scaffold, not product truth
`core` already models `RunGoal`, budgets, approval modes, done conditions, and run events.
But `apps` still executes legacy single-intent templates as the canonical surface.

This creates a split-brain product:
- the domain says “goal-oriented autonomous run”
- the CLI says “write/read/remove/freeze/halt runtime”

That must be resolved.

### P0-2. The tool execution path is still shallow
The runtime compose path still treats tool execution primarily as a log append/report writer.
The tool adapter itself is broad enough, but the runtime loop is not yet using it as an autonomous step engine.

### P0-3. Verification is not yet strong enough for autonomous development
The runtime has report artifacts and a `verify_run` phase, but it is still mostly checking:
- provider/memory/tool stage status
- patch evidence existence
- tool output existence

That is not enough for real app/server automation.
A real autonomous run must verify milestone completion using commands and assertions from the goal contract.

### P0-4. Approval is modeled but not actually enforced end-to-end
Risk tiers are computed for tools, and the runtime can produce `approval_required` outcomes.
But there is not yet a canonical approval gate that intercepts risky tool steps before execution, persists pending approval state, and supports `resume` after approval.

### P1-1. Concurrent-process safety is still weak
Within one process, the project is reasonably careful.
Across multiple processes sharing the same workspace/state, it is not yet product-grade:
- trace log append uses plain JSONL append with no file lock
- file append writes use plain append with no coordination
- markdown memory rewrites are atomic per writer, but not lock-protected across writers
- state snapshot is atomic per write, but still last-writer-wins across processes
- cached provider session reuse may allow concurrent `ask()` calls on the same active session

### P1-2. Hidden fallback still exists in async host init
`async_runtime_host` falls back to a default runtime if configured initialization fails.
That is operationally convenient, but philosophically dangerous because the project explicitly wants no hidden fallback.

### P1-3. Some tool semantics still need tightening
- `replace_in_file` is broad and should support expected replacement count / single-match modes
- `search_files` should surface unreadable/skipped files rather than silently ignoring them
- `remove_path` needs stronger approval plumbing before it is considered product-safe in autonomous mode

## Final review conclusion

AxonRunner is now **worth continuing**.

The next product should **not** widen toward channels, dashboards, skills marketplaces, or broad framework abstraction.
It should narrow further into:

> one goal, one workspace, one run loop, one evidence trail.

That is the shortest path to a serious autonomous developer agent.
