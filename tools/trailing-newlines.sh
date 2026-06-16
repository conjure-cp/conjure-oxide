#!/usr/bin/env bash
#
# trailing-newlines.sh
#
# Check or fix missing final newlines in tracked text files.
#
# Usage:
#   ./tools/trailing-newlines.sh          # report files missing a final newline
#   ./tools/trailing-newlines.sh --fix    # append a final newline where missing
#
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"

fix=false

usage() {
  cat <<'EOF'
Usage: trailing-newlines.sh [--fix]

Ensures tracked text files end with a single trailing newline.

Options:
  --fix   Append a final newline to files that are missing one
  -h, --help
          Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fix) fix=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "ERROR: unknown option: $1" >&2; usage >&2; exit 1 ;;
  esac
done

cd "$REPO_ROOT"

is_text_file() {
  local file="$1"
  case "$file" in
    crates/minion-sys/vendor/*) return 1 ;;
  esac
  case "$file" in
    *.png|*.jpg|*.jpeg|*.gif|*.ico|*.pdf|*.woff|*.woff2|*.ttf|*.eot|*.bin|*.exe|*.so|*.dylib|*.a|*.rlib|*.o|*.zip|*.gz|*.tar|*.bz2|*.wasm|*.pyc|*.DS_Store)
      return 1
      ;;
  esac
  return 0
}

normalize_trailing_newline() {
  local file="$1"
  local before after
  before="$(wc -c <"$file" | tr -d ' ')"
  perl -0777 -i -pe 'if (length) { s/\n*\z/\n/ }' "$file"
  after="$(wc -c <"$file" | tr -d ' ')"
  [[ "$before" != "$after" ]]
}

file_ends_with_newline() {
  local file="$1"
  [[ ! -s "$file" ]] && return 0
  [[ "$(tail -c 1 "$file" | xxd -p)" == "0a" ]]
}

missing=0
fixed=0

while IFS= read -r -d '' file; do
  [[ -f "$file" ]] || continue
  is_text_file "$file" || continue
  [[ -s "$file" ]] || continue

  if [[ "$fix" == true ]]; then
    if normalize_trailing_newline "$file"; then
      echo "fixed: $file"
      ((fixed+=1))
    fi
    continue
  fi

  file_ends_with_newline "$file" || {
    echo "missing final newline: $file"
    ((missing+=1))
  }
done < <(git ls-files -z)

if [[ "$fix" == true ]]; then
  echo "Done. fixed=$fixed"
  exit 0
fi

if [[ "$missing" -gt 0 ]]; then
  echo "Found $missing file(s) missing a final newline." >&2
  echo "Run: ./tools/trailing-newlines.sh --fix" >&2
  exit 1
fi

echo "All tracked text files end with a final newline."
