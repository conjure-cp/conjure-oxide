#!/usr/bin/env sh

# get_caching_paths.sh
#
# get the paths to be cached by Github Actions.
# paths are seperated by new lines

sh get_submodule_paths.sh
echo '~/.cargo/bin'
echo '~/.cargo/registry/index'
echo '~/.cargo/registry/cache'
echo '~/.cargo/git/db'
echo 'target/'
