# Release Evidence — 1.0.0

**Date**: 2026-03-17
**Version**: 1.0.0
**Platform**: darwin (macOS)

## Automated Test Evidence

| Suite | Tests | Result |
|-------|-------|--------|
| `axiomrunner_adapters` (lib) | 23 passed | ✓ |
| `contracts` | 6 passed | ✓ |
| `error` | 2 passed | ✓ |
| `memory` | 6 passed | ✓ |
| `tool` | 22 passed | ✓ |
| `axiomrunner_apps` (lib) | 68 passed | ✓ |
| `config_priority` | 18 passed | ✓ |
| `autonomous_eval_corpus` | 1 passed | ✓ |
| `e2e_cli` | 42 passed | ✓ |
| `fault_path_suite` | 1 passed | ✓ |
| `nightly_dogfood_contract` | 1 passed | ✓ |
| `release_security_gate` | 40 passed | ✓ |
| `axiomrunner_core` (lib) | 9 passed | ✓ |
| **Total** | **239 passed** | **✓** |

Run: `cargo test` — 239 passed, 0 failed

## Release Gate

`cargo test --test release_security_gate --test autonomous_eval_corpus --test nightly_dogfood_contract` — 42 passed

## Focused Release Review

이번 릴리즈 검토에서 4개 개선 사항에 대해 아래 회귀 근거를 추가 확인했다.

| Change | Verification | Result |
|--------|--------------|--------|
| `alloc_seq_id` 추출 | `cargo test -q alloc_seq_id_increments_and_saturates` | ✓ |
| `goal_done_condition_failure` 순수 함수화 + runtime workspace 경계 | `cargo test -q goal_done_condition_failure_uses_runtime_workspace_boundary` | ✓ |
| done-condition check explicit composition | `cargo test -q apply_goal_done_conditions_explicitly_composes_existing_and_new_checks` | ✓ |
| `hex_encode_bytes` 최적화 + restore payload 유지 | `cargo test -q hex_encode_bytes_renders_dense_lowercase_hex` / `cargo test -q write_file_restore_artifact_persists_hex_payload` | ✓ |

추가로 `cargo test` 전체 실행에서 아래 기존 E2E 회귀가 함께 통과했다.

- `e2e_cli_goal_file_uses_bounded_repair_budget`
- `e2e_cli_goal_file_zero_repair_budget_does_not_claim_attempt`
- `e2e_cli_goal_file_done_condition_uses_runtime_workspace_not_goal_workspace_root`

## Example Smoke (schema validation)

다음 4개 예제의 `goal.json` + `pack.json` 파싱이 goal_file_rejects 계열 테스트에서 정상 처리 확인됨.

| Example | goal.json | pack.json | Status |
|---------|-----------|-----------|--------|
| `rust_service` | verifier: cargo build/test/clippy | pack_id: rust-service-basic | ✓ parseable |
| `node_api` | verifier: npm test | pack_id: node-api-basic | ✓ parseable |
| `nextjs_app` | verifier: npm test | pack_id: nextjs-app-basic | ✓ parseable |
| `python_fastapi` | verifier: pytest | pack_id: python-fastapi-basic | ✓ parseable |

Note: verifier command validation (`validate_run_command_spec`) 통과 확인 — `cargo`, `npm`, `pytest` 모두 allowlist 포함.

## Key Contract Points (1.0.0)

- `workspace_root` is a compatibility-only field; runtime workspace boundary is set by `--workspace`
- git workspace defaults to isolated worktree execution; disable with `AXIOMRUNNER_RUNTIME_GIT_WORKTREE_ISOLATION=0`
- command surface is allowlisted command spec (program + arg shape), not just program name
- `python -c`, `node -e`, inline interpreter args are denied at parse, plan, and execution layers
- commit-stage failures (memory, snapshot, report write) are promoted to trace events for replay/status visibility
- provider/tool/memory step failures are promoted to process failure, never hidden as success
