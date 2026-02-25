# Release Security Transition (G-4)

Current release boundary:

- Release builds (`cfg!(debug_assertions) == false`) block startup when `profile=dev`.
- Debug builds (`cfg!(debug_assertions) == true`) keep dev-minimal mode permissive.
- Enforcement remains in `apps/src/dev_guard.rs` and is invoked from `apps/src/main.rs`.

Legacy bypass vectors are explicitly blocked:

- `--allow-dev-in-release` is rejected as an unknown startup option (exit code `2`).
- `AXIOM_ALLOW_DEV_IN_RELEASE` has no effect; release startup still fails on `--profile=dev`.
- `DEV_MODE=true`, `BIND=127.0.0.1`, `ALLOW_REMOTE=false` tuple has no effect in release builds; startup still fails on `--profile=dev`.
- `allow_dev_in_release` config key is rejected with `config error: unknown config key 'allow_dev_in_release'`.

Primary command (unified renewal gate):

```bash
bash scripts/run_renewal_readiness.sh
```

E4 CLI transition operations guide:

- [`docs/e4-cli-transition-ops-guide.md`](e4-cli-transition-ops-guide.md)

Gate evidence:

- `cargo test -q -p axiom_apps --test release_security_gate`
- `cargo test -q --release -p axiom_apps --test release_security_gate`
- Readiness report fields: `release_gate_debug`, `release_gate_release` in `target/release-readiness/report.json`
- Readiness logs: `target/release-readiness/release_gate_debug.log`, `target/release-readiness/release_gate_release.log`
- Separate readiness-step evidence for adapters: `cargo test -q -p axiom_adapters --tests`, `adapter_contract` field, and `target/release-readiness/adapter_contract.log`
- Neighboring CLI contract evidence: `cli_contract` field and `target/release-readiness/cli_contract.log`

Plan mapping in the unified gate:

- `G-4 release gate pass (debug+release tests)`
- Required alongside separate readiness steps `adapter_contract` (adapter tests), `E4 CLI contract check`, `F-5 benchmarks pass`, and `H-2/H-3/H-4 transition gates pass`

Artifacts:

- `target/release-readiness/report.json`
- `benchmarks/summary.md`
- `target/transition-gates/h4_sample_report.json`
