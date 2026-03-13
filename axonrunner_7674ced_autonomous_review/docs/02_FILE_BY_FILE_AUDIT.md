# 02. File-by-File Audit

This audit is organized by repository path and role.
Each entry is classified as one of:

- **Keep**: correct direction, should remain
- **Tighten**: good asset, but semantics or hardening are incomplete
- **Promote**: should become part of the next canonical product contract
- **Constrain**: keep, but behind stronger rules/approvals/contracts

---

## Root

### `.gitignore`
- **Role:** repository hygiene
- **Assessment:** Keep
- **Notes:** no product concern

### `CHANGELOG.md`
- **Role:** release history / truth surface support
- **Assessment:** Tighten
- **Notes:** must stay aligned with current product truth and the next `goal/run` transition

### `Cargo.lock`
- **Role:** reproducibility
- **Assessment:** Keep
- **Notes:** critical once `codex-runtime` compatibility is pinned and release-gated

### `Cargo.toml`
- **Role:** workspace definition
- **Assessment:** Keep
- **Notes:** current 3-crate workspace is correct; keep this narrow shape

### `LICENSE`
- **Role:** legal baseline
- **Assessment:** Keep

### `README.md`
- **Role:** public product truth
- **Assessment:** Tighten
- **Notes:** current truth is much better, but the next product shift must not create another split between runtime reality and docs

---

## `docs/`

### `docs/project-charter.md`
- **Role:** canonical project identity
- **Assessment:** Keep
- **Notes:** correct north star; explicitly resists platform sprawl

### `docs/CAPABILITY_MATRIX.md`
- **Role:** truth-lock for current surface
- **Assessment:** Keep
- **Notes:** useful release gate; should gain autonomous transition blockers later

### `docs/CODEK_RUNTIME_CONTRACT.md`
- **Role:** substrate contract for `codek`
- **Assessment:** Keep
- **Notes:** strong document; next step is to add concurrency and version-gate semantics

### `docs/RUNBOOK.md`
- **Role:** operator usage
- **Assessment:** Tighten
- **Notes:** should evolve toward goal-run lifecycle, not remain legacy-intent-centric forever

### `docs/VERSIONING.md`
- **Role:** release/version semantics
- **Assessment:** Keep
- **Notes:** important once goal schema becomes public contract

### `docs/AUTONOMOUS_AGENT_TARGET.md`
- **Role:** bridge to next product
- **Assessment:** Promote
- **Notes:** concise and correct; should be turned into actual canonical contract later

### `docs/AUTONOMOUS_AGENT_SPEC.md`
- **Role:** target goal/run schema
- **Assessment:** Promote
- **Notes:** strong direction; needs implementation parity

### `docs/DOCS_ALIGNMENT.md`
- **Role:** current vs target truth disambiguation
- **Assessment:** Keep
- **Notes:** useful during transition only; should eventually disappear once target becomes current truth

### `docs/roadmap/*`
- **Role:** transition planning
- **Assessment:** Tighten
- **Notes:** replace legacy roadmap drift with product-owned milestones once autonomous contract becomes canonical

---

## `plans/`

### `plans/IMPLEMENTATION-PLAN.md`
- **Role:** tactical project plan
- **Assessment:** Tighten
- **Notes:** post-board hardening items are mostly closed; needs reset around autonomous product milestones

### `plans/TASKS.md`
- **Role:** task ledger
- **Assessment:** Tighten
- **Notes:** move from hardening tasks to product-definition tasks; keep as execution ledger only

---

## `crates/core/src`

### `audit.rs`
- **Role:** policy audit/event metadata
- **Assessment:** Keep
- **Notes:** core observability asset

### `decision.rs`
- **Role:** decision calculation
- **Assessment:** Keep
- **Notes:** small and correct boundary

### `effect.rs`
- **Role:** effect model
- **Assessment:** Keep
- **Notes:** still valid; should remain compact

### `event.rs`
- **Role:** domain event schema
- **Assessment:** Keep
- **Notes:** addition of run events is correct and should be used more fully

### `intent.rs`
- **Role:** intent schema + autonomous goal scaffolding
- **Assessment:** Promote
- **Notes:** the important next-product types already live here; this file should stop being aspirational and start driving the app layer

### `lib.rs`
- **Role:** public re-exports
- **Assessment:** Keep
- **Notes:** clean surface matters here

### `policy.rs`
- **Role:** policy evaluation
- **Assessment:** Keep
- **Notes:** strong core asset

### `policy_codes.rs`
- **Role:** policy code taxonomy
- **Assessment:** Keep
- **Notes:** essential for replayable failure semantics

### `projection.rs`
- **Role:** event projection
- **Assessment:** Keep
- **Notes:** good basis for autonomous run status projections

### `reducer.rs`
- **Role:** autonomous run-state reducer
- **Assessment:** Promote
- **Notes:** now meaningful because run-state scaffolding exists

### `state.rs`
- **Role:** current agent state + run state
- **Assessment:** Keep
- **Notes:** product-critical once `goal/run` becomes canonical

### `validation.rs`
- **Role:** schema boundary / validation
- **Assessment:** Tighten
- **Notes:** should become the guardrail for public goal schema ingestion

---

## `crates/core/tests`

### `domain_flow.rs`
- **Assessment:** Keep
- **Notes:** core acceptance baseline

### `policy_codes.rs`
- **Assessment:** Keep

### `policy_decision.rs`
- **Assessment:** Keep

### `projection_replay.rs`
- **Assessment:** Keep

### `reducer_cases.rs`
- **Assessment:** Promote
- **Notes:** should expand with budget exhaustion / resume / abort / approval cases

