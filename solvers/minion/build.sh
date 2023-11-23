#!/bin/bash

SCRIPT_DIR=$(dirname "$0")

git submodule init -- vendor
git submodule sync -- vendor 
git submodule update --init --recursive -- vendor

cd "$SCRIPT_DIR"

echo "------ CONFIGURE STEP ------"

if [ -d vendor/build ]; then
  echo "vendor/build already exists; skipping"
  echo "if you need to reconfigure minion (such as after an update), delete this directory!"
else
  mkdir -p vendor/build
  cd vendor/build
  python3 ../configure.py --lib --quick
fi

echo "------ BUILD STEP ------"
cd "$SCRIPT_DIR"
cd vendor/build
make
