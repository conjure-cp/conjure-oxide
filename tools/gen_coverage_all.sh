# !/bin/bash
# generate coverage for all crates in the workspace

PATH_TO_GEN_COV="tools/gen_coverage.sh"
echo_err () {
  echo "$@" 1>&2
}

if ! command -v cargo &> /dev/null 
then
  echo_err "cargo is not found!"
  exit 1
fi

if ! command -v jq &> /dev/null 
then
  echo_err "jq is not found!"
  exit 1
fi

# Setup - enter rust workspace
cargo locate-project --workspace &>/dev/null || { echo_err "Cannot find a rust workspace"; usage_and_fail; }
WORKSPACE_ROOT=$(dirname $(cargo locate-project --workspace | jq -r .root 2> /dev/null))


# conjure-oxide coverage
echo_err "GENERATING COVERAGE FOR CONJURE-OXIDE"
cd "$WORKSPACE_ROOT/conjure_oxide"
rm -rf coverage
bash "$WORKSPACE_ROOT/$PATH_TO_GEN_COV"

# solver coverage
for dir in "$WORKSPACE_ROOT/solvers"/*/
do 
  echo_err "GENERATING COVERAGE FOR $dir"
  cd "$dir"
  rm -rf coverage
  bash "$WORKSPACE_ROOT/$PATH_TO_GEN_COV"
done


