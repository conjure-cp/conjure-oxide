#!/bin/bash

# gen_coverage.sh
echo_err () {
  echo "$@" 1>&2
}

usage_and_fail () { 
  echo_err 'gen_coverage.sh'
  echo_err ''
  echo_err 'Generate coverage reports for the current rust project.'
  echo_err 'These will be output into <project_root>/coverage'
  exit 1
}

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
cargo locate-project &>/dev/null || { echo_err "Cannot find a rust project"; usage_and_fail; }

PROJECT_ROOT=$(dirname $(cargo locate-project | jq -r .root 2> /dev/null))
TARGET_DIR=$(cargo metadata 2> /dev/null | jq -r .target_directory 2>/dev/null)

cd "$PROJECT_ROOT"

rm -rf coverage
mkdir -p coverage || { echo_err "Cannot create coverage directory"; exit 1; }



# Install required tools
echo_err "info: installing nightly rust:"
rustup install nightly

echo_err "info: installing llvm-tools-preview:"
rustup component add llvm-tools-preview

echo_err "info: installing grcov:"
rustup run nightly cargo install grcov


# Run tests, profile, generate coverage
# See https://blog.rng0.io/how-to-do-code-coverage-in-rust
# and https://doc.rust-lang.org/beta/unstable-book/language-features/profiler-runtime.html
echo_err "info: running tests with nightly profiler:"
CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE='coverage/cargo-test-%p-%m.profraw' rustup run nightly cargo test

cd coverage
echo_err "info: generating coverage reports:"
grcov . -s .. --binary-path "$TARGET_DIR/debug/deps" -t html --branch --ignore-not-existing --keep-only src/** --ignore **/main.rs -o html
grcov . -s .. --binary-path "$TARGET_DIR/debug/deps" -t lcov --branch --ignore-not-existing --keep-only src/** --ignore **/main.rs -o lcov.info

