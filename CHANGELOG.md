# Changelog

## 0.0.1

- locked product identity to `AxiomRunner` / `axiomrunner_apps` / `AXIOMRUNNER_*`
- reduced AxiomRunner to a minimal CLI agent runtime
- kept only `run`, `status`, `replay`, `resume`, `abort`, `doctor`, `health`, `help`
- shipped baseline already includes lifecycle commands, status/replay/doctor/report surfaces, workspace lock, representative examples, and nightly dogfood
- current backlog is hardening-first: verification truth, constraint enforcement, workspace safety evidence, pack completion, and release gate tightening
- removed `batch` and single-intent compatibility aliases from the product surface
- removed multi-channel, daemon, gateway, service, cron, skills, integrations, benchmark, and rehearsal surfaces
- collapsed workspace to `core`, `apps`, `adapters`
- pinned `codex-runtime` to `0.5.0` and documented minimum supported Codex CLI `0.104.0`
- upgraded patch and command artifacts for operator-facing evidence
