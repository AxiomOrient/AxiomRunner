#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage:
  rollback_recovery.sh \
    --workspace-root <path> \
    --runtime-root <path> \
    --snapshot-root <path> \
    --health-file <path> \
    --report <path>
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

json_string_or_null() {
    local value="${1-}"
    if [[ -z "${value}" ]]; then
        printf 'null'
        return
    fi
    printf '"%s"' "$(json_escape "${value}")"
}

now_ms() {
    perl -MTime::HiRes=time -e 'printf("%.0f\n", time() * 1000)'
}

sleep_ms() {
    local ms="$1"
    perl -e 'select(undef, undef, undef, $ARGV[0] / 1000)' "${ms}"
}

canonicalize_existing_dir() {
    local input="$1"
    if [[ ! -d "${input}" ]]; then
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

emit_report() {
    local end_ms
    local elapsed
    local payload

    end_ms="$(now_ms)"
    if [[ -n "${START_MS}" ]]; then
        elapsed=$((end_ms - START_MS))
    else
        elapsed=0
    fi

    payload="$(cat <<JSON
{
  "workspace_root": $(json_string_or_null "${WORKSPACE_CANON}"),
  "runtime_root": $(json_string_or_null "${RUNTIME_CANON}"),
  "snapshot_root": $(json_string_or_null "${SNAPSHOT_CANON}"),
  "health_file": $(json_string_or_null "${HEALTH_CANON}"),
  "report": $(json_string_or_null "${REPORT_CANON}"),
  "expected_health": $(json_string_or_null "${RECOVERY_EXPECT_HEALTH}"),
  "probe_cmd": $(json_string_or_null "${RECOVERY_PROBE_CMD}"),
  "max_retries": ${RECOVERY_MAX_RETRIES},
  "backoff_ms": ${RECOVERY_BACKOFF_MS},
  "timeout_ms": ${RECOVERY_TIMEOUT_MS},
  "attempts": ${ATTEMPTS},
  "elapsed_ms": ${elapsed},
  "timed_out": ${TIMED_OUT},
  "recovered": ${RECOVERED},
  "last_error": $(json_string_or_null "${LAST_ERROR}")
}
JSON
)"

    printf '%s\n' "${payload}"

    if [[ "${REPORT_WRITABLE}" -eq 1 ]]; then
        printf '%s\n' "${payload}" > "${REPORT_CANON}"
    fi
}

fail_with() {
    local message="$1"
    local code="$2"
    LAST_ERROR="${message}"
    RECOVERED=false
    emit_report
    exit "${code}"
}

require_u64() {
    local key="$1"
    local value="$2"

    if [[ ! "${value}" =~ ^[0-9]+$ ]]; then
        fail_with "invalid numeric env ${key}=${value}" 2
    fi
}

