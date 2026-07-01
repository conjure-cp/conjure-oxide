#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: tools/optimised-translation-time.sh

Runs non-skipped integration cases once with --rewriter optimised, translates only
with --no-run-solver, writes per-case CSV, and prints aggregate timing summary.
Dirty-trace and rule-trace environment variables are cleared for each child run.

Environment:
  TEST_ROOT   Integration test root. Default: test-suite/tests/integration
  OXIDE_BIN   conjure-oxide binary. Default: target/release/conjure-oxide
  OUT         Output CSV path. Default: optimised-translation-time.csv
  JOBS        GNU parallel job count. Default: number of local CPUs
  KEEP_TMP    Set to 1 to keep temporary stdout/stderr/info JSON files.

The translation_time_s column is wall-clock time for a direct conjure-oxide
translation command. rewriter_time_s is read from --info-json-path.
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

TEST_ROOT="${TEST_ROOT:-test-suite/tests/integration}"
OXIDE_BIN="${OXIDE_BIN:-target/release/conjure-oxide}"
OUT="${OUT:-optimised-translation-time.csv}"
REWRITER="optimised"
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

export SHELL="${BASH:-$(command -v bash)}"

if [[ ! -x "$OXIDE_BIN" ]]; then
  echo "error: OXIDE_BIN is not executable: $OXIDE_BIN" >&2
  echo "hint: cargo build --release --bin conjure-oxide" >&2
  exit 2
fi

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/oxide-optimised-time.XXXXXX")"
if [[ "${KEEP_TMP:-0}" == "1" ]]; then
  echo "keeping temporary files in $tmpdir" >&2
else
  trap 'rm -rf "$tmpdir"' EXIT
fi
jobs_tsv="$tmpdir/jobs.tsv"

python3 - "$TEST_ROOT" > "$jobs_tsv" <<'PY'
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError as exc:
    raise SystemExit("python 3.11+ is required for tomllib") from exc

test_root = Path(sys.argv[1])

def as_list(value, default):
    if value is None:
        return default
    if isinstance(value, list):
        return [str(x) for x in value]
    return [str(value)]

def is_skipped(value):
    if value is None:
        return False
    if isinstance(value, bool):
        return value
    return str(value).strip().lower() not in {"", "0", "false", "none"}

for essence in sorted(test_root.rglob("input.essence")):
    test_dir = essence.parent
    config_path = test_dir / "config.toml"
    config = {}
    if config_path.exists():
        with config_path.open("rb") as handle:
            config = tomllib.load(handle)
    if is_skipped(config.get("skip")):
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
                print("\t".join([
                    str(test_dir),
                    str(essence),
                    param_field,
                    parser,
                    expander,
                    solver,
                    threshold,
                ]))
PY

csv_escape() {
  local value="${1//$'\r'/ }"
  value="${value//$'\n'/ }"
  value="${value//\"/\"\"}"
  printf '"%s"' "$value"
}

extract_rewriter_stats() {
  local info_json="$1"
  python3 - "$info_json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
if not path.exists():
    print("\t\t\t")
    raise SystemExit

try:
    data = json.loads(path.read_text())
except Exception:
    print("\t\t\t")
    raise SystemExit

runs = data.get("stats", {}).get("rewriterRuns", [])
run = runs[-1] if runs else {}

def duration_seconds(value):
    if not isinstance(value, dict):
        return ""
    secs = value.get("secs", value.get("seconds", 0)) or 0
    nanos = value.get("nanos", value.get("nanoseconds", 0)) or 0
    return f"{float(secs) + float(nanos) / 1_000_000_000:.9f}"

def value(name):
    item = run.get(name, "")
    if item is None:
        return ""
    return str(item)

print("\t".join([
    duration_seconds(run.get("rewriterRunTime")),
    value("rewriterRuleApplicationAttempts"),
    value("rewriterRuleApplications"),
    value("rewriterValueLettingRewrites"),
]))
PY
}

