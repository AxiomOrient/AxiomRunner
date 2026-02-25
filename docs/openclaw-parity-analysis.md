# OpenClaw Parity Analysis (axiomAi vs zeroclaw)

## Scope
- Analyzed repositories:
  - `axiomAi` (145 files, excluding `target/` and `.git/`)
  - `zeroclaw` (124 files, excluding `.git/`)
- File-level manifests:
  - `analysis_manifest.tsv`
  - `docs/zeroclaw_manifest.tsv`

## Architecture Findings
- `zeroclaw` parity-critical strengths:
  - broad adapter surface (providers/channels/tools/memory/runtime)
  - OpenClaw-style bootstrap context pipeline
  - strict CI required gate (`fmt/clippy/test/build` + docs-only fast path)
- `axiomAi` current strengths:
  - clear core/schema separation
  - deterministic policy/event/reducer tests
  - migration/release-readiness scripts and transition gates

## Highest Gaps (P0/P1/P2)
- `P0`: CI required gate coverage too narrow in main CI workflow.
- `P1`: adapter breadth behind zeroclaw (provider/channel variety).
- `P2`: OpenClaw bootstrap context injection missing from runtime provider prompt path.

## Implemented In This Pass
- `apps/src/daemon.rs`
  - Added daemon health pointer state file support.
  - New env key: `AXIOM_DAEMON_HEALTH_STATE_PATH`.
  - Daemon now writes latest health file location to a stable pointer path.
- `apps/src/cli_runtime.rs`
  - `status`/`doctor` now read daemon health via:
    1) `AXIOM_DAEMON_HEALTH_PATH`
    2) fallback pointer from daemon state path.
- `apps/src/runtime_compose.rs`
  - Added OpenClaw-style bootstrap context injection into provider prompt.
  - New env key: `AXIOM_RUNTIME_BOOTSTRAP_ROOT`.
  - Reads bootstrap files such as `AGENTS.md`, `SOUL.md`, `IDENTITY.md`, `USER.md`, `HEARTBEAT.md`, `TOOLS.md`, `BOOTSTRAP.md`, `MEMORY.md` when present.
- `.github/workflows/ci.yml`
  - Rebuilt into docs-only aware required gate pipeline:
    - change-scope detection
    - lint (`fmt`, `clippy`)
    - tests
    - release build smoke
    - release-security-gate tests
    - required status enforcement job.

## Verification Run
- `cargo fmt --all`
- `cargo clippy -p axiom_apps --all-targets -- -D warnings`
- `cargo test -p axiom_apps daemon::tests::health_state_path_round_trip_resolves_health_file`
- `cargo test -p axiom_apps runtime_compose::tests::runtime_compose_appends_bootstrap_context_from_workspace_files`
- `cargo test -p axiom_apps --test e2e_cli e2e_cli_status_reads_daemon_health_from_state_pointer_when_env_missing`
- `cargo test -p axiom_apps --test e2e_cli e2e_cli_status_includes_runtime_daemon_and_channel_summary`
- `cargo test -p axiom_apps --test e2e_cli e2e_cli_doctor_reports_deterministic_summary`

## Remaining Work For Stronger Parity
- Expand provider/channel adapters toward zeroclaw breadth.
- Align adapter contract traits with concrete implementations in `adapters/`.
- Add identity format loader (markdown + structured config) and deeper channel/runtime injection points.
