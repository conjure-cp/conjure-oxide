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
rm -rf "$TARGET_DIR/doc"

RUSTDOCFLAGS="-Zunstable-options --markdown-no-toc --index-page ${PROJECT_ROOT}/doc/index.md" cargo +nightly doc --no-deps --all-features -p conjure_oxide -p conjure_rule_macros -p conjure_core -p minion_rs -p enum_compatability_macro -p conjure_essence_parser -p conjure_essence_macros $@
