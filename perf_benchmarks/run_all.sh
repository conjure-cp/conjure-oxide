#!/usr/bin/env bash
#
# ./run_all.sh
#
# DESCRIPTION: run all benchmarks on the current branch and main.
#
# Author: niklasdewally
# Date: 2025/06/18

conjure_oxide_dir=..
main_dir="build/conjure-oxide-main"
mkdir -p $main_dir

set -e

echo "======= COMPILING CONJURE_OXIDE (MAIN) =======" >/dev/stderr

# build binary for main
pushd $main_dir >/dev/null
{ git clone https://github.com/conjure-cp/conjure-oxide . --single-branch >/dev/null 2>/dev/null && git submodule update --init --remote >/dev/null 2>/dev/null; } || git pull >/dev/null 2>/dev/null
cargo build -q --release 
before_bin=$(realpath target/release/conjure_oxide)
popd >/dev/null

echo "======= COMPILING CONJURE_OXIDE (CURRENT) =======" >/dev/stderr

# build binary on current branch
pushd $conjure_oxide_dir >/dev/null
cargo build -q --release
after_bin=$(realpath target/release/conjure_oxide)
popd >/dev/null

models_fast="$(find models/fast -iname '*.eprime' | sort)"
models_slow="$(find models/slow -iname '*.eprime' | sort)"

for model in $models_fast; do
	echo "=======[ $model ]======="
	hyperfine --warmup 2 \
		--command-name main "$before_bin solve --no-run-solver $model" \
		--command-name current "$after_bin solve --no-run-solver $model"
	echo ""
done

for model in $models_slow; do
	echo "=======[ $model ]======="
	hyperfine --warmup 1 --runs 3 \
		--command-name main "$before_bin solve --no-run-solver $model" \
		--command-name current "$after_bin solve --no-run-solver $model"
	echo ""
done
