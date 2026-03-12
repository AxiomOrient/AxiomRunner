# AxonRunner Charter

## Purpose

AxonRunner is a minimal event-sourced CLI agent runtime.

## Retained Surface

- `agent`
- `read`
- `write`
- `remove`
- `freeze`
- `halt`
- `batch`
- `status`
- `health`

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
- `crates/adapters`: memory/provider/tool/agent implementations

## Rule

If a feature does not directly support the retained CLI surface, remove it instead of abstracting it.
