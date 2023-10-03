#!/bin/bash

SCRIPT_DIR=$(dirname "$0")

git submodule init
git submodule update
cd "$SCRIPT_DIR"

echo "------ BUILDING ------"
cd vendor || exit
cmake -B build -S .
cmake --build build
