#!/usr/bin/env bash
set -euo pipefail

if [[ ! -f Cargo.toml ]]; then
  echo "error: run from repository root" >&2
  exit 2
fi

echo "step: integrations info status variants"
cargo test --locked -q -p axonrunner_apps --test e2e_cli \
  e2e_cli_integrations_info_supports_case_insensitive_and_status_variants

echo "step: integrations list executable status sync"
cargo test --locked -q -p axonrunner_apps --test e2e_cli \
  e2e_cli_integrations_list_syncs_executable_statuses

echo "step: channel add/start/doctor/remove contract"
cargo test --locked -q -p axonrunner_apps --test e2e_cli \
  e2e_cli_channel_add_start_doctor_remove_flow

echo "execution contract gate passed"
