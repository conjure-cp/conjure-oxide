#!/usr/bin/env bash
#
# discard-config-time-changes.sh
#
# Discard local edits to config.toml / stats.toml when the *only* difference
# from HEAD is in keys ending in "-time" (e.g. translation-time, solve-time,
# expected-time). If any non-time line differs, the file is left alone.
#
# Usage:
#   ./tools/discard-config-time-changes.sh            # modified files only
#   ./tools/discard-config-time-changes.sh --dry-run  # show what would change
#
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"

dry_run=false

usage() {
  cat <<'EOF'
Usage: discard-config-time-changes.sh [options]

Restore config.toml / stats.toml from HEAD when the only local changes are
*-time field values. Files with any other edits are skipped.

Only considers files modified relative to HEAD.

Options:
  --dry-run   Print paths; do not write files
  -h, --help  Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) dry_run=true; shift ;;
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
