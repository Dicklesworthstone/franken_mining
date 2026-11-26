#!/usr/bin/env bash
# Reusable bash helpers for FrankenMining tests/e2e/ scripts (BD-142, plan §33.5).
# Source this file from any tests/e2e/*.sh script. Every step invokes a real binary, logs one
# strict NDJSON event, captures a SHA-256-verified CrashPack on failure, and renders summary.md
# strictly from events.ndjson.

E2E_HARNESS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
E2E_REPO_ROOT="$(cd "$E2E_HARNESS_DIR/../.." && pwd)"
E2E_HARNESS_PY="$E2E_HARNESS_DIR/harness.py"

e2e_resolve_fmine_bin() {
    if [ -n "${FMINE_BIN:-}" ] && [ -x "$FMINE_BIN" ]; then
        printf '%s\n' "$FMINE_BIN"
        return 0
    fi
    if [ -n "${FMINE_TEST_BINARY:-}" ] && [ -x "$FMINE_TEST_BINARY" ]; then
        printf '%s\n' "$FMINE_TEST_BINARY"
        return 0
    fi
    if [ -x "$E2E_REPO_ROOT/target/debug/fmine" ]; then
        printf '%s\n' "$E2E_REPO_ROOT/target/debug/fmine"
        return 0
    fi
    if [ -x "$E2E_REPO_ROOT/target/release/fmine" ]; then
        printf '%s\n' "$E2E_REPO_ROOT/target/release/fmine"
        return 0
    fi
    (cd "$E2E_REPO_ROOT" && cargo build -q -p fmine-cli) || return 1
    printf '%s\n' "$E2E_REPO_ROOT/target/debug/fmine"
}

# e2e_init <suite_name> [run_dir]
e2e_init() {
    E2E_SUITE="$1"
    if [ -n "${2:-}" ]; then
        E2E_RUN_DIR="$2"
        mkdir -p "$E2E_RUN_DIR"
    else
        E2E_RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fmine-e2e-${E2E_SUITE}.XXXXXX")"
    fi
    E2E_LOG_FILE="$E2E_RUN_DIR/events.ndjson"
    E2E_SUMMARY_FILE="$E2E_RUN_DIR/summary.md"
    E2E_DIGEST_TMP="$E2E_RUN_DIR/.last_digest"
    E2E_LAST_DIGEST=""
    E2E_FAILURES=0
    E2E_FMINE_BIN="$(e2e_resolve_fmine_bin)" || return 1
    E2E_GIT_SHA="$(git -C "$E2E_REPO_ROOT" rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
    : >"$E2E_LOG_FILE"
}

# e2e_step <step_name> <want_exit> <want_status> [want_refusal|-] [extra harness.py flags...] -- <fmine args...>
e2e_step() {
    local step="$1"
    local want_exit="$2"
    local want_status="$3"
    local want_refusal="${4:--}"
    shift 4

    local extra=()
    while [ "$#" -gt 0 ]; do
        if [ "$1" = "--" ]; then
            shift
            break
        fi
        extra+=("$1")
        shift
    done

    local rc=0
    python3 -I "$E2E_HARNESS_PY" run-step \
        --run-dir "$E2E_RUN_DIR" \
        --suite "$E2E_SUITE" \
        --step "$step" \
        --bin "$E2E_FMINE_BIN" \
        --want-exit "$want_exit" \
        --want-status "$want_status" \
        --want-refusal "$want_refusal" \
        --git-sha "$E2E_GIT_SHA" \
        --digest-out "$E2E_DIGEST_TMP" \
        ${extra[@]+"${extra[@]}"} \
        -- "$@" || rc=$?

    if [ -f "$E2E_DIGEST_TMP" ]; then
        E2E_LAST_DIGEST="$(cat "$E2E_DIGEST_TMP")"
    fi
    if [ "$rc" -ne 0 ]; then
        E2E_FAILURES=$((E2E_FAILURES + 1))
    fi
    return "$rc"
}

# e2e_log_event <json_object>
e2e_log_event() {
    python3 -I "$E2E_HARNESS_PY" log-event \
        --run-dir "$E2E_RUN_DIR" \
        --suite "$E2E_SUITE" \
        --git-sha "$E2E_GIT_SHA" \
        --json "$1"
}

# e2e_replay_crashpack <crashpack_dir>
e2e_replay_crashpack() {
    python3 -I "$E2E_HARNESS_PY" replay "$1" --bin "$E2E_FMINE_BIN"
}

# e2e_summarize <events.ndjson> [out.md]
e2e_summarize() {
    if [ -n "${2:-}" ]; then
        python3 -I "$E2E_HARNESS_PY" summarize "$1" --out "$2"
    else
        python3 -I "$E2E_HARNESS_PY" summarize "$1"
    fi
}

# e2e_finish
e2e_finish() {
    python3 -I "$E2E_HARNESS_PY" finish --run-dir "$E2E_RUN_DIR"
}
