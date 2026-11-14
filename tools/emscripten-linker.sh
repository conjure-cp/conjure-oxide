#!/usr/bin/env bash
set -euo pipefail
# Rules are discovered through inventory constructors, not ordinary symbol references.
# Retain every object in this archive, while allowing normal linking for other crates.
args=()
for arg in "$@"; do
    case "$arg" in
        */libconjure_cp_rules-*.rlib)
            args+=(-Wl,--whole-archive "$arg" -Wl,--no-whole-archive)
            ;;
        *) args+=("$arg") ;;
    esac
done
exec emcc "${args[@]}"
