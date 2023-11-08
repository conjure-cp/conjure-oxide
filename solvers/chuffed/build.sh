#!/bin/bash

SCRIPT_DIR=$(dirname "$0")

git submodule init
git submodule update
cd "$SCRIPT_DIR" || exit

echo "------ BUILDING ------"
cd vendor || exit
cmake -B build -S .
cmake --build build
cd ..

# Build wrapper.cpp as static library
c++ -c wrapper.cpp -Ivendor --std=c++11
ar rvs libwrapper.a wrapper.o
