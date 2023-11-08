#!/usr/bin/env sh

# get_submodules_hash
# 
# compute a hash that represents the current state of all git submodule 
# dependencies. Primarily used for cache invalidation in GitHub Actions.


CI_SCRIPTS_DIR=$(realpath $(dirname "$0"))

# go to top level of git repo
cd $(git rev-parse --show-toplevel)

git submodule update --init --recursive 1>&2 2>/dev/null

{ 
  for module in $(sh "$CI_SCRIPTS_DIR/get_submodule_paths.sh")
  do
    git rev-parse "HEAD:$module" >> $SHAS
  done;
} 2>/dev/null | sha256sum | head -c 40 # note: sha256sum print a - at the end - this is removed here.
