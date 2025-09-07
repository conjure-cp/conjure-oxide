#!/usr/bin/env bash


# ./test-summary.sh
#
# DESCRIPTION: print the status of each conjure oxide integration test.
#   Statuses are pass, fail, and disabled.
#
#   Unlike cargo test, this script reports the number of disabled tests.
#
# USAGE: 
#   -F: only show failing tests
#
# Author: niklasdewally
# Date: 2024/10/02
# SPDX-Licence-Identifier: MPL-2.0

failonly=false

while getopts "F" flag; do
  case ${flag} in
      F) failonly=true
        ;;
  esac
done

cargo locate-project &>/dev/null || { echo_err "Cannot find a rust project"; usage; exit 1; }

PROJECT_ROOT=$(dirname $(cargo locate-project | jq -r .root 2> /dev/null))
cd "$PROJECT_ROOT/tests-integration/"

TMP=$(mktemp)
cargo +nightly test --test generated_tests -- --format=json -Z unstable-options 2>/dev/null | jq -s '.[] | select(.type | contains("test"))?' > "$TMP"

FAILED_TESTS=$(jq -sr '.[] | select(.event | contains("failed"))? | .name' < "$TMP")
PASSED_TESTS=$(jq -sr '.[] | select(.event | contains("ok"))? | .name' < "$TMP")


cd tests/integration
DISABLED_ESSENCE_TESTS=$(find -iname '*.essence.disabled' -exec dirname "{}" \; | sed 's/^\.\///' | sed 's/\//_/g' | sed 's/-/_/g' )
DISABLED_EPRIME_TESTS=$(find -iname '*.eprime.disabled' -exec dirname "{}" \; | sed 's/^\.\///' | sed 's/\//_/g' | sed 's/-/_/g' )

cd ../..

clear

# put a dummy field at the beginning without colour then remove it, as sort doesnt like ansi codes
{
  for test in $FAILED_TESTS; do 
    test_p=$(sed 's/tests_integration_\(.*\)/\1/' <<< $test)
    echo -e "$test_p , \033[0;31m$test_p, fail\033[0m\n"
  done

  if  [ $failonly = false ]; then

    for test in $PASSED_TESTS; do
      test_p=$(sed 's/tests_integration_\(.*\)/\1/' <<< $test)
      echo -e "$test_p , \033[0;32m$test_p, pass\033[0m\n"
    done

    for test in $DISABLED_ESSENCE_TESTS; do
      echo -e "$test, \033[0;33m$test , disabled \033[0m\n"
    done
    for test in $DISABLED_EPRIME_TESTS; do
      echo -e "$test, \033[0;33m$test , disabled \033[0m\n"
    done
  fi

} | sort -k1 -t, | cut -d, -f 2,3 | column -t -s,

# cast all wc outputs to numbers, otherwise spacing is wierd on macos
# also , wc -l returns one for empty input, so we need to -1
echo ""
echo "Passed: $(($(wc -l <<< $PASSED_TESTS) -1))"
echo "Failed: $(($(wc -l <<< $FAILED_TESTS) -1))"
echo "Disabled: $(($(wc -l <<< $DISABLED_ESSENCE_TESTS ) + $(wc -l <<< $DISABLED_EPRIME_TESTS) -1))"
