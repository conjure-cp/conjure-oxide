#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: tools/rewrite-flag-matrix.sh [rewriter ...]

Runs translation-only integration cases for a matrix of rewriter configurations and writes CSV.
This script requires GNU parallel and Python 3.11+.

Environment:
  TEST_ROOT   Integration test root. Default: test-suite/tests/integration
  OXIDE_BIN   conjure-oxide binary. Default: target/release/conjure-oxide
  OUT         Output CSV path. Default: rewrite-flag-matrix.csv
  JOBS        GNU parallel job count. Default: number of local CPUs

If no rewriter arguments are provided, every combination of rewriter options is used.
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

TEST_ROOT="${TEST_ROOT:-test-suite/tests/integration}"
OXIDE_BIN="${OXIDE_BIN:-target/release/conjure-oxide}"
OUT="${OUT:-rewrite-flag-matrix.csv}"
if [[ -z "${JOBS:-}" ]]; then
  JOBS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
fi

if ! command -v parallel >/dev/null 2>&1; then
  echo "error: GNU parallel is required" >&2
  exit 2
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required" >&2
  exit 2
fi

if [[ ! -x "$OXIDE_BIN" ]]; then
  echo "error: OXIDE_BIN is not executable: $OXIDE_BIN" >&2
  echo "hint: cargo build --release --bin conjure-oxide" >&2
  exit 2
fi

if [[ "$#" -gt 0 ]]; then
  REWRITERS=("$@")
