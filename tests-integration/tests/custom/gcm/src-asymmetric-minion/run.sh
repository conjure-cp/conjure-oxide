#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
MODEL_DIR="$SCRIPT_DIR/../eprime"
TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/src-asymmetric-minion.XXXXXX")

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT INT TERM

trace_path="$TMP_DIR/src-asymmetric.trace"
stderr_path="$TMP_DIR/src-asymmetric.stderr"
stdout_path="$TMP_DIR/src-asymmetric.stdout"

conjure-oxide solve \
    --parser tree-sitter \
    --rewriter morph-levelson-fixedpoint \
    --comprehension-expander=native \
    --number-of-solutions=5 \
    --solver minion \
    --rule-trace "$trace_path" \
    "$MODEL_DIR/SRC-asymmetric.eprime" \
    "$MODEL_DIR/params/100166617566-SRC-asymmetric.eprime-param" \
    >"$stdout_path" \
    2>"$stderr_path"

grep -F "Dominance pruning retained 5 of 5 solutions." "$stderr_path"

rule_applications=$(grep -c '~~>' "$trace_path" || true)
echo "rule applications: $rule_applications"
test "$rule_applications" -eq 1327
