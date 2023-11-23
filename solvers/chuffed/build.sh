#!/bin/bash

SCRIPT_DIR=$(realpath $(dirname "$0"))

git submodule init
git submodule update
cd "$SCRIPT_DIR" || exit 1

if ! [[ -v OUT_DIR ]]; then
  echo "OUT_DIR env variable does not exist - did you run this script through cargo build?"
  exit 1
fi

echo "------ BUILDING ------"
cd vendor || exit 1
cmake -B build -S .
cmake --build build
cd ..

# Build wrapper.cpp as static library
cd "$OUT_DIR" || exit 1
c++ -c "$SCRIPT_DIR/wrapper.cpp" -I"$SCRIPT_DIR/vendor" --std=c++11
ar rvs libwrapper.a wrapper.o
