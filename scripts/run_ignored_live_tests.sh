#!/usr/bin/env bash
set -euo pipefail

echo "[ignored-live] running ignored live tests in a separate lane"
echo "[ignored-live] OPENAI_API_KEY: ${OPENAI_API_KEY:+set}"
echo "[ignored-live] COMPOSIO_API_KEY: ${COMPOSIO_API_KEY:+set}"
echo "[ignored-live] AXONRUNNER_RUN_AXIOMME_LIVE: ${AXONRUNNER_RUN_AXIOMME_LIVE:-0}"
echo "[ignored-live] AXONRUNNER_RUN_DELEGATE_LIVE: ${AXONRUNNER_RUN_DELEGATE_LIVE:-0}"

# Apps ignored live tests (network-tolerant smoke)
cargo test --locked -q -p axonrunner_apps -- --ignored --nocapture

# Adapters ignored live tests (API/network dependent; tests self-skip when env/setup is missing)
cargo test --locked -q -p axonrunner_adapters -- --ignored --nocapture
