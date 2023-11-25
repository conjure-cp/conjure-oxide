#!/bin/bash

# gen_coverage.sh
echo_err () {
  echo "$@" 1>&2
}

if ! command -v cargo &> /dev/null 
then
  echo_err "cargo is not found!"
  exit 1
fi

if ! command -v jq &> /dev/null 
then
  echo_err "jq is not found!"
  exit 1
fi

# Setup - enter rust project
cargo locate-project &>/dev/null || { echo_err "Cannot find a rust project"; usage_and_fail; }

PROJECT_ROOT=$(dirname $(cargo locate-project | jq -r .root 2> /dev/null))
TARGET_DIR=$(cargo metadata 2> /dev/null | jq -r .target_directory 2>/dev/null)

cd "$PROJECT_ROOT"
rm -rf "$TARGET_DIR/docs"

echo_err "=== DOCS FOR CONJURE OXIDE ==="
cd "conjure_oxide"
cargo doc --no-deps
cd -

echo_err "=== DOCS FOR MINION ==="
cd "solvers/minion"
cargo doc --no-deps
cd -

echo_err "=== DOCS FOR CHUFFED ==="
cd "solvers/chuffed"
cargo doc --no-deps
cd -
