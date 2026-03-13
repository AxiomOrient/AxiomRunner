# 04. Next Product Definition

## One-sentence definition

**AxonRunner should become a single-agent, single-workspace autonomous developer harness that takes an explicit goal package, executes it step by step, verifies completion, repairs failures, and leaves a replayable evidence trail.**

## What it is not

It is **not**:
- a multi-channel agent platform
- a gateway/server product
- a daemon/cron ecosystem
- a skills marketplace
- a multi-agent orchestration system by default
- a broad swappable-abstract-everything framework

## The right mental model

The correct model is:

1. **Externalized target**
   - spec, constraints, deliverables, non-goals, validations, current status live in files
2. **Single workspace boundary**
   - one run touches one workspace or worktree
3. **Explicit run state machine**
   - planning -> executing -> verifying -> repairing -> completed/blocked/approval_required/failed/aborted
4. **Deterministic substrate + agentic loop**
   - tools and verifiers are deterministic contracts
   - planning/repair remain model-driven
5. **Verify-before-done**
   - a run cannot claim success without evidence for every declared done condition
6. **Evidence-first autonomy**
   - patch artifacts, commands, outputs, validations, run summaries, failure boundaries must be replayable

## Why this is better than copying ZeroClaw breadth

For your use case—automating app/server development from planning documents—breadth is not the bottleneck.
The bottleneck is **trustworthy long-horizon execution**.

That means the winning product is not the biggest framework.
It is the one that gets these few things right:
- goal contract
- workspace/worktree isolation
- step execution
- verification
- repair loop
- approvals
- replay/report

## Best architecture choice for your adapter idea

### Your current idea
> “개발 워크플로우와 각종 도구들은 내가 개발해서 제공 할거야. 어댑터 형식으로.”

### Verdict
This is **mostly the right idea**, but only if AxonRunner owns the orchestration contract.

### Keep in AxonRunner core product
AxonRunner itself should own:
- goal schema
- run state machine
- step model
- budget model
- approval model
- trace/report/replay schema
- completion/verification contract
- resume/abort semantics

### Put behind adapters
Adapters should own:
- provider substrate (`codek`, optional compat providers)
- workspace tool primitives
- verifier execution backends
- optional domain-specific workflow packs or planners
- optional SCM/worktree operations

### What not to do
Do **not** let each external adapter invent its own run lifecycle or completion semantics.
If that happens, the product becomes impossible to reason about.

## Recommended contract shape

### 1. Goal package
A run starts from a goal package such as:
- `GOAL.md` or `goal.yaml`
- linked `SPEC.md`
- linked `PLAN.md`
- linked `IMPLEMENT.md`
- linked `STATUS.md`
- optional `AGENTS.md`

### 2. Canonical CLI
Target public surface:

```bash
axonrunner_apps run <goal-file>
axonrunner_apps status [run-id|latest]
axonrunner_apps replay [run-id|latest]
axonrunner_apps doctor [--json]
axonrunner_apps resume [run-id|latest]
axonrunner_apps abort [run-id|latest]
```

Legacy fact-intent commands remain only as migration/debug paths.

### 3. Canonical run phases
- Planning
- ExecutingStep
- Verifying
- Repairing
- WaitingApproval
- Blocked
- Completed
- Failed
- Aborted

### 4. Terminal outcomes
- success
- blocked
- budget_exhausted
- approval_required
- failed
- aborted

### 5. Canonical step types
The minimum step vocabulary should be small:
- inspect_files
- read_files
- search_files
- mutate_files
- run_verification_command
- summarize_state
- request_approval

## What “simple but complete” means here

Simplicity does **not** mean fewer capabilities.
It means:
- one obvious way to run work
- one obvious place to inspect state
- one obvious definition of done
- one obvious evidence format
- one obvious resume story

That is the product to build.
