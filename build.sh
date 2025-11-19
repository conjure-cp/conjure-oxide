#!/bin/bash

set -e
set -x

SCRIPT_DIR="$(readlink -f "$(dirname "$0")")"
cd "$SCRIPT_DIR"

git submodule update --init --recursive
