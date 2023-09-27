#!/bin/bash

SCRIPT_DIR=$(dirname "$0")

git submodule init
git submodule update

cd "$SCRIPT_DIR"
if [ -d "vendor/build" ]
then 
  echo "vendor/build already exists, skipping"
else
  echo "configuring minion"
  mkdir -p vendor/build
  cd vendor/build
  python3 ../configure.py
fi
