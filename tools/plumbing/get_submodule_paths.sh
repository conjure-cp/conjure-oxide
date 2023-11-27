#!/usr/bin/env sh

# get_submodule_paths
# 
# returns the paths of each submodule in this repository, seperated by a new line
# these paths are relative to the git repository.

# go to top level of git repo
cd $(git rev-parse --show-superproject-working-tree --show-toplevel | head -n 1)

# https://stackoverflow.com/questions/12641469/list-submodules-in-a-git-repository
git config --file .gitmodules --get-regexp path | awk '{print $2}'

