#!/bin/bash

SCRIPT_DIR=$(dirname "$0")

git submodule init -- vendor
git submodule sync -- vendor 
git submodule update --init --recursive -- vendor

cd "$SCRIPT_DIR"

echo "------ CONFIGURE STEP ------"

mkdir -p vendor/build
cd vendor/build
python3 ../configure.py --lib --quick

echo "------ BUILD STEP ------"
cd "$SCRIPT_DIR"
cd vendor/build
make
