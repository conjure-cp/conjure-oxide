#!/bin/bash

set -e
set -x

SCRIPT_DIR="$(readlink -f "$(dirname "$0")")"
cd "$SCRIPT_DIR"

REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"

# This crate depends on the `vendor/` submodule.
# Don't mutate git state during a Cargo build (i.e. do not run `git submodule update`)
# as build scripts may run concurrently).
# This causes builds to fail intermitently.
# Instead, fail with a clear instruction if the submodule hasn't been initialised.
if [ ! -f "$SCRIPT_DIR/vendor/configure.py" ]; then
  echo "minion-sys: missing submodule contents at crates/minion-sys/vendor/" >&2
  echo "Run: git -C \"$REPO_ROOT\" submodule update --init --recursive -- crates/minion-sys/vendor" >&2
  exit 1
fi

if [[ -z "$OUT_DIR" ]]; then
  echo "OUT_DIR env variable does not exist - did you run this script through cargo build?"
  exit 1
fi

echo "------ CONFIGURE STEP ------"

mkdir -p "$OUT_DIR/build"
cd "$OUT_DIR/build"

if [[ ${DEBUG_MINION-default} != "default" ]]; then
  python3 "$SCRIPT_DIR/vendor/configure.py" --lib --quick --debug
else
  python3 "$SCRIPT_DIR/vendor/configure.py" --lib --quick
fi

echo "------ BUILD STEP ------"
cd "$OUT_DIR/build"
make