run_one() {
  local test_dir="$1"
  local essence="$2"
  local param="$3"
  local parser="$4"
  local expander="$5"
  local solver="$6"
  local threshold="$7"

  local job_id
  if command -v shasum >/dev/null 2>&1; then
    job_id="$(printf '%s|%s|%s|%s' "$test_dir" "$parser" "$expander" "$solver" | shasum | awk '{print $1}')"
  else
    job_id="$(printf '%s|%s|%s|%s' "$test_dir" "$parser" "$expander" "$solver" | sha1sum | awk '{print $1}')"
  fi

  local stdout_file="$tmpdir/$job_id.stdout"
  local stderr_file="$tmpdir/$job_id.stderr"
  local time_file="$tmpdir/$job_id.time"
  local info_json="$tmpdir/$job_id.info.json"

  local cmd=(
    "$OXIDE_BIN"
    --parser "$parser"
    --rewriter "$REWRITER"
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
      --rewriter "$REWRITER"
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

  local status exit_code old_timeformat elapsed rewriter_time attempts applications value_letting_rewrites stderr_tail
  old_timeformat="${TIMEFORMAT-}"
  TIMEFORMAT='%3R'
  if {
    time env \
      -u CONJURE_DIRTY_TRACE \
      -u CONJURE_RULE_TRACE \
      -u CONJURE_RULE_TRACE_VERBOSE \
      -u CONJURE_RULE_TRACE_AGGREGATES \
      "${cmd[@]}" >"$stdout_file" 2>"$stderr_file"
  } 2>"$time_file"; then
    status="ok"
    exit_code=0
  else
    exit_code=$?
    status="fail"
  fi
  if [[ -n "${old_timeformat+x}" ]]; then
    TIMEFORMAT="$old_timeformat"
  else
    unset TIMEFORMAT
  fi

  elapsed="$(tail -n 1 "$time_file" 2>/dev/null | tr -d '[:space:]')"
  IFS=$'\t' read -r rewriter_time attempts applications value_letting_rewrites < <(extract_rewriter_stats "$info_json")
  stderr_tail="$(tail -20 "$stderr_file" 2>/dev/null || true)"

  csv_escape "$test_dir"; printf ','
  csv_escape "$parser"; printf ','
  csv_escape "$expander"; printf ','
  csv_escape "$solver"; printf ','
  csv_escape "$REWRITER"; printf ','
  csv_escape "$status"; printf ','
  printf '%s,%s,%s,%s,%s,%s,' "$exit_code" "${elapsed:-}" "${rewriter_time:-}" "${attempts:-}" "${applications:-}" "${value_letting_rewrites:-}"
  csv_escape "$stderr_tail"; printf '\n'
}

summarize_csv() {
  local csv_path="$1"
  python3 - "$csv_path" <<'PY'
import csv
import sys
from pathlib import Path

path = Path(sys.argv[1])
rows = list(csv.DictReader(path.open(newline="")))
ok = [row for row in rows if row["status"] == "ok"]
failed = [row for row in rows if row["status"] != "ok"]

def numeric(row, field):
    try:
        return float(row.get(field, "") or 0.0)
    except ValueError:
        return 0.0

total_translation = sum(numeric(row, "translation_time_s") for row in ok)
total_rewriter = sum(numeric(row, "rewriter_time_s") for row in ok)

print(f"wrote {path}")
print(f"ok={len(ok)} failed={len(failed)}")
print(f"total_translation_time_s={total_translation:.6f}")
print(f"total_rewriter_time_s={total_rewriter:.6f}")
print()
print("10 slowest translation runs:")
for index, row in enumerate(sorted(ok, key=lambda row: numeric(row, "translation_time_s"), reverse=True)[:10], start=1):
    print(
        f"{index:2d}. {numeric(row, 'translation_time_s'):10.6f}s  "
        f"{row['test_dir']}  parser={row['parser']} "
        f"expander={row['comprehension_expander']} solver={row['solver']}"
    )

if failed:
    print()
    print("failed runs:")
    for row in failed[:10]:
        message = row.get("stderr_tail", "").strip().replace("\n", " ")
        if len(message) > 180:
            message = message[:177] + "..."
        print(f"- {row['test_dir']} parser={row['parser']} expander={row['comprehension_expander']} solver={row['solver']}: {message}")
PY
}

export OXIDE_BIN REWRITER tmpdir
export -f csv_escape extract_rewriter_stats run_one

{
  echo '"test_dir","parser","comprehension_expander","solver","rewriter","status","exit_code","translation_time_s","rewriter_time_s","rule_attempts","rule_applications","value_letting_rewrites","stderr_tail"'
  if [[ -t 2 ]]; then
    parallel --no-notice --jobs "$JOBS" --eta --colsep '\t' run_one {1} {2} {3} {4} {5} {6} {7} :::: "$jobs_tsv"
  else
    parallel --no-notice --jobs "$JOBS" --colsep '\t' run_one {1} {2} {3} {4} {5} {6} {7} :::: "$jobs_tsv"
  fi
} > "$OUT"

summarize_csv "$OUT"
