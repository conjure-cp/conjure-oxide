#!/usr/bin/env bash
#
# discard-config-time-changes.sh
#
# Revert *-time fields in config.toml files to their committed (HEAD) values,
# while keeping any other local edits.
#
# Usage:
#   ./tools/discard-config-time-changes.sh              # modified config.toml only
#   ./tools/discard-config-time-changes.sh --all      # every tracked config.toml
#   ./tools/discard-config-time-changes.sh --dry-run  # show what would change
#
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"

dry_run=false
all_files=false

usage() {
  cat <<'EOF'
Usage: discard-config-time-changes.sh [options]

Revert keys ending in "-time" (e.g. translation-time, solve-time, expected-time)
to their values in HEAD. Other lines in each config.toml are left unchanged.

Options:
  --all       Process every tracked config.toml (default: only modified files)
  --dry-run   Print paths and diffs; do not write files
  -h, --help  Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --all) all_files=true; shift ;;
    --dry-run) dry_run=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "ERROR: unknown option: $1" >&2; usage >&2; exit 1 ;;
  esac
done

file_ends_with_newline() {
  local file="$1"
  [[ -s "$file" ]] || return 0
  [[ "$(tail -c 1 "$file")" == $'\n' ]]
}

restore_time_fields() {
  local head_file="$1"
  local work_file="$2"
  local out_file="$3"
  local preserve_final_newline="$4"

  awk -v headpath="$head_file" '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }

    function time_key(line,    parts, key) {
      if (line ~ /^[[:space:]]*#/) return ""
      if (line !~ /-time[[:space:]]*=/) return ""
      if (split(line, parts, "=") < 2) return ""
      key = trim(parts[1])
      if (key ~ /-time$/) return key
      return ""
    }

    function section_id(section, key) {
      if (section == "") return key
      return section SUBSEP key
    }

    function read_file(path,    line, key, section) {
      section = ""
      while ((getline line < path) > 0) {
        if (line ~ /^[[:space:]]*\[/) {
          section = line
          gsub(/^[[:space:]]*\[|\][[:space:]]*$/, "", section)
          section = trim(section)
          continue
        }
        key = time_key(line)
        if (key != "") head[section_id(section, key)] = line
      }
      close(path)
    }

    BEGIN {
      read_file(headpath)
      section = ""
    }

    /^[[:space:]]*\[/ {
      section = $0
      gsub(/^[[:space:]]*\[|\][[:space:]]*$/, "", section)
      section = trim(section)
      print
      next
    }

    {
      key = time_key($0)
      id = section_id(section, key)
      if (key != "" && (id in head)) {
        print head[id]
      } else {
        print
      }
    }
  ' "$work_file" > "$out_file"

  if [[ "$preserve_final_newline" == false ]]; then
    perl -i -pe 'chomp if eof' "$out_file"
  fi
}

cd "$REPO_ROOT"

list_config_files() {
  if [[ "$all_files" == true ]]; then
    git ls-files -- '**/config.toml'
  else
    git diff --name-only --diff-filter=ACMR HEAD -- '**/config.toml'
  fi
}

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
  out_tmp="$(mktemp)"
  trap 'rm -f "$head_tmp" "$out_tmp"' RETURN

  git show "HEAD:$rel" > "$head_tmp"
  preserve_final_newline=true
  file_ends_with_newline "$rel" || preserve_final_newline=false
  restore_time_fields "$head_tmp" "$rel" "$out_tmp" "$preserve_final_newline"

  if cmp -s "$rel" "$out_tmp"; then
    ((skipped+=1))
    rm -f "$head_tmp" "$out_tmp"
    trap - RETURN
    continue
  fi

  if [[ "$dry_run" == true ]]; then
    echo "Would update: $rel"
    diff -u --label "$rel" --label "$rel (restored)" "$rel" "$out_tmp" || true
    echo
  else
    mv "$out_tmp" "$rel"
    echo "Updated: $rel"
  fi

  rm -f "$head_tmp"
  [[ -f "$out_tmp" ]] && rm -f "$out_tmp"
  trap - RETURN
  ((updated+=1))
done < <(list_config_files)

if [[ "$found" -eq 0 ]]; then
  echo "No config.toml files to process."
  exit 0
fi

echo "Done. updated=$updated skipped=$skipped"
