#!/usr/bin/env bash
#
# ./run_all.sh
#
# DESCRIPTION: run all benchmarks, comparing the current branch and main.
#
# ENVIRONMENT VARIABLES:
#
#   + BENCHMARK_GIT_REMOTE: git remote to compare to. if "local", will use the local conjure oxide repository.
#       (default: https://github.com/conjure-cp/conjure-oxide)
#
#   + BENCHMARK_GIT_REF: git reference to compare to.
#       (default: main)
#
#   + EXTRA_FLAGS: extra flags to pass to conjure-oxide
#
# Author: niklasdewally
# Date: 2025/06/18 (updated 2025/08/14)

set -e

cd "$(dirname "${0}")"

conjure_oxide_dir="$(realpath ..)"
baseline_dir="build/conjure-oxide-baseline"

git_remote="${BENCHMARK_GIT_REMOTE:-https://github.com/conjure-cp/conjure-oxide}"
git_ref="${BENCHMARK_GIT_REF:-main}"
extra_git_clone_flags=""

# when working with local branch, resolve relative references like HEAD^^
if [[ "${git_remote}" = "local" ]]; then
	git_ref=$(git rev-parse "${git_ref}")
	git_remote="${conjure_oxide_dir}"
	extra_git_clone_flags="-l"
fi

# is the remote we cloned before the same as this one?
if [[ -f "${baseline_dir}/.remote-url" && $(cat "${baseline_dir}/.remote-url") = "${git_remote}" ]]; then
	pushd "${baseline_dir}" >/dev/null 2>/dev/null
	git fetch "${git_remote}" "${git_ref}" 2>/dev/null
	git checkout FETCH_HEAD >/dev/null 2>/dev/null
	git submodule update --init --remote 2>/dev/null >/dev/null
else
	# different remote, or the folder doesnt exist
	rm -rf "${baseline_dir}"
	mkdir -p "${baseline_dir}"
	pushd "${baseline_dir}" >/dev/null 2>/dev/null
	git clone ${extra_git_clone_flags} "${git_remote}" . >/dev/null 2>/dev/null
	git fetch "${git_remote}" "${git_ref}" 2>/dev/null
	git checkout FETCH_HEAD >/dev/null 2>/dev/null
	git submodule update --init --remote 2>/dev/null >/dev/null
	echo "${git_remote}" >.remote-url
fi

echo "baseline repo: ${git_remote}" >/dev/stderr
echo "baseline ref: ${git_ref}" >/dev/stderr

popd >/dev/null 2>/dev/null

echo "======= COMPILING CONJURE_OXIDE (baseline) =======" >/dev/stderr
pushd "$baseline_dir" >/dev/null

# build binary for baseline
cargo build -q --release
baseline_bin=$(realpath target/release/conjure-oxide)
popd >/dev/null

echo "======= COMPILING CONJURE_OXIDE (CURRENT) =======" >/dev/stderr

# build binary on current branch
pushd "$conjure_oxide_dir" >/dev/null
cargo build -q --release
after_bin=$(realpath target/release/conjure-oxide)
popd >/dev/null

models_fast="$(find models/fast -iname '*.eprime' | sort)"
models_slow="$(find models/slow -iname '*.eprime' | sort)"

for model in $models_fast; do
	echo "=======[ $model ]======="
	hyperfine --warmup 2 \
		--command-name "baseline" "$baseline_bin solve ${EXTRA_FLAGS} --no-run-solver $model" \
		--command-name current "$after_bin solve ${EXTRA_FLAGS} --no-run-solver $model"
	echo ""
done

for model in $models_slow; do
	echo "=======[ $model ]======="
	hyperfine --warmup 1 --runs 5 \
		--command-name "baseline" "$baseline_bin solve ${EXTRA_FLAGS} --no-run-solver $model" \
		--command-name current "$after_bin solve ${EXTRA_FLAGS} --no-run-solver $model"
	echo ""
done
