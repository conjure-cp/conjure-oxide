#!/usr/bin/env bash
#
# discard-config-time-changes.sh
#
# Discard local edits to config.toml / stats.toml when the *only* difference
# from HEAD is in keys ending in "-time" (e.g. translation-time, solve-time,
# expected-time), and each changed time is small enough to discard: both values
# are <= 1 second, or the absolute change is <= 10% of the HEAD value.
# If any non-time line differs or any timing change exceeds these thresholds,
# the file is left alone.
#
# Usage:
#   ./tools/discard-config-time-changes.sh            # modified files only
#   ./tools/discard-config-time-changes.sh --dry-run  # show what would change
#
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"

dry_run=false
max_time=1
max_change_percent=10

usage() {
  cat <<'EOF'
Usage: discard-config-time-changes.sh [options]

Restore config.toml / stats.toml from HEAD when the only local changes are
*-time field values and every changed time meets either threshold: both the HEAD
and local values are <= --max-time seconds, or the absolute change is
<= --max-change-percent percent of the HEAD value. Files with any other edits,
added/removed timing fields, or non-numeric timing changes are skipped.

Only considers files modified relative to HEAD.

Options:
  --max-time SECONDS           Small-time threshold (default: 1)
  --max-change-percent PERCENT Relative-change threshold (default: 10)
  --dry-run                    Print paths; do not write files
  -h, --help                   Show this help

Thresholds must be non-negative decimal numbers. When the HEAD value is zero,
only the small-time threshold can permit a change.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) dry_run=true; shift ;;
    --max-time|--max-change-percent)
      option="$1"
      if [[ $# -lt 2 || ! "$2" =~ ^([0-9]+([.][0-9]*)?|[.][0-9]+)$ ]]; then
        echo "ERROR: $option requires a non-negative decimal number" >&2
        exit 1
      fi
      case "$option" in
        --max-time) max_time="$2" ;;
        --max-change-percent) max_change_percent="$2" ;;
      esac
      shift 2
      ;;
    -h|--help) usage; exit 0 ;;
    *) echo "ERROR: unknown option: $1" >&2; usage >&2; exit 1 ;;
  esac
done

# Print file with *-time assignment lines removed (comments kept).
without_time_fields() {
  awk '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }
    /^[[:space:]]*#/ { print; next }
    /-time[[:space:]]*=/ {
      if (split($0, parts, "=") >= 2 && trim(parts[1]) ~ /-time$/) next
    }
    { print }
  ' "$1"
}

# Match timing assignments in order so repeated keys in TOML tables stay distinct.
# Added/removed fields and changed non-numeric values cannot be safely discarded.
time_changes_are_small() {
  awk -v max_time="$max_time" -v max_change_percent="$max_change_percent" '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }
    function numeric(s) {
      return s ~ /^[+]?[0-9]+([.][0-9]+)?([eE][+-]?[0-9]+)?$/
    }
    /^[[:space:]]*#/ { next }
    /-time[[:space:]]*=/ {
      pos = index($0, "=")
      key = trim(substr($0, 1, pos - 1))
      if (key !~ /-time$/) next
      value = trim(substr($0, pos + 1))
      if (FILENAME == ARGV[1]) {
        keys[++old_count] = key
        values[old_count] = value
        next
      }
      ++new_count
      if (new_count > old_count || key != keys[new_count]) exit 1
      old_value = values[new_count]
      if (value == old_value) next
      # Inline comments remain significant, just as other non-time edits do.
      old_comment = old_value
      new_comment = value
      sub(/^[^#]*/, "", old_comment)
      sub(/^[^#]*/, "", new_comment)
      if (old_comment != new_comment) exit 1
      sub(/#.*/, "", old_value)
      sub(/#.*/, "", value)
      old_value = trim(old_value)
      value = trim(value)
      if (!numeric(old_value) || !numeric(value)) exit 1
      old_time = old_value + 0
      new_time = value + 0
      if (old_time <= max_time && new_time <= max_time) next
      change = new_time - old_time
      if (change < 0) change = -change
      # Allow rounding error at the inclusive percentage boundary.
      if (old_time > 0 && change / old_time * 100 <= max_change_percent + 1e-12) next
      exit 1
    }
    END { if (old_count != new_count) exit 1 }
  ' "$1" "$2"
}

cd "$REPO_ROOT"

updated=0
skipped=0
found=0

while IFS= read -r rel || [[ -n "$rel" ]]; do
  [[ -z "$rel" ]] && continue
  found=1
  [[ -f "$rel" ]] || { ((skipped+=1)); continue; }

  if ! git cat-file -e "HEAD:$rel" 2>/dev/null; then
    echo "SKIP (not in HEAD): $rel" >&2
    ((skipped+=1))
    continue
  fi

  head_tmp="$(mktemp)"
  trap 'rm -f "$head_tmp"' RETURN
  git show "HEAD:$rel" > "$head_tmp"

  if cmp -s "$head_tmp" "$rel"; then
    ((skipped+=1))
    rm -f "$head_tmp"
    trap - RETURN
    continue
  fi

  if ! cmp -s <(without_time_fields "$head_tmp") <(without_time_fields "$rel"); then
    echo "SKIP (non-time changes present): $rel" >&2
    ((skipped+=1))
    rm -f "$head_tmp"
    trap - RETURN
    continue
  fi

  if ! time_changes_are_small "$head_tmp" "$rel"; then
    echo "SKIP (timing changes exceed thresholds or cannot be compared): $rel" >&2
    ((skipped+=1))
    rm -f "$head_tmp"
    trap - RETURN
    continue
  fi

  if [[ "$dry_run" == true ]]; then
    echo "Would restore: $rel"
    diff -u --label "$rel (HEAD)" --label "$rel" "$head_tmp" "$rel" || true
    echo
  else
    mv "$head_tmp" "$rel"
    echo "Restored: $rel"
  fi

  [[ -f "$head_tmp" ]] && rm -f "$head_tmp"
  trap - RETURN
  ((updated+=1))
done < <(git diff --name-only --diff-filter=ACMR HEAD -- '**/config.toml' '**/stats.toml')

if [[ "$found" -eq 0 ]]; then
  echo "No config.toml or stats.toml files to process."
  exit 0
fi

echo "Done. updated=$updated skipped=$skipped"
