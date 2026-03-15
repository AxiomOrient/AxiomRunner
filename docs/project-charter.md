# AxiomRunner Charter

## Purpose

AxiomRunner is a goal-file oriented CLI agent runtime.
Its retained operator identity is `AxiomRunner` / `axiomrunner_apps` / `AXIOMRUNNER_*`.

## Transition Target

The current contract remains the retained CLI runtime surface below.
The next product target is a single-workspace autonomous agent that can
plan, act, verify, repair, and report against an explicit goal contract.

Transition references:

- `docs/AUTONOMOUS_AGENT_TARGET.md`
- `docs/AUTONOMOUS_AGENT_SPEC.md`
- `docs/WORKFLOW_PACK_CONTRACT.md`

## Retained Surface

- `run`
- `status`
- `replay`
- `resume`
- `abort`
- `doctor`
- `health`
- `help`

## Non-goals

- multi-channel messaging
- daemon and service lifecycle management
- HTTP gateway mode
- cron scheduling
- skills marketplace
- integrations catalog
- benchmark and rehearsal automation

## Structure

- `crates/core`: domain and policy
- `crates/apps`: CLI runtime
- `crates/adapters`: memory/provider/tool implementations

## Rule

If a feature does not directly support the retained CLI surface, remove it instead of abstracting it.
`resume` is approval-only pending goal-file control, not a generic interrupted-run resume.
