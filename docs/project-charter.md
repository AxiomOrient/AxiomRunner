# AxiomAI Project Charter

Date: 2026-02-20
Status: Active

## 1. Mission
- Build AxiomAI as a reliable agentic automator.
- Optimize for correct automation outcomes, controllable execution, and fast recovery.
- Do not optimize for feature-count parity with another project.

## 2. Product Definition
- AxiomAI is a data-driven automation runtime.
- It takes typed intent input, evaluates policy, derives explicit effects, executes adapters, and returns observable outcomes.
- It must remain operable under failure: bounded retries, explicit stop conditions, and rollback-ready transitions.

## 3. Non-Goals
- 1:1 imitation of ZeroClaw/OpenClaw feature surface.
- Score chasing as a product goal.
- Decorative abstractions, implicit global state, and hidden side-effect chains.

## 4. Engineering Principles
- Data first:
  - Make data models explicit and primary.
  - Keep invariants visible in types and validation paths.
- Pure transforms first:
  - Keep reducers/projections/policy evaluation pure and deterministic.
  - Isolate side effects at runtime/adapters boundaries.
- Simplicity first:
  - Prefer concrete structures over deep hierarchies.
  - Add abstraction only when it removes real duplication or clarifies invariants.
- Performance and control:
  - Make allocation, ownership, complexity, timeout, retry, and queue limits explicit.
  - Avoid hidden process spawning, hidden IO fan-out, and hidden retries.
- Operational clarity:
  - Emit actionable logs for failure boundaries.
  - Keep rollback path executable and tested.

## 5. Architectural Contract
- `core/`: pure domain logic (intent -> policy -> decision -> effects -> projection).
- `apps/`: orchestration, CLI/runtime control plane, and explicit side-effect sequencing.
- `adapters/`: provider/channel/tool/memory/runtime edges; contract-checked and failure-classified.
- `schema/`: compatibility and config merge rules.

## 6. Release Contract
- Release gate blocks only safety regressions:
  - correctness tests
  - compatibility/migration safety
  - rollback/recovery validation
  - observability minimums
- Release gate must not block on arbitrary parity targets.

## 7. Decision Policy for New Work
- A feature is accepted only if all are true:
  - Improves real automation outcomes.
  - Fits the data-first + explicit-side-effect model.
  - Keeps failure modes observable and recoverable.
  - Has clear ownership and bounded operational cost.
- Otherwise: reject, defer, or implement as optional plugin.

## 8. Success Signals
- Stable pass on core purity + contract tests + release safety tests.
- Reduced unknown-failure rate via explicit error boundaries.
- Faster diagnosis and rollback time in transition rehearsals.
