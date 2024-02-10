#!/bin/bash

# gen_coverage.sh
echo_err () {
  echo "$@" 1>&2
}

usage () { 
  echo_err 'gen_coverage.sh'
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

# Install required tools
echo_err "info: installing nightly rust"
rustup install nightly

echo_err "info: installing llvm-tools-preview"
rustup component add llvm-tools-preview

echo_err "info: installing grcov"
rustup run nightly cargo install grcov


# Uses rust's source code instrument-based coverage, processed by grcov.
# See: 
# https://doc.rust-lang.org/rustc/instrument-coverage.html
# https://github.com/mozilla/grcov

rm -rf target/debug/coverage

export CARGO_INCREMENTAL=0 
export RUSTFLAGS='-Z unstable-options -Cinstrument-coverage' 
export RUSTDOCFLAGS="-C instrument-coverage -Z unstable-options --persist-doctests target/debug/doctestbins"
export LLVM_PROFILE_FILE='conjure-oxide-%p-%m.profraw' 

echo_err "info: building with nightly"
cargo +nightly build --workspace

echo_err "info: running tests"
cargo +nightly test --workspace

echo_err "info: generating coverage reports"
grcov . -s . --binary-path ./target/debug -t html --ignore-not-existing --ignore "$HOME"'/.cargo/**/*.rs' --ignore 'target/**/*.rs' --ignore '**/main.rs' --ignore '**/build.rs' -o ./target/debug/coverage
grcov . -s . --binary-path ./target/debug -t lcov --ignore-not-existing --ignore "$HOME"'/.cargo/**/*.rs' --ignore 'target/**/*.rs' --ignore '.cargo/**/*.rs' --ignore '**/main.rs' --ignore '**/build.rs' -o ./target/debug/lcov.info
rm -rf **/*.profraw *.profraw
