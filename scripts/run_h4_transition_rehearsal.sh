#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage:
  run_h4_transition_rehearsal.sh \
    --workspace-root <path> \
    --runtime-root <path> \
    --snapshot-root <path> \
    --health-file <path> \
    --apps-bin <path> \
    --h2-bin <path> \
    --report <path> \
    [--rollback-script <path>]

Env:
  H4_ALLOWED_DIFF             (default: 0)
  H4_ROLLBACK_SLO_MS          (default: 300000)
  H4_TOTAL_TIMEOUT_MS         (default: 600000)
  H4_RECOVERY_MAX_RETRIES     (default: 3)
  H4_RECOVERY_BACKOFF_MS      (default: 100)
  H4_RECOVERY_TIMEOUT_MS      (default: 10000)
  H4_RECOVERY_EXPECT_HEALTH   (default: running)
  H4_RECOVERY_PROBE_CMD       (optional)
USAGE
}

json_escape() {
    local input="${1-}"
    input="${input//\\/\\\\}"
    input="${input//\"/\\\"}"
    input="${input//$'\n'/\\n}"
    input="${input//$'\r'/\\r}"
    input="${input//$'\t'/\\t}"
    printf '%s' "${input}"
}

json_string() {
    local value="${1-}"
    printf '"%s"' "$(json_escape "${value}")"
}

json_errors() {
    local out="["
    local first=1
    local err
    for err in "${ERRORS[@]+${ERRORS[@]}}"; do
        if [[ "${first}" -eq 0 ]]; then
            out+=","
        fi
        first=0
        out+="$(json_string "${err}")"
    done
    out+="]"
    printf '%s' "${out}"
}

now_ms() {
    perl -MTime::HiRes=time -e 'printf("%.0f\n", time() * 1000)'
}

canonicalize_existing_dir() {
    local input="$1"
    if [[ ! -d "${input}" ]]; then
        return 1
    fi
    realpath "${input}"
}

canonicalize_existing_file() {
    local input="$1"
    if [[ ! -f "${input}" ]]; then
        return 1
    fi
    realpath "${input}"
}

canonicalize_path() {
    local input="$1"
    if [[ -e "${input}" ]]; then
        realpath "${input}"
        return
    fi

    local parent
    local base
    parent="$(dirname "${input}")"
    base="$(basename "${input}")"

    if [[ ! -d "${parent}" ]]; then
        return 1
    fi

    local parent_canon
    parent_canon="$(realpath "${parent}")"
    printf '%s/%s\n' "${parent_canon}" "${base}"
}

