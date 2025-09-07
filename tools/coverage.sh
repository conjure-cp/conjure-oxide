#!/bin/bash

set -e

# coverage.sh
echo_err () {
  echo "$@" 1>&2
}

usage () { 
  echo_err 'coverage.sh'
  echo_err ''
  echo_err 'Generate code coverage reports for the repository.'
  echo_err 'This generates target/debug/coverage/lcov.info (for editors) and target/debug/coverage/index.html.'
}

if [ "$1" == "-h" ] || [ "$1" == "--help" ]; then
  usage
  exit 0
fi


if ! command -v rustup &> /dev/null 
then
  echo_err "rustup is not found!"
  exit 1
fi

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
cargo locate-project &>/dev/null || { echo_err "Cannot find a rust project"; usage; exit 1; }

PROJECT_ROOT=$(dirname $(cargo locate-project | jq -r .root 2> /dev/null))
TARGET_DIR=$(cargo metadata 2> /dev/null | jq -r .target_directory 2>/dev/null)

cd "$PROJECT_ROOT"

echo_err "info: installing llvm-tools-preview"
rustup component add llvm-tools-preview

echo_err "info: installing grcov"
cargo install grcov


# Uses rust's source code instrument-based coverage, processed by grcov.
# See: 
# https://doc.rust-lang.org/rustc/instrument-coverage.html
# https://github.com/mozilla/grcov

rm -rf target/debug/coverage

export CARGO_INCREMENTAL=0 
export RUSTFLAGS="$RUSTFLAGS -Cinstrument-coverage"
export RUSTDOCFLAGS="$RUSTDOCFLAGS -C instrument-coverage -Zunstable-options --persist-doctests target/debug/doctestbins"
export LLVM_PROFILE_FILE='conjure-oxide-%p-%m.profraw' 

# regex patterns to ignore
GRCOV_EXCLUDE_LINES=(
  'consider covered'
  'bug!'
  '#\[register_rule'
  'register_rule_set!'
)

# construct an or regex
GRCOV_EXCLUDE_FLAG="--excl-line=$(echo ${GRCOV_EXCLUDE_LINES[@]} | tr ' ' '|'})"

GRCOV_IGNORE_FLAGS=(
  '--ignore-not-existing'
  '--ignore'
  "${HOME}"'/.cargo/**/*.rs'
  '--ignore'
  'target/**/*.rs'
  '--ignore'
  '**/examples/*.rs'
  '--ignore'
  '**/build.rs'
  '--ignore'
  'tests-integration/tests/generated_tests.rs'
)



echo_err "info: building"
cargo +nightly build --workspace

echo_err "info: running tests"
cargo +nightly test --workspace

echo_err "info: generating coverage reports"
grcov . -s . --binary-path ./target/debug -t html\
  "${GRCOV_IGNORE_FLAGS[@]}" ${GRCOV_EXCLUDE_FLAG}\
  -o ./target/debug/coverage || { echo_err "fatal: html coverage generation failed" ; exit 1; }

echo_err "info: html coverage report generated to target/debug/coverage/index.html"

grcov . -s . --binary-path ./target/debug -t lcov\
  "${GRCOV_IGNORE_FLAGS[@]}" ${GRCOV_EXCLUDE_FLAG}\
  -o ./target/debug/lcov.info || { echo_err "fatal: lcov coverage generation failed" ; exit 1; }

echo_err "info: lcov coverage report generated to target/debug/lcov.info"

rm -rf **/*.profraw *.profraw

