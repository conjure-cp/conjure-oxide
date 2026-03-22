#!/usr/bin/env bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WT2_DIR="/home/mayday/Coding/conjure/conjure-oxide-wt2"

PROFILE="profiling"
BIN="target/$PROFILE/conjure-oxide"
TEST_DIR="tests-integration/tests/integration/basic/matrix"

echo "Building main..."
cd "$WT2_DIR" || exit
#git switch main
#git pull
cargo build --profile $PROFILE

echo "Building current..."
cd "$SCRIPT_DIR" || exit
cargo build --profile $PROFILE


BIN1="$SCRIPT_DIR/$BIN"
BIN2="$WT2_DIR/$BIN"

echo "Comparing speeds..."

for test_case in "$SCRIPT_DIR/$TEST_DIR"/*; do
    if [ -d "$test_case" ]; then
        dir_name=$(basename "$test_case")
        wt2_test_case="$WT2_DIR/$TEST_DIR/$dir_name"

        if [ -d "$wt2_test_case" ]; then
            # Find the input file (.essence or .eprime) in SCRIPT_DIR
            file1=$(find "$test_case" -maxdepth 1 -name "*.essence" -o -name "*.eprime" | head -n 1)

            # Find the input file in WT2_DIR
            file2=$(find "$wt2_test_case" -maxdepth 1 -name "*.essence" -o -name "*.eprime" | head -n 1)

            if [ -n "$file1" ] && [ -n "$file2" ]; then
                echo "---------------------------------------------------"
                echo "Test: $dir_name"
                hyperfine --warmup 5 --min-runs 10 \
                    -n "current" "$BIN1 solve \"$file1\"" \
                    -n "main"    "$BIN2 solve \"$file2\""
            fi
        fi
    fi
done
