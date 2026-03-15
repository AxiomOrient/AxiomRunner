# Changelog

## 0.2.0

- retained CLI surface를 `run`, `status`, `replay`, `resume`, `abort`, `doctor`, `health`, `help` 로 고정했다
- goal-file 중심 단일 런타임으로 제품면을 다시 정리하고 문서 truth lock을 맞췄다
- verification truth, rollback evidence, nightly dogfood, release gate 묶음을 릴리즈 기준으로 잠갔다
- representative verifier examples와 workflow pack 경계를 현재 제품 계약에 맞게 정리했다
- 이전 제품 이름 잔재와 경로 흔적을 정리했다

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
