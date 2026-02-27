#!/usr/bin/env bash
set -euo pipefail

echo "[ignored-live] running ignored live tests in a separate lane"
echo "[ignored-live] OPENAI_API_KEY: ${OPENAI_API_KEY:+set}"
echo "[ignored-live] COMPOSIO_API_KEY: ${COMPOSIO_API_KEY:+set}"
echo "[ignored-live] AXIOM_RUN_AXIOMME_LIVE: ${AXIOM_RUN_AXIOMME_LIVE:-0}"
echo "[ignored-live] AXIOM_RUN_DELEGATE_LIVE: ${AXIOM_RUN_DELEGATE_LIVE:-0}"

# Apps ignored live tests (network-tolerant smoke)
cargo test --locked -q -p axiom_apps -- --ignored --nocapture

# Adapters ignored live tests (API/network dependent; tests self-skip when env/setup is missing)
cargo test --locked -q -p axiom_adapters -- --ignored --nocapture
