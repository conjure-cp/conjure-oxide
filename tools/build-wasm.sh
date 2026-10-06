#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
output="${1:-$root/target/wasm-package}"
target_dir="$(cargo metadata --format-version 1 --no-deps --locked | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
if [[ -z "${EMSDK_PYTHON:-}" && -x /opt/homebrew/opt/python@3.14/bin/python3.14 ]]; then
    export EMSDK_PYTHON=/opt/homebrew/opt/python@3.14/bin/python3.14
fi
cargo rustc -p conjure-cp-cli --bin conjure-oxide --locked --release \
    --no-default-features --features wasm-target --target wasm32-unknown-emscripten \
    -- -C debuginfo=0 -C "linker=$root/tools/emscripten-linker.sh" \
    -C strip=debuginfo -C link-arg=-g0 \
    -C link-arg=-sDEFAULT_TO_CXX=1 \
    -C link-arg=-sMODULARIZE=1 -C link-arg=-sEXPORT_ES6=1 \
    -C link-arg=-sEXPORTED_RUNTIME_METHODS=FS,callMain \
    -C link-arg=-sALLOW_MEMORY_GROWTH=1 -C link-arg=-sSTACK_SIZE=8388608 \
    -C link-arg=-sENVIRONMENT=web,worker
mkdir -p "$output"
cp "$target_dir/wasm32-unknown-emscripten/release/conjure-oxide.js" "$output/conjure.mjs"
cp "$target_dir/wasm32-unknown-emscripten/release/conjure_oxide.wasm" "$output/"
cp tools/wasm/*.mjs tools/wasm/*.mts tools/wasm/*.html "$output/"
cp tools/wasm/package.json LICENSE "$output/"
printf 'Wasm package built in %s\n' "$output"