### `schema_boundaries.rs`
- **Assessment:** Tighten
- **Notes:** should become stronger once public goal file/schema is introduced

### `state_invariants.rs`
- **Assessment:** Keep

---

## `crates/adapters/src`

### `contracts.rs`
- **Role:** substrate contract definitions
- **Assessment:** Keep
- **Notes:** this is one of the strongest files in the repo; do not bloat it

### `error.rs`
- **Role:** adapter error taxonomy
- **Assessment:** Keep
- **Notes:** useful for stable operator-visible failure reasons

### `lib.rs`
- **Role:** adapter exports/builders
- **Assessment:** Tighten
- **Notes:** keep it as thin assembly only

### `memory.rs`
- **Role:** memory builder and tiering
- **Assessment:** Tighten
- **Notes:** fine today; will need explicit single-writer/concurrency policy

### `memory_markdown.rs`
- **Role:** lightweight recall backend
- **Assessment:** Constrain
- **Notes:** acceptable for local/simple runs; not ideal as primary concurrent backend

### `memory_sqlite.rs`
- **Role:** stronger recall backend
- **Assessment:** Keep
- **Notes:** better default for real autonomous runs; likely should become canonical

### `provider_codex_runtime.rs`
- **Role:** primary execution substrate provider
- **Assessment:** Tighten
- **Notes:** good direction, but session concurrency semantics need to be made explicit and enforced

### `provider_openai.rs`
- **Role:** compat fallback provider
- **Assessment:** Constrain
- **Notes:** keep experimental; do not let it blur the core product path

### `provider_registry.rs`
- **Role:** provider catalog / resolution
- **Assessment:** Keep
- **Notes:** small and useful

### `tool.rs`
- **Role:** workspace tool adapter
- **Assessment:** Tighten
- **Notes:** very important file; good breadth, but append concurrency, replacement constraints, and approval integration need work

### `tool_workspace.rs`
- **Role:** workspace boundary resolution
- **Assessment:** Keep
- **Notes:** essential safety asset

### `tool_write.rs`
- **Role:** patch/evidence writing helpers
- **Assessment:** Keep
- **Notes:** strong implementation area; evidence can still become richer

---

## `crates/adapters/tests`

### `error.rs`
- **Assessment:** Keep

### `memory.rs`
- **Assessment:** Tighten
- **Notes:** add concurrency and corruption-recovery cases

### `tool.rs`
- **Assessment:** Keep
- **Notes:** one of the strongest test assets; add approval and inter-process cases

---

## `crates/apps/src`

### `async_runtime_host.rs`
- **Role:** async call host / bounded execution helper
- **Assessment:** Tighten
- **Notes:** fallback mode is convenient, but philosophically conflicts with no-hidden-fallback

### `cli_args.rs`
- **Role:** argv parsing shell
- **Assessment:** Tighten
- **Notes:** current parsing is fine for legacy runtime; next product must add `run <goal-file>`, `resume`, `abort`

### `cli_command.rs`
- **Role:** command model
- **Assessment:** Promote
- **Notes:** this is where current truth must be replaced carefully

### `cli_runtime.rs`
- **Role:** top-level run orchestration
- **Assessment:** Tighten
- **Notes:** strongest current application file, but still legacy-intent-centric and structurally-verified rather than truly autonomous

### `config_loader.rs`
- **Role:** configuration loading/precedence
- **Assessment:** Keep
- **Notes:** must remain boring and deterministic

### `dev_guard.rs`
- **Role:** dev-only protections
- **Assessment:** Keep
- **Notes:** useful if kept minimal

### `display.rs`
- **Role:** human-readable formatting
- **Assessment:** Keep
- **Notes:** should be expanded only in support of operator clarity

### `doctor.rs`
- **Role:** machine/human diagnostics
- **Assessment:** Keep
- **Notes:** strong product feature already

### `env_util.rs`
- **Role:** small env helpers
- **Assessment:** Keep

### `lib.rs`
- **Role:** crate composition
- **Assessment:** Keep

### `main.rs`
- **Role:** process entrypoint
- **Assessment:** Keep
- **Notes:** should stay extremely thin

### `parse_util.rs`
- **Role:** parsing utilities
- **Assessment:** Keep

### `replay.rs`
- **Role:** replay/report rendering
- **Assessment:** Keep
- **Notes:** should become more central in the autonomous product

### `runtime_compose.rs`
- **Role:** provider/memory/tool composition and execution
- **Assessment:** Tighten
- **Notes:** the most important application file to evolve next; currently too shallow for real goal execution

### `runtime_compose/plan.rs`
- **Role:** run-plan shaping
- **Assessment:** Promote
- **Notes:** natural home for step planning once goal-run becomes canonical

### `state_store.rs`
- **Role:** persisted state snapshot
- **Assessment:** Tighten
- **Notes:** snapshot compatibility is improved; still needs explicit single-writer semantics or locking

### `status.rs`
- **Role:** status rendering
- **Assessment:** Keep

### `trace_store.rs`
- **Role:** append-only trace log and replay basis
- **Assessment:** Tighten
- **Notes:** strong basis, but process-safe append semantics need work

---

## `crates/apps/tests`

### `common/*`
- **Assessment:** Keep

### `config_priority.rs`
- **Assessment:** Keep

### `e2e_cli.rs`
- **Assessment:** Keep
- **Notes:** currently the single most important regression file in the repo; should be expanded into autonomous golden runs

### `release_security_gate.rs`
- **Assessment:** Keep
- **Notes:** exactly the kind of release discipline this project needs

---

## Overall file-by-file conclusion

The repository does **not** need structural expansion.
It needs **promotion** of existing autonomous scaffolding and **tightening** of runtime semantics, approvals, verification, and concurrency.
