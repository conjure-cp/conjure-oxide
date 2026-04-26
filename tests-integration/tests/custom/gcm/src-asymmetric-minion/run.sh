#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
MODEL_DIR="$SCRIPT_DIR/../eprime"

trace_path="${SCRIPT_DIR}/trace.txt"
stderr_path="${SCRIPT_DIR}/stderr.txt"
stdout_path="${SCRIPT_DIR}/stdout.txt"

conjure-oxide solve \
    --parser tree-sitter \
    --rewriter morph-levelson-fixedpoint \
    --comprehension-expander=native \
    --number-of-solutions=10000 \
    --solver minion \
    --rule-trace "$trace_path" \
    --minion-valorder random \
    "$MODEL_DIR/SRC-asymmetric.eprime" \
    "$MODEL_DIR/params/100166617566-SRC-asymmetric.eprime-param" \
    >"$stdout_path" \
    2>"$stderr_path"

grep -F "Dominance pruning retained" "$stderr_path"

rule_applications=$(grep -c '~~>' "$trace_path" || true)
echo "rule applications: $rule_applications"
# test "$rule_applications" -eq 1327
