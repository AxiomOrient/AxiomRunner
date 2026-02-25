# E4 CLI Transition Operations Guide

As of February 17, 2026, this guide is the operator-facing runbook for the E4 CLI contract.

## Related Transition Docs

- [Unified Renewal Readiness Gate](release-readiness-gate.md)
- [Release Security Transition (G-4)](release-security-transition.md)
- [Transition Gate Automation (H-2/H-3/H-4)](transition-gates.md)

## Canonical CLI Contract

Top-level commands:

- `read <key>`
- `write <key> <value>`
- `remove <key>`
- `freeze`
- `halt`
- `status`
- `health`
- `batch [--reset-state] <intent-spec>...`
- `serve --mode=<gateway|daemon>`

Intent spec forms accepted by `batch`:

- `read:<key>`
- `write:<key>=<value>`
- `remove:<key>`
- `freeze`
- `halt`

## Exact Old -> New Command Mapping

| Old automation command | New canonical command | Notes |
|---|---|---|
| `apply <intent-spec>` | `batch <intent-spec>` | `apply` token is removed. |
| `replay <intent-spec>...` | `batch --reset-state <intent-spec>...` | Use `--reset-state` only when old replay semantics required state reset. |
| `gateway` | `serve --mode=gateway` | `gateway` token is removed. |
| `daemon` | `serve --mode=daemon` | `daemon` token is removed. |

Removed tokens now fail startup with exit code `2` and `unknown command '<token>'` on stderr.

## Batch Workflow (Operators and CI)

Set one binary path for all jobs:

```bash
CLI_BIN="./target/debug/axiom_apps"
```

Standard batch flow (no reset):

```bash
"${CLI_BIN}" --actor=system batch \
  write:alpha=1 \
  read:alpha \
  freeze \
  halt
```

Replay-equivalent flow (explicit reset):

```bash
"${CLI_BIN}" --actor=system batch --reset-state \
  write:alpha=1 \
  remove:alpha
```

Expected success signal:

- stdout includes `batch completed count=...`
- exit code is `0`

## Serve Workflow (Gateway/Daemon)

Gateway mode:

```bash
"${CLI_BIN}" --profile=prod --endpoint=http://127.0.0.1:8080 serve --mode=gateway
```

Daemon mode:

```bash
"${CLI_BIN}" --profile=prod --endpoint=http://127.0.0.1:8080 serve --mode=daemon
```

Release boundary note:

- Release builds block `--profile=dev` with exit code `2` and `release gate blocked startup` on stderr.
- Legacy bypass vectors are blocked: `--allow-dev-in-release` is unknown, `AXIOM_ALLOW_DEV_IN_RELEASE` does not bypass release startup, and `allow_dev_in_release` is an unknown config key.

## Automation Migration Checklist

- [ ] Inventory old command tokens in jobs/scripts:
  `rg -n "\b(apply|replay|gateway|daemon)\b" scripts .github docs`
- [ ] Rewrite every invocation to canonical forms (`batch`, `serve --mode=...`).
- [ ] Replace replay-style jobs with explicit `batch --reset-state`.
- [ ] Remove legacy bypass attempts (`--allow-dev-in-release`, `AXIOM_ALLOW_DEV_IN_RELEASE`, `allow_dev_in_release`).
- [ ] Update runbooks and alerts to parse `batch completed` and `serve --mode=...` logs.
- [ ] Run validation commands below before promotion.

## Validation and Troubleshooting Commands

Contract smoke tests:

```bash
CLI_BIN="./target/debug/axiom_apps"
"${CLI_BIN}" status
"${CLI_BIN}" health
"${CLI_BIN}" batch write:key=value read:key
"${CLI_BIN}" batch --reset-state write:key=value remove:key
"${CLI_BIN}" serve --mode=gateway
"${CLI_BIN}" serve --mode=daemon
```

Negative-path checks for removed/invalid commands:

```bash
CLI_BIN="./target/debug/axiom_apps"
"${CLI_BIN}" apply write:key=value || true
"${CLI_BIN}" replay write:key=value || true
"${CLI_BIN}" gateway || true
"${CLI_BIN}" serve --mode=worker || true
```

Gate-level validations:

```bash
cargo test -q -p axiom_apps release_gate_s2
cargo test -q --release -p axiom_apps release_gate_s2
cargo test -q -p axiom_adapters --tests
cargo test -q -p axiom_apps e2e_cli
bash scripts/run_transition_gates.sh
bash scripts/run_renewal_readiness.sh
cat target/release-readiness/report.json
```

Readiness evidence locations after `bash scripts/run_renewal_readiness.sh`:

- `target/release-readiness/report.json` fields: `suite`, `release_gate_debug`, `release_gate_release`, `adapter_contract`, `cli_contract`, `transition_gates`, `benchmarks`, `passed`, `errors`
- `target/release-readiness/adapter_contract.log`
- `target/release-readiness/cli_contract.log`
- `target/release-readiness/release_gate_debug.log`
- `target/release-readiness/release_gate_release.log`
- `target/release-readiness/transition_gates.log`
- `target/release-readiness/benchmarks.log`

If startup fails, read stderr first. E4 startup failures are intentionally explicit and stable.

## Rollback and Mitigation Notes

Primary mitigation for broken legacy automation: add a temporary command shim in the caller script while jobs are migrated.

```bash
#!/usr/bin/env bash
set -euo pipefail

CLI_BIN="./target/debug/axiom_apps"

case "${1:-}" in
  apply)
    shift
    set -- batch "$@"
    ;;
  replay)
    shift
    set -- batch --reset-state "$@"
    ;;
  gateway)
    shift
    set -- serve --mode=gateway "$@"
    ;;
  daemon)
    shift
    set -- serve --mode=daemon "$@"
    ;;
esac

exec "${CLI_BIN}" "$@"
```

Mitigation policy:

- Keep shim lifetime short and tracked.
- Prefer fixing source automation immediately.
- If release startup is blocked by `profile=dev`, move jobs to `--profile=prod` (or approved non-dev profile) instead of bypassing guardrails.