else
  FLAGS=(prefilter dirty cache rulememo worklist candidateindex dirtyqueues)
  REWRITERS=()
  for ((mask = 0; mask < (1 << ${#FLAGS[@]}); mask++)); do
    rewriter="baseline"
    for ((flag_index = 0; flag_index < ${#FLAGS[@]}; flag_index++)); do
      if ((mask & (1 << flag_index))); then
        rewriter+="+${FLAGS[$flag_index]}"
      fi
    done
    REWRITERS+=("$rewriter")
  done
  REWRITERS+=("optimised")
fi

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/oxide-rewrite-flags.XXXXXX")"
trap 'rm -rf "$tmpdir"' EXIT
jobs_tsv="$tmpdir/jobs.tsv"

python3 - "$TEST_ROOT" "${REWRITERS[@]}" > "$jobs_tsv" <<'PY'
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError as exc:
    raise SystemExit("python 3.11+ is required for tomllib") from exc

test_root = Path(sys.argv[1])
rewriters = sys.argv[2:]

def as_list(value, default):
    if value is None:
        return default
    if isinstance(value, list):
        return [str(x) for x in value]
    return [str(value)]

for essence in sorted(test_root.rglob("input.essence")):
    test_dir = essence.parent
    config_path = test_dir / "config.toml"
    config = {}
    if config_path.exists():
        with config_path.open("rb") as handle:
            config = tomllib.load(handle)
    if str(config.get("skip", "")).strip():
        continue

    parsers = as_list(config.get("parser"), ["tree-sitter"])
    expanders = as_list(config.get("comprehension-expander"), ["native"])
    solvers = as_list(config.get("solver"), ["minion"])
    threshold = str(config.get("minion-discrete-threshold", 0) or 0)
    param = test_dir / "input.param"
    param_field = str(param) if param.exists() else "-"

    for parser in parsers:
        for expander in expanders:
            for solver in solvers:
                for rewriter in rewriters:
                    print("\t".join([
                        str(test_dir),
                        str(essence),
                        param_field,
                        parser,
                        expander,
                        solver,
                        rewriter,
                        threshold,
                    ]))
PY

csv_escape() {
  local value="${1//$'\r'/ }"
  value="${value//$'\n'/ }"
  value="${value//\"/\"\"}"
  printf '"%s"' "$value"
}

extract_json_field() {
  local info_json="$1"
  local field="$2"
  python3 - "$info_json" "$field" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
field = sys.argv[2]
if not path.exists():
    print("")
    raise SystemExit

try:
    data = json.loads(path.read_text())
except Exception:
    print("")
    raise SystemExit

runs = data.get("stats", {}).get("rewriterRuns", [])
run = runs[-1] if runs else {}
value = run.get(field, "")
if isinstance(value, dict):
    secs = value.get("secs", value.get("seconds", 0)) or 0
    nanos = value.get("nanos", value.get("nanoseconds", 0)) or 0
    print(f"{float(secs) + float(nanos) / 1_000_000_000:.9f}")
else:
    print(value if value is not None else "")
PY
}

run_one() {
  local test_dir="$1"
  local essence="$2"
  local param="$3"
  local parser="$4"
  local expander="$5"
  local solver="$6"
  local rewriter="$7"
  local threshold="$8"

  local job_id
  if command -v shasum >/dev/null 2>&1; then
    job_id="$(printf '%s|%s|%s|%s|%s' "$test_dir" "$parser" "$expander" "$solver" "$rewriter" | shasum | awk '{print $1}')"
  else
    job_id="$(printf '%s|%s|%s|%s|%s' "$test_dir" "$parser" "$expander" "$solver" "$rewriter" | sha1sum | awk '{print $1}')"
  fi
  local stdout_file="$tmpdir/$job_id.stdout"
  local stderr_file="$tmpdir/$job_id.stderr"
  local info_json="$tmpdir/$job_id.info.json"

  local start end elapsed status exit_code
  start="$(python3 -c 'import time; print(time.perf_counter())')"

  local cmd=(
    "$OXIDE_BIN"
    --parser "$parser"
    --rewriter "$rewriter"
    --comprehension-expander "$expander"
    --solver "$solver"
    solve
    --no-run-solver
    --info-json-path "$info_json"
    "$essence"
  )
  if [[ "$threshold" != "0" ]]; then
    cmd=(
      "$OXIDE_BIN"
      --parser "$parser"
      --rewriter "$rewriter"
      --comprehension-expander "$expander"
      --solver "$solver"
      --minion-discrete-threshold "$threshold"
      solve
      --no-run-solver
      --info-json-path "$info_json"
      "$essence"
    )
  fi
  if [[ "$param" != "-" ]]; then
    cmd+=("$param")
  fi

  if "${cmd[@]}" >"$stdout_file" 2>"$stderr_file"; then
    status="ok"
    exit_code=0
  else
    exit_code=$?
    status="fail"
  fi

  end="$(python3 -c 'import time; print(time.perf_counter())')"
  elapsed="$(python3 - "$start" "$end" <<'PY'
import sys
print(f"{float(sys.argv[2]) - float(sys.argv[1]):.6f}")
PY
)"

  local rewriter_time attempts applications value_letting_rewrites stderr_tail
  rewriter_time="$(extract_json_field "$info_json" rewriterRunTime)"
  attempts="$(extract_json_field "$info_json" rewriterRuleApplicationAttempts)"
  applications="$(extract_json_field "$info_json" rewriterRuleApplications)"
  value_letting_rewrites="$(extract_json_field "$info_json" rewriterValueLettingRewrites)"
  stderr_tail="$(tail -20 "$stderr_file" 2>/dev/null || true)"

  csv_escape "$test_dir"; printf ','
  csv_escape "$parser"; printf ','
  csv_escape "$expander"; printf ','
  csv_escape "$solver"; printf ','
  csv_escape "$rewriter"; printf ','
  csv_escape "$status"; printf ','
  printf '%s,%s,%s,%s,%s,%s,' "$exit_code" "$elapsed" "${rewriter_time:-}" "${attempts:-}" "${applications:-}" "${value_letting_rewrites:-}"
  csv_escape "$stderr_tail"; printf '\n'
}

export OXIDE_BIN tmpdir
export -f csv_escape extract_json_field run_one

{
  echo '"test_dir","parser","comprehension_expander","solver","rewriter","status","exit_code","elapsed_s","rewriter_time_s","rule_attempts","rule_applications","value_letting_rewrites","stderr_tail"'
  parallel --no-notice --jobs "$JOBS" --eta --colsep '\t' run_one {1} {2} {3} {4} {5} {6} {7} {8} :::: "$jobs_tsv"
} > "$OUT"

echo "wrote $OUT" >&2
