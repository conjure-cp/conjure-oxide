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

if ! rustup component list --installed | grep -Eq 'llvm-tools-preview|llvm-tools'; then
  rustup component add llvm-tools-preview
fi

if ! command -v grcov &> /dev/null; then
  echo_err "info: installing grcov"
  cargo install grcov
fi


# Uses rust's source code instrument-based coverage, processed by grcov.
# See: 
# https://doc.rust-lang.org/rustc/instrument-coverage.html
# https://github.com/mozilla/grcov

rm -rf target/debug/coverage

export CARGO_INCREMENTAL=0 
export RUSTFLAGS="$RUSTFLAGS -Cinstrument-coverage"
export RUSTDOCFLAGS="$RUSTDOCFLAGS -C instrument-coverage -Zunstable-options --persist-doctests target/debug/doctestbins"
# According to https://doc.rust-lang.org/beta/rustc/instrument-coverage.html#running-the-instrumented-binary-to-generate-raw-coverage-profiling-data
# If give a path to the LLVM_PROFILE_FILE envvar, you can ensure that the passed directory
# is created automatically to place all profiling files there. 
# This was done to avoid the presence of 80+ files in the project's root directory.
export LLVM_PROFILE_FILE='target/coverage/conjure-oxide-%p-%m.profraw'
mkdir -p target/coverage

# regex patterns to ignore
GRCOV_EXCLUDE_LINES=(
  'consider covered'
  'bug!'
  '#\[derive'
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

echo "info: coverage HTML report path: $(realpath ./target/debug/coverage/index.html)"

rm -rf ./target/coverage/*.profraw

