#!/bin/bash
# vim: cc=80

# root of project
cd $(dirname "$0")/..

# update, initialise, and sync all submodules to ensure that changes are 
# reflected here. `git submodule sync` means change where the submodule points 
# to match .gitmodules, should it have changed (in particular, from a dev fork
# to upstream).

echo "=== Cleaning submodules ===" 1>&2

bash ./tools/plumbing/get_submodule_paths.sh | 
  {
  while read -r submodule_path; do
    cd "$submodule_path"
    git clean -dfx .
    cd - &> /dev/null
  done 
}

echo "=== Updating and resetting submodules ===" 1>&2

git submodule init --recursive
git submodule sync --recursive
git submodule update --init --recursive

