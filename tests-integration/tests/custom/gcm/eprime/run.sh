#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/gcm-eprime-trace.XXXXXX")
<<<<<<< HEAD
=======
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
>>>>>>> 397bf5d8 (feat: various fixes to (partially) support the GCM models (#1750))

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT INT TERM
<<<<<<< HEAD
first_case=1
=======
>>>>>>> 397bf5d8 (feat: various fixes to (partially) support the GCM models (#1750))

run_case() {
    model_path=$1
    param_path=$2
    model_name=$(basename "$model_path")
    aggregate_path="$TMP_DIR/$model_name.aggregate"
<<<<<<< HEAD

    if [ "$first_case" -eq 0 ]; then
        echo ""
    fi
    first_case=0

    conjure-oxide-debug solve \
        --parser tree-sitter \
        --rewriter morph-levelson-fixedpoint \
=======
    stderr_path="$TMP_DIR/$model_name.stderr"

    set +e
    "$TIMEOUT_BIN" 60 \
        conjure-oxide-debug solve \
        --parser tree-sitter \
<<<<<<< HEAD
        --rewriter morph-levelson-nocache-prefilteroff-fixedpoint \
>>>>>>> 397bf5d8 (feat: various fixes to (partially) support the GCM models (#1750))
=======
        --rewriter morph-levelson-fixedpoint \
>>>>>>> 3fe72a10 (feat(morph): enable morph for as many integration tests as possible, fix issues for the sat backend (#1754))
        --solver smt-lia-arrays \
        --no-run-solver \
        --rule-trace-aggregates "$aggregate_path" \
        "$model_path" \
        "$param_path" \
<<<<<<< HEAD
        >/dev/null

    echo "CASE $model_name"
    echo "param: $(basename "$param_path")"
    echo "status: ok"
    if [ -s "$aggregate_path" ]; then
        cat "$aggregate_path"
    else
        echo "total_rule_applications: 0"
    fi
=======
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
        tail -n 20 "$stderr_path" \
            | grep -v '^version: ' \
            | sed -E "s/(thread 'main') \\([0-9]+\\)( panicked at )/\\1\\2/"
    fi

    echo ""
>>>>>>> 397bf5d8 (feat: various fixes to (partially) support the GCM models (#1750))
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
<<<<<<< HEAD

# Disabled: currently times out under this rewriter configuration.
# run_case \
#     "$SCRIPT_DIR/CLA-OC.eprime" \
#     "$SCRIPT_DIR/params/100166617566-CLA-OC.eprime-param"

# Disabled: currently triggers a panic in expression indexing/type handling.
# run_case \
#     "$SCRIPT_DIR/CLA-general.eprime" \
#     "$SCRIPT_DIR/params/100166617566-CLA-general.eprime-param"
=======
run_case \
    "$SCRIPT_DIR/CLA-OC.eprime" \
    "$SCRIPT_DIR/params/100166617566-CLA-OC.eprime-param"
run_case \
    "$SCRIPT_DIR/CLA-general.eprime" \
    "$SCRIPT_DIR/params/100166617566-CLA-general.eprime-param"
>>>>>>> 397bf5d8 (feat: various fixes to (partially) support the GCM models (#1750))
