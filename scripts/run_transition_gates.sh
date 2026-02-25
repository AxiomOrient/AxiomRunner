#!/usr/bin/env bash
set -euo pipefail

repo_root="$(pwd)"
cd "${repo_root}"

if [[ ! -f Cargo.toml ]]; then
    echo "error: run from repository root" >&2
    exit 2
fi

TG_H4_ALLOWED_DIFF="${TG_H4_ALLOWED_DIFF:-0}"
TG_H4_ROLLBACK_SLO_MS="${TG_H4_ROLLBACK_SLO_MS:-300000}"
TG_H4_TOTAL_TIMEOUT_MS="${TG_H4_TOTAL_TIMEOUT_MS:-600000}"

for key in TG_H4_ALLOWED_DIFF TG_H4_ROLLBACK_SLO_MS TG_H4_TOTAL_TIMEOUT_MS; do
    value="${!key}"
    if [[ ! "${value}" =~ ^[0-9]+$ ]]; then
        echo "error: ${key} must be a non-negative integer" >&2
        exit 2
    fi
done

apps_bin="${repo_root}/target/debug/axiom_apps"
h2_bin="${repo_root}/target/debug/h2_verify"

echo "step: build bins"
cargo build -q -p axiom_apps --bin axiom_apps --bin h2_verify

echo "step: test h2_parallel"
cargo test -q -p axiom_apps h2_parallel

echo "step: test rollback_recovery_h3"
cargo test -q -p axiom_apps rollback_recovery_h3

echo "step: test transition_rehearsal_h4"
cargo test -q -p axiom_apps transition_rehearsal_h4

tmp_workspace="$(mktemp -d "${TMPDIR:-/tmp}/transition-gates.h4.XXXXXX")"
cleanup() {
    rm -rf "${tmp_workspace}"
}
trap cleanup EXIT

runtime_root="${tmp_workspace}/runtime"
snapshot_root="${tmp_workspace}/snapshot"
health_file="${runtime_root}/health.status"
report_file="${tmp_workspace}/h4_transition_report.json"
sample_stdout="${tmp_workspace}/h4_transition_stdout.log"

echo "step: setup h4 sample"
mkdir -p "${runtime_root}/memory" "${snapshot_root}/memory"

cat >"${snapshot_root}/config.toml" <<'EOF'
profile = "safe"
version = 7
EOF

cat >"${snapshot_root}/memory/MEMORY.md" <<'EOF'
# Snapshot Memory
- baseline fact
EOF

cat >"${runtime_root}/config.toml" <<'EOF'
profile = "drifted"
version = 11
EOF

cat >"${runtime_root}/memory/MEMORY.md" <<'EOF'
# Runtime Memory
- stale fact
EOF

printf 'state=running\n' >"${health_file}"

echo "step: run h4 sample rehearsal"
if ! H4_ALLOWED_DIFF="${TG_H4_ALLOWED_DIFF}" \
    H4_ROLLBACK_SLO_MS="${TG_H4_ROLLBACK_SLO_MS}" \
    H4_TOTAL_TIMEOUT_MS="${TG_H4_TOTAL_TIMEOUT_MS}" \
    bash scripts/run_h4_transition_rehearsal.sh \
    --workspace-root "${tmp_workspace}" \
    --runtime-root "${runtime_root}" \
    --snapshot-root "${snapshot_root}" \
    --health-file "${health_file}" \
    --apps-bin "${apps_bin}" \
    --h2-bin "${h2_bin}" \
    --report "${report_file}" >"${sample_stdout}"; then
    cat "${sample_stdout}"
    exit 1
fi

echo "step: parse h4 sample report"
perl -MJSON::PP -e '
    my ($path) = @ARGV;
    local $/;
    open my $fh, "<", $path or die "open failed: $!";
    my $doc = eval { JSON::PP::decode_json(<$fh>) };
    die "invalid json\n" if $@ || ref($doc) ne "HASH";
' "${report_file}"

artifact_dir="${repo_root}/target/transition-gates"
mkdir -p "${artifact_dir}"
cp "${report_file}" "${artifact_dir}/h4_sample_report.json"

echo "step: complete"
