#!/usr/bin/env bash
#
# ./watch-tests.sh.
#
# DESCRIPTION: give a live-updating summary of integration test passes and failures.
#
# USAGE: 
#   Environment variables such as ACCEPT and ALLTESTS are passed directly
#   through to the tester.
#
#   This tool requires watchman.
#
# Author: niklasdewally
# Date: 2024/10/31

[ $(command -v watchman) ] || { >&2 echo "fatal: watchman not found!"; exit 1; }

cargo locate-project &>/dev/null || { echo_err "Cannot find a rust project"; usage; exit 1; }
PROJECT_ROOT=$(dirname $(cargo locate-project | jq -r .root 2> /dev/null))
cd "$PROJECT_ROOT"

./tools/test-summary.sh -F
watchman-make -p '**/*.rs' '**/*.essence' '**/*.eprime' --run "./tools/test-summary.sh -F"
