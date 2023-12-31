#!/bin/bash

SCRIPT_DIR="$(readlink -f "$(dirname "$0")")"
cd "$SCRIPT_DIR"

git submodule init -- vendor
git submodule sync -- vendor
git submodule update --init --recursive -- vendor


if [[ -z "$OUT_DIR" ]]; then
  echo "OUT_DIR env variable does not exist - did you run this script through cargo build?"
  exit 1
fi

echo "------ BUILDING ------"

mkdir -p "$OUT_DIR/build"
cd "$OUT_DIR/build"
cmake -B . -S "$SCRIPT_DIR/vendor"
cmake --build .

# Build wrapper.cpp as static library
cd "$OUT_DIR" || exit 1
c++ -c "$SCRIPT_DIR/wrapper.cpp" -I"$SCRIPT_DIR/vendor" --std=c++11
ar rvs libwrapper.a wrapper.o
