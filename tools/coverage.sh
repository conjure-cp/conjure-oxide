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
  echo_err 'If diff-cover is available, also generates target/debug/coverage/diff-coverage.html for changed lines.'
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
TARGET_DIR=$(cargo metadata --format-version 1 | jq -r .target_directory)

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

export CARGO_INCREMENTAL=0 
export RUSTFLAGS="$RUSTFLAGS -Cinstrument-coverage"
export RUSTDOCFLAGS="$RUSTDOCFLAGS -C instrument-coverage -Zunstable-options --persist-doctests target/debug/doctestbins"
# According to https://doc.rust-lang.org/beta/rustc/instrument-coverage.html#running-the-instrumented-binary-to-generate-raw-coverage-profiling-data
# If give a path to the LLVM_PROFILE_FILE envvar, you can ensure that the passed directory
# is created automatically to place all profiling files there. 
# This was done to avoid the presence of 80+ files in the project's root directory.
export LLVM_PROFILE_FILE="${TARGET_DIR}/coverage/conjure-oxide-%p-%m.profraw"
mkdir -p "${TARGET_DIR}/coverage"

# regex patterns to ignore
GRCOV_EXCLUDE_LINES=(
  'consider covered'
  'bug!'
  '#\[derive'
  '#\[register_rule'
  'register_rule_set!'
)

# construct an or regex
GRCOV_EXCLUDE_FLAG="--excl-line=$(echo ${GRCOV_EXCLUDE_LINES[@]} | tr ' ' '|')"

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
grcov "${TARGET_DIR}/coverage" -s . --binary-path ./target/debug -t html\
  "${GRCOV_IGNORE_FLAGS[@]}" ${GRCOV_EXCLUDE_FLAG}\
  -o ./target/debug/coverage || { echo_err "fatal: html coverage generation failed" ; exit 1; }

echo_err "info: html coverage report generated to target/debug/coverage/index.html"

# Some grcov versions/layouts emit HTML under target/debug/coverage/html/.
# Normalise to a single root to avoid stale or duplicated report trees.
if [ -d ./target/debug/coverage/html ]; then
  echo_err "info: normalising grcov html output layout"
  rsync -a --delete ./target/debug/coverage/html/ ./target/debug/coverage/
  rm -rf ./target/debug/coverage/html
fi

grcov "${TARGET_DIR}/coverage" -s . --binary-path ./target/debug -t lcov\
  "${GRCOV_IGNORE_FLAGS[@]}" ${GRCOV_EXCLUDE_FLAG}\
  -o ./target/debug/lcov.info || { echo_err "fatal: lcov coverage generation failed" ; exit 1; }

echo_err "info: lcov coverage report generated to target/debug/lcov.info"

# diff-cover's LCOV parser is strict about FN/FNDA. Rust can emit commas in
# function names, which makes those lines look malformed to diff-cover.
# We keep the full LCOV for editors and filter FN/FNDA only for diff coverage.
DIFF_COVER_LCOV=./target/debug/diff-cover.lcov
awk '!/^(FN|FNDA):/' ./target/debug/lcov.info > "$DIFF_COVER_LCOV"

# Optional: diff-only coverage report for PR changes
if command -v python3 &> /dev/null; then
  if python3 -c "import diff_cover" &> /dev/null; then
    BASE_REF="${BASE_REF:-origin/main}"
    if git rev-parse --verify --quiet "$BASE_REF" > /dev/null; then
      echo_err "info: generating diff-only coverage report against ${BASE_REF}"
      DIFF_COVER_FAIL_UNDER="${DIFF_COVER_FAIL_UNDER:-0}"
      python3 -m diff_cover.diff_cover_tool \
        --compare-branch "$BASE_REF" \
        --fail-under "$DIFF_COVER_FAIL_UNDER" \
        --format "html:./target/debug/coverage/diff-coverage.html,json:./target/debug/coverage/diff-coverage.json" \
        "$DIFF_COVER_LCOV"
      echo_err "info: diff-only coverage report generated to target/debug/coverage/diff-coverage.html"
    else
      echo_err "info: base ref '${BASE_REF}' not found; skipping diff-only coverage"
      echo_err "info: ensure the base ref is fetched (e.g., git fetch origin main)"
    fi
  else
    echo_err "info: diff-cover not installed; skipping diff-only coverage"
    echo_err "info: install with: python3 -m pip install diff-cover"
  fi
else
  echo_err "info: python3 not found; skipping diff-only coverage"
fi

echo "info: coverage HTML report path: $(realpath ./target/debug/coverage/index.html)"

rm -rf "${TARGET_DIR}/coverage"/*.profraw
