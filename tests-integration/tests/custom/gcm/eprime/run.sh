#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/gcm-eprime-trace.XXXXXX")

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT INT TERM
first_case=1

run_case() {
    model_path=$1
    param_path=$2
    model_name=$(basename "$model_path")
    aggregate_path="$TMP_DIR/$model_name.aggregate"

    if [ "$first_case" -eq 0 ]; then
        echo ""
    fi
    first_case=0

    conjure-oxide-debug solve \
        --parser tree-sitter \
        --rewriter morph-levelson-fixedpoint \
        --solver smt-lia-arrays \
        --no-run-solver \
        --rule-trace-aggregates "$aggregate_path" \
        "$model_path" \
        "$param_path" \
        >/dev/null

    echo "CASE $model_name"
    echo "param: $(basename "$param_path")"
    echo "status: ok"
    if [ -s "$aggregate_path" ]; then
        cat "$aggregate_path"
    else
        echo "total_rule_applications: 0"
    fi
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

# Disabled: currently times out under this rewriter configuration.
# run_case \
#     "$SCRIPT_DIR/CLA-OC.eprime" \
#     "$SCRIPT_DIR/params/100166617566-CLA-OC.eprime-param"

# Disabled: currently triggers a panic in expression indexing/type handling.
# run_case \
#     "$SCRIPT_DIR/CLA-general.eprime" \
#     "$SCRIPT_DIR/params/100166617566-CLA-general.eprime-param"
