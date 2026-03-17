# Changelog

## 0.2.0

- retained CLI surface를 `run`, `status`, `replay`, `resume`, `abort`, `doctor`, `health`, `help` 로 고정했다
- goal-file 중심 단일 런타임으로 제품면을 다시 정리하고 문서 truth lock을 맞췄다
- verification truth, rollback evidence, nightly dogfood, release gate 묶음을 릴리즈 기준으로 잠갔다
- representative verifier examples와 workflow pack 경계를 현재 제품 계약에 맞게 정리했다
- 이전 제품 이름 잔재와 경로 흔적을 정리했다
- `cli_runtime.rs`의 ID 할당 로직을 `alloc_seq_id` 순수 함수로 고정해 mutation point를 한 곳으로 줄였다
- `lifecycle.rs`의 repair loop 데이터 흐름을 정리해 초기 verification shadowing과 말미 dead path를 제거했다
- `tool_write.rs`의 hex 인코딩을 사전 할당 + 직접 쓰기 패턴으로 바꿔 바이트당 `String` 할당을 없앴다
- `lifecycle.rs`의 done-condition 평가를 반환값 기반으로 바꿔 외부 `Vec` mutation 없이 check 합성이 드러나게 했다

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
