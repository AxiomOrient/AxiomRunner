# Changelog

## 0.0.1

- reduced AxonRunner to a minimal CLI agent runtime
- kept only `run`, `batch`, `doctor`, `replay`, `status`, `health`, `help`
- kept legacy single-intent aliases `read`, `write`, `remove`, `freeze`, `halt`
- removed multi-channel, daemon, gateway, service, cron, skills, integrations, benchmark, and rehearsal surfaces
- collapsed workspace to `core`, `apps`, `adapters`
- pinned `codex-runtime` to `0.5.0` and documented minimum supported Codex CLI `0.104.0`
- upgraded patch and command artifacts for operator-facing evidence
