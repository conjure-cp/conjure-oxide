#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/gcm-eprime-trace.XXXXXX")
TIMEOUT_BIN="${TIMEOUT_BIN:-}"

if [ -z "$TIMEOUT_BIN" ]; then
    if command -v timeout >/dev/null 2>&1; then
        TIMEOUT_BIN=$(command -v timeout)
    elif command -v gtimeout >/dev/null 2>&1; then
        TIMEOUT_BIN=$(command -v gtimeout)
    else
        echo "missing timeout command: install timeout or gtimeout, or set TIMEOUT_BIN" >&2
        exit 1
    fi
fi

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT INT TERM

run_case() {
    model_path=$1
    param_path=$2
    model_name=$(basename "$model_path")
    aggregate_path="$TMP_DIR/$model_name.aggregate"
    stderr_path="$TMP_DIR/$model_name.stderr"

    set +e
    "$TIMEOUT_BIN" 60 \
        conjure-oxide-debug solve \
        --parser tree-sitter \
        --rewriter morph-levelson-nocache-prefilteroff-fixedpoint \
        --solver smt-lia-arrays \
        --no-run-solver \
        --rule-trace-aggregates "$aggregate_path" \
        "$model_path" \
        "$param_path" \
        >/dev/null 2>"$stderr_path"
    rc=$?
    set -e

    case "$rc" in
        0)
            status="ok"
            ;;
        124)
            status="timeout"
            ;;
        *)
            status="error($rc)"
            ;;
    esac

    echo "CASE $model_name"
    echo "param: $(basename "$param_path")"
    echo "status: $status"
    if [ "$status" = "ok" ]; then
        if [ -s "$aggregate_path" ]; then
            cat "$aggregate_path"
        else
            echo "total_rule_applications: 0"
        fi
    fi

    if [ "$rc" -ne 0 ] && [ -s "$stderr_path" ]; then
        echo "stderr_tail:"
        tail -n 20 "$stderr_path"
    fi

    echo ""
}

run_case \
    "$SCRIPT_DIR/RC.eprime" \
    "$SCRIPT_DIR/params/100166617566-RC.eprime-param"
run_case \
    "$SCRIPT_DIR/SRC-asymmetric.eprime" \
    "$SCRIPT_DIR/params/100166617566-SRC-asymmetric.eprime-param"
run_case \
    "$SCRIPT_DIR/SRC-spo.eprime" \
    "$SCRIPT_DIR/params/100166617566-SRC-spo.eprime-param"
run_case \
    "$SCRIPT_DIR/SRC-acyclic.eprime" \
    "$SCRIPT_DIR/params/100166617566-SRC-acyclic.eprime-param"
run_case \
    "$SCRIPT_DIR/SRC-multivariate.eprime" \
    "$SCRIPT_DIR/params/100166617566-SRC-multivariate.eprime-param"
run_case \
    "$SCRIPT_DIR/CLA-OC.eprime" \
    "$SCRIPT_DIR/params/100166617566-CLA-OC.eprime-param"
run_case \
    "$SCRIPT_DIR/CLA-general.eprime" \
    "$SCRIPT_DIR/params/100166617566-CLA-general.eprime-param"