WORKSPACE_ROOT=""
RUNTIME_ROOT=""
SNAPSHOT_ROOT=""
HEALTH_FILE=""
REPORT_PATH=""

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
        --report)
            [[ $# -ge 2 ]] || {
                usage
                exit 2
            }
            REPORT_PATH="$2"
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

if [[ -z "${WORKSPACE_ROOT}" || -z "${RUNTIME_ROOT}" || -z "${SNAPSHOT_ROOT}" || -z "${HEALTH_FILE}" || -z "${REPORT_PATH}" ]]; then
    usage
    exit 2
fi

RECOVERY_MAX_RETRIES="${RECOVERY_MAX_RETRIES:-3}"
RECOVERY_BACKOFF_MS="${RECOVERY_BACKOFF_MS:-100}"
RECOVERY_TIMEOUT_MS="${RECOVERY_TIMEOUT_MS:-10000}"
RECOVERY_EXPECT_HEALTH="${RECOVERY_EXPECT_HEALTH:-running}"
RECOVERY_PROBE_CMD="${RECOVERY_PROBE_CMD:-}"

WORKSPACE_CANON=""
RUNTIME_CANON=""
SNAPSHOT_CANON=""
HEALTH_CANON=""
REPORT_CANON=""
REPORT_WRITABLE=0

ATTEMPTS=0
TIMED_OUT=false
RECOVERED=false
LAST_ERROR=""
START_MS="$(now_ms)"

require_u64 "RECOVERY_MAX_RETRIES" "${RECOVERY_MAX_RETRIES}"
require_u64 "RECOVERY_BACKOFF_MS" "${RECOVERY_BACKOFF_MS}"
require_u64 "RECOVERY_TIMEOUT_MS" "${RECOVERY_TIMEOUT_MS}"

if [[ "${RECOVERY_MAX_RETRIES}" -eq 0 ]]; then
    fail_with "RECOVERY_MAX_RETRIES must be greater than 0" 2
fi

if [[ "${RECOVERY_TIMEOUT_MS}" -eq 0 ]]; then
    fail_with "RECOVERY_TIMEOUT_MS must be greater than 0" 2
fi

if ! WORKSPACE_CANON="$(canonicalize_existing_dir "${WORKSPACE_ROOT}")"; then
    fail_with "workspace root does not exist: ${WORKSPACE_ROOT}" 2
fi

if ! REPORT_CANON="$(canonicalize_path "${REPORT_PATH}")"; then
    fail_with "report path parent must exist: ${REPORT_PATH}" 2
fi

if ! is_within_root "${REPORT_CANON}" "${WORKSPACE_CANON}"; then
    fail_with "report path is outside workspace root" 1
fi
REPORT_WRITABLE=1

if ! RUNTIME_CANON="$(canonicalize_existing_dir "${RUNTIME_ROOT}")"; then
    fail_with "runtime root does not exist: ${RUNTIME_ROOT}" 1
fi

if ! SNAPSHOT_CANON="$(canonicalize_existing_dir "${SNAPSHOT_ROOT}")"; then
    fail_with "snapshot root does not exist: ${SNAPSHOT_ROOT}" 1
fi

if ! HEALTH_CANON="$(canonicalize_path "${HEALTH_FILE}")"; then
    fail_with "health file parent must exist: ${HEALTH_FILE}" 1
fi

if ! is_within_root "${RUNTIME_CANON}" "${WORKSPACE_CANON}"; then
    fail_with "runtime root is outside workspace root" 1
fi

if ! is_within_root "${SNAPSHOT_CANON}" "${WORKSPACE_CANON}"; then
    fail_with "snapshot root is outside workspace root" 1
fi

if ! is_within_root "${HEALTH_CANON}" "${WORKSPACE_CANON}"; then
    fail_with "health file is outside workspace root" 1
fi

CONFIG_SRC="${SNAPSHOT_CANON}/config.toml"
MEMORY_SRC="${SNAPSHOT_CANON}/memory/MEMORY.md"
CONFIG_DST="${RUNTIME_CANON}/config.toml"
MEMORY_DST="${RUNTIME_CANON}/memory/MEMORY.md"

for ((attempt = 1; attempt <= RECOVERY_MAX_RETRIES; attempt++)); do
    ATTEMPTS="${attempt}"

    elapsed_before=$(( $(now_ms) - START_MS ))
    if [[ "${elapsed_before}" -ge "${RECOVERY_TIMEOUT_MS}" ]]; then
        TIMED_OUT=true
        LAST_ERROR="timeout reached before attempt ${attempt}"
        break
    fi

    if [[ ! -f "${CONFIG_SRC}" ]]; then
        LAST_ERROR="snapshot missing config.toml at ${CONFIG_SRC}"
        break
    fi

    if [[ ! -f "${MEMORY_SRC}" ]]; then
        LAST_ERROR="snapshot missing memory/MEMORY.md at ${MEMORY_SRC}"
        break
    fi

    if ! mkdir -p "${RUNTIME_CANON}/memory"; then
        LAST_ERROR="failed to prepare runtime memory directory"
        break
    fi

    if ! cp "${CONFIG_SRC}" "${CONFIG_DST}"; then
        LAST_ERROR="failed to restore config.toml"
        break
    fi

    if ! cp "${MEMORY_SRC}" "${MEMORY_DST}"; then
        LAST_ERROR="failed to restore memory/MEMORY.md"
        break
    fi

    if [[ -n "${RECOVERY_PROBE_CMD}" ]]; then
        if bash -lc "${RECOVERY_PROBE_CMD}"; then
            RECOVERED=true
            LAST_ERROR=""
            break
        fi
        probe_code=$?
        LAST_ERROR="probe command failed at attempt ${attempt} with exit ${probe_code}"
    else
        if [[ ! -f "${HEALTH_CANON}" ]]; then
            LAST_ERROR="health file not found: ${HEALTH_CANON}"
        elif grep -Fq "${RECOVERY_EXPECT_HEALTH}" "${HEALTH_CANON}"; then
            RECOVERED=true
            LAST_ERROR=""
            break
        else
            LAST_ERROR="health file missing expected token: ${RECOVERY_EXPECT_HEALTH}"
        fi
    fi

    elapsed_after=$(( $(now_ms) - START_MS ))
    if [[ "${elapsed_after}" -ge "${RECOVERY_TIMEOUT_MS}" ]]; then
        TIMED_OUT=true
        LAST_ERROR="timeout reached after attempt ${attempt}"
        break
    fi

    if [[ "${attempt}" -lt "${RECOVERY_MAX_RETRIES}" && "${RECOVERY_BACKOFF_MS}" -gt 0 ]]; then
        sleep_ms "${RECOVERY_BACKOFF_MS}"
    fi
done

if [[ "${RECOVERED}" != true && "${TIMED_OUT}" != true && "${ATTEMPTS}" -ge "${RECOVERY_MAX_RETRIES}" ]]; then
    if [[ -z "${LAST_ERROR}" ]]; then
        LAST_ERROR="recovery failed after max retries"
    fi
fi

emit_report
if [[ "${RECOVERED}" == true ]]; then
    exit 0
fi
exit 1
