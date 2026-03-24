#!/usr/bin/env bash
set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────
CORES="${FUZZ_CORES:-8}"
CAMPAIGN_TIMEOUT="${FUZZ_CAMPAIGN_TIMEOUT:-86400}"  # whole campaign timeout (seconds), default 24h
RUN_TIMEOUT="${FUZZ_RUN_TIMEOUT:-1200}"               # per-harness-run timeout (seconds), default 20min
MEM_LIMIT="${FUZZ_MEM_LIMIT:-2048}"                  # per-instance memory limit (MB), default 2G

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

SEEDS_DIR="$SCRIPT_DIR/seeds"
CORPUS_DIR="$SCRIPT_DIR/corpus"
DICT="$SCRIPT_DIR/essence.dict"
HARNESS="$ROOT_DIR/target/profiling/conjure-fuzz-harness"

# AFL++ environment
export AFL_TESTCACHE_SIZE=500           # shared testcase cache (MB)
export AFL_IGNORE_SEED_PROBLEMS=1       # skip seeds that crash/timeout on startup
export AFL_IMPORT_FIRST=1               # load other fuzzers' findings first
export AFL_FINAL_SYNC=1                 # main instance does a final sync pass

# ── Build ────────────────────────────────────────────────────────────
echo "[*] Building harness..."
cargo afl build -p conjure-cp-fuzz --profile profiling

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
cargo afl cmin -i "$SEEDS_DIR" -o "$SEEDS_MIN_DIR" -m "$MEM_LIMIT" -t "$((RUN_TIMEOUT * 1000))" -- "$HARNESS"
echo "[*] Minimized: $(find "$SEEDS_MIN_DIR" -maxdepth 1 -type f | wc -l) seeds (from $(find "$SEEDS_DIR" -maxdepth 1 -type f | wc -l))"

# ── Common fuzz flags ────────────────────────────────────────────────
RUN_TIMEOUT_MS=$((RUN_TIMEOUT * 1000))
COMMON_FLAGS=(-i "$SEEDS_MIN_DIR" -o "$CORPUS_DIR" -x "$DICT" -m "$MEM_LIMIT" -t "$RUN_TIMEOUT_MS")

# ── Launch AFL instances ─────────────────────────────────────────────
mkdir -p "$CORPUS_DIR"

AFL_PIDS=()

# Power schedules to rotate through for secondaries
SCHEDULES=(fast coe lin quad exploit rare)

# ── Main (deterministic) instance ────────────────────────────────────
echo "[*] Starting $CORES AFL instances (campaign: ${CAMPAIGN_TIMEOUT}s, per-run: ${RUN_TIMEOUT}s, mem: ${MEM_LIMIT}MB)..."

cargo afl fuzz "${COMMON_FLAGS[@]}" \
    -M "main-$(hostname)" \
    -p explore \
    -a binary \
    -- "$HARNESS" &
AFL_PIDS+=($!)

# ── Secondary instances ──────────────────────────────────────────────
# Each secondary gets a randomized combination of AFL++ options following
# the recommended distribution from the AFL++ docs.
NUM_SECONDARIES=$((CORES - 1))

for i in $(seq 1 "$NUM_SECONDARIES"); do
    EXTRA_FLAGS=()
    EXTRA_ENV=()
    SCHEDULE_IDX=$(( (i - 1) % ${#SCHEDULES[@]} ))
    EXTRA_FLAGS+=(-p "${SCHEDULES[$SCHEDULE_IDX]}")

    # ~10%: MOpt mutator
    if (( RANDOM % 10 == 0 )); then
        EXTRA_FLAGS+=(-L 0)
    fi

    # ~10%: old queue cycling
    if (( RANDOM % 10 == 0 )); then
        EXTRA_FLAGS+=(-Z)
    fi

    # ~50-70%: disable trim
    if (( RANDOM % 10 < 6 )); then
        EXTRA_ENV+=("AFL_DISABLE_TRIM=1")
    fi

    # ~40% explore, ~20% exploit (via -P)
    R=$((RANDOM % 10))
    if (( R < 4 )); then
        EXTRA_FLAGS+=(-P explore)
    elif (( R < 6 )); then
        EXTRA_FLAGS+=(-P exploit)
    fi

    # ~30%: no ascii mode, ~30%: ascii, ~40%: binary
    R=$((RANDOM % 10))
    if (( R < 3 )); then
        : # no -a flag
    elif (( R < 6 )); then
        EXTRA_FLAGS+=(-a ascii)
    else
        EXTRA_FLAGS+=(-a binary)
    fi

    env ${EXTRA_ENV[@]+"${EXTRA_ENV[@]}"} \
        cargo afl fuzz "${COMMON_FLAGS[@]}" \
            -S "secondary_$i" \
            "${EXTRA_FLAGS[@]}" \
            -- "$HARNESS" &
    AFL_PIDS+=($!)
done

echo "[*] AFL PIDs: ${AFL_PIDS[*]}"
echo "[*] Corpus output: $CORPUS_DIR"
echo "[*] Will auto-stop after ${CAMPAIGN_TIMEOUT}s. Press Ctrl-C to stop early."

# ── Cleanup on exit ──────────────────────────────────────────────────
cleanup() {
    echo ""
    echo "[*] Stopping all AFL instances..."
    for pid in "${AFL_PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
    echo "[*] Done."
}
trap cleanup INT TERM EXIT

# ── Global timeout ───────────────────────────────────────────────────
# Run a background timer; when it fires, signal ourselves so the trap fires.
(
    sleep "$CAMPAIGN_TIMEOUT"
    echo ""
    echo "[*] Campaign timeout (${CAMPAIGN_TIMEOUT}s) reached."
    kill -TERM $$ 2>/dev/null || true
) &
TIMER_PID=$!

# Wait for all AFL children (or the timeout to fire)
wait "${AFL_PIDS[@]}" 2>/dev/null || true

# If we get here naturally (all fuzzers exited), kill the timer
kill "$TIMER_PID" 2>/dev/null || true