is_within_root() {
    local candidate="$1"
    local root="$2"

    if [[ "${candidate}" == "${root}" ]]; then
        return 0
    fi

    case "${candidate}" in
        "${root}"/*) return 0 ;;
        *) return 1 ;;
    esac
}

require_u64() {
    local key="$1"
    local value="$2"
    if [[ ! "${value}" =~ ^[0-9]+$ ]]; then
        ERRORS+=("invalid numeric env ${key}=${value}")
    fi
}

check_total_timeout() {
    local phase="$1"
    local elapsed
    elapsed=$(( $(now_ms) - START_MS ))
    if [[ "${elapsed}" -gt "${H4_TOTAL_TIMEOUT_MS}" ]]; then
        ERRORS+=("total timeout exceeded during ${phase}: elapsed_ms=${elapsed} limit_ms=${H4_TOTAL_TIMEOUT_MS}")
        TOTAL_TIMEOUT_EXCEEDED=true
        return 1
    fi
    return 0
}

build_payload() {
    cat <<JSON
{
  "suite": "h4_transition_rehearsal_v1",
  "allowed_diff": ${H4_ALLOWED_DIFF},
  "h2_gate": $(json_string "${h2_gate}"),
  "h2_diff_count": ${h2_diff_count},
  "rollback_recovered": ${rollback_recovered},
  "rollback_elapsed_ms": ${rollback_elapsed_ms},
  "rollback_slo_ms": ${H4_ROLLBACK_SLO_MS},
  "data_loss_files": ${data_loss_files},
  "passed": ${passed},
  "errors": $(json_errors)
}
JSON
}

WORKSPACE_ROOT=""
RUNTIME_ROOT=""
SNAPSHOT_ROOT=""
HEALTH_FILE=""
APPS_BIN=""
H2_BIN=""
REPORT_PATH=""
ROLLBACK_SCRIPT="scripts/rollback_recovery.sh"

while (($# > 0)); do
    case "$1" in
        --workspace-root)
            [[ $# -ge 2 ]] || {
                usage
                exit 2
            }
            WORKSPACE_ROOT="$2"
            shift 2
            ;;
        --runtime-root)
            [[ $# -ge 2 ]] || {
                usage
                exit 2
            }
            RUNTIME_ROOT="$2"
            shift 2
            ;;
        --snapshot-root)
            [[ $# -ge 2 ]] || {
                usage
                exit 2
            }
            SNAPSHOT_ROOT="$2"
            shift 2
            ;;
        --health-file)
            [[ $# -ge 2 ]] || {
                usage
                exit 2
            }
            HEALTH_FILE="$2"
            shift 2
            ;;
        --apps-bin)
            [[ $# -ge 2 ]] || {
                usage
                exit 2
            }
            APPS_BIN="$2"
            shift 2
            ;;
        --h2-bin)
            [[ $# -ge 2 ]] || {
                usage
                exit 2
            }
            H2_BIN="$2"
            shift 2
            ;;
        --report)
            [[ $# -ge 2 ]] || {
                usage
                exit 2
            }
            REPORT_PATH="$2"
            shift 2
            ;;
        --rollback-script)
            [[ $# -ge 2 ]] || {
                usage
                exit 2
            }
            ROLLBACK_SCRIPT="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage
            exit 2
            ;;
    esac
done

if [[ -z "${WORKSPACE_ROOT}" || -z "${RUNTIME_ROOT}" || -z "${SNAPSHOT_ROOT}" || -z "${HEALTH_FILE}" || -z "${APPS_BIN}" || -z "${H2_BIN}" || -z "${REPORT_PATH}" ]]; then
    usage
    exit 2
fi

H4_ALLOWED_DIFF="${H4_ALLOWED_DIFF:-0}"
H4_ROLLBACK_SLO_MS="${H4_ROLLBACK_SLO_MS:-300000}"
H4_TOTAL_TIMEOUT_MS="${H4_TOTAL_TIMEOUT_MS:-600000}"
H4_RECOVERY_MAX_RETRIES="${H4_RECOVERY_MAX_RETRIES:-3}"
H4_RECOVERY_BACKOFF_MS="${H4_RECOVERY_BACKOFF_MS:-100}"
H4_RECOVERY_TIMEOUT_MS="${H4_RECOVERY_TIMEOUT_MS:-10000}"
H4_RECOVERY_EXPECT_HEALTH="${H4_RECOVERY_EXPECT_HEALTH:-running}"
H4_RECOVERY_PROBE_CMD="${H4_RECOVERY_PROBE_CMD:-}"

declare -a ERRORS=()
START_MS="$(now_ms)"
TOTAL_TIMEOUT_EXCEEDED=false

h2_gate="fail"
h2_diff_count=-1
rollback_recovered=false
rollback_elapsed_ms=0
data_loss_files=0
passed=false

require_u64 "H4_ALLOWED_DIFF" "${H4_ALLOWED_DIFF}"
require_u64 "H4_ROLLBACK_SLO_MS" "${H4_ROLLBACK_SLO_MS}"
require_u64 "H4_TOTAL_TIMEOUT_MS" "${H4_TOTAL_TIMEOUT_MS}"
require_u64 "H4_RECOVERY_MAX_RETRIES" "${H4_RECOVERY_MAX_RETRIES}"
require_u64 "H4_RECOVERY_BACKOFF_MS" "${H4_RECOVERY_BACKOFF_MS}"
require_u64 "H4_RECOVERY_TIMEOUT_MS" "${H4_RECOVERY_TIMEOUT_MS}"

if [[ "${H4_RECOVERY_MAX_RETRIES}" =~ ^[0-9]+$ ]] && [[ "${H4_RECOVERY_MAX_RETRIES}" -eq 0 ]]; then
    ERRORS+=("H4_RECOVERY_MAX_RETRIES must be greater than 0")
fi

if [[ "${H4_TOTAL_TIMEOUT_MS}" =~ ^[0-9]+$ ]] && [[ "${H4_TOTAL_TIMEOUT_MS}" -eq 0 ]]; then
    ERRORS+=("H4_TOTAL_TIMEOUT_MS must be greater than 0")
fi

if [[ "${H4_RECOVERY_TIMEOUT_MS}" =~ ^[0-9]+$ ]] && [[ "${H4_RECOVERY_TIMEOUT_MS}" -eq 0 ]]; then
    ERRORS+=("H4_RECOVERY_TIMEOUT_MS must be greater than 0")
fi

WORKSPACE_CANON=""
RUNTIME_CANON=""
SNAPSHOT_CANON=""
HEALTH_CANON=""
REPORT_CANON=""
APPS_BIN_CANON=""
H2_BIN_CANON=""
ROLLBACK_SCRIPT_CANON=""

if ! WORKSPACE_CANON="$(canonicalize_existing_dir "${WORKSPACE_ROOT}")"; then
    ERRORS+=("workspace root does not exist: ${WORKSPACE_ROOT}")
fi

if ! RUNTIME_CANON="$(canonicalize_existing_dir "${RUNTIME_ROOT}")"; then
    ERRORS+=("runtime root does not exist: ${RUNTIME_ROOT}")
fi

if ! SNAPSHOT_CANON="$(canonicalize_existing_dir "${SNAPSHOT_ROOT}")"; then
    ERRORS+=("snapshot root does not exist: ${SNAPSHOT_ROOT}")
fi

if ! HEALTH_CANON="$(canonicalize_path "${HEALTH_FILE}")"; then
    ERRORS+=("health file parent must exist: ${HEALTH_FILE}")
fi

if ! REPORT_CANON="$(canonicalize_path "${REPORT_PATH}")"; then
    ERRORS+=("report path parent must exist: ${REPORT_PATH}")
fi

if ! APPS_BIN_CANON="$(canonicalize_existing_file "${APPS_BIN}")"; then
    ERRORS+=("apps bin does not exist: ${APPS_BIN}")
fi

if ! H2_BIN_CANON="$(canonicalize_existing_file "${H2_BIN}")"; then
    ERRORS+=("h2 bin does not exist: ${H2_BIN}")
fi

if ! ROLLBACK_SCRIPT_CANON="$(canonicalize_existing_file "${ROLLBACK_SCRIPT}")"; then
    ERRORS+=("rollback script does not exist: ${ROLLBACK_SCRIPT}")
fi

if [[ -n "${WORKSPACE_CANON}" && -n "${RUNTIME_CANON}" ]] && ! is_within_root "${RUNTIME_CANON}" "${WORKSPACE_CANON}"; then
    ERRORS+=("runtime root is outside workspace root")
fi

if [[ -n "${WORKSPACE_CANON}" && -n "${SNAPSHOT_CANON}" ]] && ! is_within_root "${SNAPSHOT_CANON}" "${WORKSPACE_CANON}"; then
    ERRORS+=("snapshot root is outside workspace root")
fi

if [[ -n "${WORKSPACE_CANON}" && -n "${HEALTH_CANON}" ]] && ! is_within_root "${HEALTH_CANON}" "${WORKSPACE_CANON}"; then
    ERRORS+=("health file is outside workspace root")
fi

if [[ -n "${WORKSPACE_CANON}" && -n "${REPORT_CANON}" ]] && ! is_within_root "${REPORT_CANON}" "${WORKSPACE_CANON}"; then
    ERRORS+=("report path is outside workspace root")
fi

if [[ -n "${APPS_BIN_CANON}" && ! -x "${APPS_BIN_CANON}" ]]; then
    ERRORS+=("apps bin is not executable: ${APPS_BIN_CANON}")
fi

if [[ -n "${H2_BIN_CANON}" && ! -x "${H2_BIN_CANON}" ]]; then
    ERRORS+=("h2 bin is not executable: ${H2_BIN_CANON}")
fi

if [[ -n "${ROLLBACK_SCRIPT_CANON}" && ! -x "${ROLLBACK_SCRIPT_CANON}" ]]; then
    ERRORS+=("rollback script is not executable: ${ROLLBACK_SCRIPT_CANON}")
fi

TMP_DIR=""
TMP_H2_REPORT=""
TMP_ROLLBACK_REPORT=""
TMP_ROLLBACK_REPORT_CANON=""
TMP_H2_STDOUT=""
TMP_H2_STDERR=""
TMP_ROLLBACK_STDOUT=""
TMP_ROLLBACK_STDERR=""

if [[ -n "${WORKSPACE_CANON}" ]]; then
    if ! TMP_DIR="$(mktemp -d "${WORKSPACE_CANON}/.h4_transition_rehearsal.XXXXXX")"; then
        ERRORS+=("failed to create temporary directory inside workspace root")
    fi
else
    if ! TMP_DIR="$(mktemp -d)"; then
        ERRORS+=("failed to create temporary directory")
    fi
fi

if [[ -n "${TMP_DIR}" ]]; then
    trap 'rm -rf "${TMP_DIR}"' EXIT
    TMP_H2_REPORT="${TMP_DIR}/h2_report.json"
    TMP_ROLLBACK_REPORT="${TMP_DIR}/rollback_report.json"
    TMP_H2_STDOUT="${TMP_DIR}/h2_stdout.log"
    TMP_H2_STDERR="${TMP_DIR}/h2_stderr.log"
    TMP_ROLLBACK_STDOUT="${TMP_DIR}/rollback_stdout.log"
    TMP_ROLLBACK_STDERR="${TMP_DIR}/rollback_stderr.log"
fi

if [[ "${#ERRORS[@]}" -eq 0 ]]; then
    if ! TMP_ROLLBACK_REPORT_CANON="$(canonicalize_path "${TMP_ROLLBACK_REPORT}")"; then
        ERRORS+=("temporary rollback report path parent must exist")
    elif ! is_within_root "${TMP_ROLLBACK_REPORT_CANON}" "${WORKSPACE_CANON}"; then
        ERRORS+=("temporary rollback report path is outside workspace root")
    fi
fi

if [[ "${#ERRORS[@]}" -eq 0 ]]; then
    if check_total_timeout "pre_h2"; then
        if "${H2_BIN_CANON}" --apps-bin "${APPS_BIN_CANON}" --allowed-diff "${H4_ALLOWED_DIFF}" --report "${TMP_H2_REPORT}" >"${TMP_H2_STDOUT}" 2>"${TMP_H2_STDERR}"; then
            H2_EXIT=0
        else
            H2_EXIT=$?
        fi

        if [[ ! -f "${TMP_H2_REPORT}" ]]; then
            ERRORS+=("h2 report was not generated")
        else
            if H2_PARSED="$(perl -MJSON::PP -e '
                my ($path) = @ARGV;
                local $/;
                open my $fh, "<", $path or exit 2;
                my $doc = eval { decode_json(<$fh>) };
                exit 3 if $@ || ref($doc) ne "HASH";
                my $gate = $doc->{gate};
                my $diff = $doc->{diff_count};
                exit 4 if !defined($gate) || ref($gate);
                exit 5 if $gate ne "pass" && $gate ne "fail";
                exit 6 if !defined($diff) || ref($diff) || $diff !~ /\A[0-9]+\z/;
                print $gate, "\n", $diff, "\n";
            ' "${TMP_H2_REPORT}")"; then
                h2_gate="$(printf '%s\n' "${H2_PARSED}" | sed -n '1p')"
                h2_diff_count="$(printf '%s\n' "${H2_PARSED}" | sed -n '2p')"
            else
                ERRORS+=("failed to parse h2 report")
            fi
        fi

        if [[ "${H2_EXIT}" -ne 0 ]]; then
            if [[ "${h2_gate}" == "pass" ]]; then
                ERRORS+=("h2 verifier exited non-zero with gate=pass")
            fi
            h2_gate="fail"
        fi
    fi
fi

if [[ "${#ERRORS[@]}" -eq 0 && "${TOTAL_TIMEOUT_EXCEEDED}" != true ]]; then
    if check_total_timeout "pre_rollback"; then
        rollback_started_ms="$(now_ms)"
        if RECOVERY_MAX_RETRIES="${H4_RECOVERY_MAX_RETRIES}" \
            RECOVERY_BACKOFF_MS="${H4_RECOVERY_BACKOFF_MS}" \
            RECOVERY_TIMEOUT_MS="${H4_RECOVERY_TIMEOUT_MS}" \
            RECOVERY_EXPECT_HEALTH="${H4_RECOVERY_EXPECT_HEALTH}" \
            RECOVERY_PROBE_CMD="${H4_RECOVERY_PROBE_CMD}" \
            "${ROLLBACK_SCRIPT_CANON}" \
            --workspace-root "${WORKSPACE_CANON}" \
            --runtime-root "${RUNTIME_CANON}" \
            --snapshot-root "${SNAPSHOT_CANON}" \
            --health-file "${HEALTH_CANON}" \
            --report "${TMP_ROLLBACK_REPORT_CANON}" >"${TMP_ROLLBACK_STDOUT}" 2>"${TMP_ROLLBACK_STDERR}"; then
            ROLLBACK_EXIT=0
        else
            ROLLBACK_EXIT=$?
        fi
        rollback_ended_ms="$(now_ms)"
        rollback_elapsed_ms=$((rollback_ended_ms - rollback_started_ms))

        if [[ ! -f "${TMP_ROLLBACK_REPORT_CANON}" ]]; then
            ERRORS+=("rollback report was not generated")
        else
            if ROLLBACK_PARSED="$(perl -MJSON::PP=decode_json -e '
                my ($path) = @ARGV;
                local $/;
                open my $fh, "<", $path or exit 2;
                my $doc = eval { decode_json(<$fh>) };
                exit 3 if $@ || ref($doc) ne "HASH";
                my $recovered = $doc->{recovered};
                my $elapsed = $doc->{elapsed_ms};
                exit 4 if !defined($recovered) || !JSON::PP::is_bool($recovered);
                exit 5 if !defined($elapsed) || ref($elapsed) || $elapsed !~ /\A[0-9]+\z/;
                print(($recovered ? "true" : "false"), "\n", $elapsed, "\n");
            ' "${TMP_ROLLBACK_REPORT_CANON}")"; then
                rollback_recovered="$(printf '%s\n' "${ROLLBACK_PARSED}" | sed -n '1p')"
                rollback_elapsed_ms="$(printf '%s\n' "${ROLLBACK_PARSED}" | sed -n '2p')"
            else
                ERRORS+=("failed to parse rollback report")
            fi
        fi

        if [[ "${ROLLBACK_EXIT}" -ne 0 ]]; then
            rollback_recovered=false
        fi
    fi
fi

if [[ "${#ERRORS[@]}" -eq 0 && "${TOTAL_TIMEOUT_EXCEEDED}" != true ]]; then
    check_total_timeout "pre_data_loss" || true
fi

FILES_TO_CHECK=("config.toml" "memory/MEMORY.md")
for rel in "${FILES_TO_CHECK[@]}"; do
    src="${SNAPSHOT_CANON}/${rel}"
    dst="${RUNTIME_CANON}/${rel}"
    if [[ ! -f "${src}" ]]; then
        data_loss_files=$((data_loss_files + 1))
        ERRORS+=("snapshot file missing: ${src}")
        continue
    fi
    if [[ ! -f "${dst}" ]]; then
        data_loss_files=$((data_loss_files + 1))
        ERRORS+=("runtime file missing: ${dst}")
        continue
    fi
    if ! cmp -s "${src}" "${dst}"; then
        data_loss_files=$((data_loss_files + 1))
    fi
done

if [[ ! "${rollback_elapsed_ms}" =~ ^[0-9]+$ ]]; then
    ERRORS+=("rollback_elapsed_ms missing or non-numeric: ${rollback_elapsed_ms:-<empty>}")
fi

if [[ "${h2_gate}" == "pass" && "${rollback_recovered}" == "true" && "${data_loss_files}" -eq 0 && "${#ERRORS[@]}" -eq 0 && "${rollback_elapsed_ms}" -le "${H4_ROLLBACK_SLO_MS}" ]]; then
    passed=true
else
    passed=false
fi

PAYLOAD="$(build_payload)"
if ! printf '%s\n' "${PAYLOAD}" > "${REPORT_CANON}"; then
    ERRORS+=("failed to write report file: ${REPORT_CANON}")
    passed=false
    PAYLOAD="$(build_payload)"
fi
printf '%s\n' "${PAYLOAD}"

if [[ "${passed}" == true ]]; then
    exit 0
fi
exit 1
