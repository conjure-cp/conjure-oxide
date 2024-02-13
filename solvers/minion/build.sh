#!/bin/bash

set -x

{
  SCRIPT_DIR="$(readlink -f "$(dirname "$0")")"
  cd "$SCRIPT_DIR"


  cd "$SCRIPT_DIR"

  git submodule init -- vendor || exit 1
  git submodule sync -- vendor || exit 1
  git submodule update --init --recursive --remote -- vendor || exit 1

  if [[ -z "$OUT_DIR" ]]; then
    echo "OUT_DIR env variable does not exist - did you run this script through cargo build?"
    exit 1
  fi

  echo "------ CONFIGURE STEP ------"

  mkdir -p "$OUT_DIR/build"
  cd "$OUT_DIR/build"
  python3 "$SCRIPT_DIR/vendor/configure.py" --lib --quick

  echo "------ BUILD STEP ------"
  cd "$OUT_DIR/build"
  make
} 2>&1
