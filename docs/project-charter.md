# AxonRunner Charter

## Purpose

AxonRunner is a minimal event-sourced CLI agent runtime.

## Retained Surface

- `run`
- `batch`
- `doctor`
- `replay`
- `status`
- `health`
- `help`
- legacy aliases: `read`, `write`, `remove`, `freeze`, `halt`

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
