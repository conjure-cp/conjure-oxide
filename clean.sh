#!/bin/bash
# vim: cc=80
cd $(dirname "$0")

# update, initialise, and sync all submodules to ensure that changes are 
# reflected here. `git submodule sync` means change where the submodule points 
# to match .gitmodules, should it have changed (in particular, from a dev fork
# to upstream).

echo "=== Updating and resetting submodules ===" 1>&2

git submodule sync --recursive
git submodule update --init --recursive

echo "=== Cleaning submodule build files ===" 1>&2

bash ./etc/scripts/get_submodule_paths.sh | 
  {
  while read -r submodule_path; do
    cd "$submodule_path"
    git clean -dfx .
    cd - &> /dev/null
  done 
}

cd $(dirname "$0")

echo "=== Cargo clean ===" 1>&2
echo "" 1>&2

# clean our stuff
cargo clean

