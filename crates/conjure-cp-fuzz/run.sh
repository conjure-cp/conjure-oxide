#!/usr/bin/env bash
set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────
CORES="${FUZZ_CORES:-8}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

SEEDS_DIR="$SCRIPT_DIR/seeds"
CORPUS_DIR="$SCRIPT_DIR/corpus"
DICT="$SCRIPT_DIR/essence.dict"
HARNESS="$ROOT_DIR/target/release/conjure-fuzz-harness"

# ── Build ────────────────────────────────────────────────────────────
echo "[*] Building harness (release)..."
cargo afl build -p conjure-cp-fuzz --release

# ── Validate seeds dir ───────────────────────────────────────────────
if [ ! -d "$SEEDS_DIR" ] || [ -z "$(find "$SEEDS_DIR" -maxdepth 1 -type f 2>/dev/null)" ]; then
    echo "[!] No seed files found in $SEEDS_DIR"
    echo "    Add .essence files there before running."
    exit 1
fi

# ── Seed corpus minimization (afl-cmin) ──────────────────────────────
SEEDS_MIN_DIR="$SCRIPT_DIR/seeds_minimized"
echo "[*] Minimizing seed corpus into $SEEDS_MIN_DIR ..."
rm -rf "$SEEDS_MIN_DIR"
mkdir -p "$SEEDS_MIN_DIR"
cargo afl cmin -i "$SEEDS_DIR" -o "$SEEDS_MIN_DIR" -- "$HARNESS"
echo "[*] Minimized: $(find "$SEEDS_MIN_DIR" -maxdepth 1 -type f | wc -l) seeds (from $(find "$SEEDS_DIR" -maxdepth 1 -type f | wc -l))"

# ── Launch AFL instances ─────────────────────────────────────────────
mkdir -p "$CORPUS_DIR"

echo "[*] Starting $CORES AFL instances..."

# Main (deterministic) instance
cargo afl fuzz \
    -i "$SEEDS_MIN_DIR" \
    -o "$CORPUS_DIR" \
    -x "$DICT" \
    -M main \
    -- "$HARNESS" &
AFL_PIDS=($!)

# Secondary instances
for i in $(seq 1 $((CORES - 1))); do
    cargo afl fuzz \
        -i "$SEEDS_MIN_DIR" \
        -o "$CORPUS_DIR" \
        -x "$DICT" \
        -S "secondary_$i" \
        -- "$HARNESS" &
    AFL_PIDS+=($!)
done

echo "[*] AFL PIDs: ${AFL_PIDS[*]}"
echo "[*] Corpus output: $CORPUS_DIR"
echo "[*] Press Ctrl-C to stop all instances."

# Forward Ctrl-C to all AFL children
cleanup() {
    echo ""
    echo "[*] Stopping all AFL instances..."
    for pid in "${AFL_PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait
    echo "[*] Done."
}
trap cleanup INT TERM

# Wait for all children
wait
