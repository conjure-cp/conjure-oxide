#!/usr/bin/env bash
#
# ./update-test-configs.sh
#
# DESCRIPTION: bulk manipulate the config.toml files of integration tests
#
# USAGE:
#   See the "usage" message below. Run script with --help to print it.
#
#   This script uses Linux implementations of bash and awk.
#   It has not been tested on Mac OS and results may vary.
#   Please commit all your code before using, and review the bulk changes manually afterwards!
#
# Author: gskorokhod
# Date: 2026/03/22

set -euo pipefail

ROOT="tests-integration/tests/integration"
ORIG_CWD="$PWD"

usage() {
  cat <<'EOF'
Usage:
  update-test-configs.sh [options]

Options:
  --tests FILE
      File containing test names, one per line, e.g.
      tests_integration_basic_div_04
      Supports basic globbing, e.g.
      tests_integration_cnf*

  --solvers LIST
      Comma-separated enabled solvers, e.g. minion,sat-direct
      (If set, non-listed solver entries are commented out in [solver])

  --comment TEXT
      Adds "# TEXT" at the top of each config.toml

  --remove-lines-matching PATTERN
      Remove all lines matching PATTERN from each config.toml.
      PATTERN is treated as a glob by default (*, ?), e.g.
      "# TODO(repr):*"
      For regex, prefix with re:, e.g.
      "re:^# TODO\(repr\):"

  --parser tree-sitter|via-conjure
      If set, enable only this parser in [parser]

  --rewriter naive|morph
      If set, enable only this rewriter in [rewriter]

  --comprehension-expander native|via-solver|via-solver-ac
      If set, enable only this value in [comprehension-expander]

  --dry-run
      Print modified config.toml paths and unified diffs; do not write files

  -h, --help
      Show this help
EOF
}

err() {
  echo "ERROR: $*" >&2
  exit 1
}

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null || true)"
[[ -n "$REPO_ROOT" ]] || err "Could not determine repository root from: $SCRIPT_DIR"

normalize_csv() {
  # Lowercase + convert spaces to commas + collapse duplicate commas + trim edge commas
  printf '%s' "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | tr ' ' ',' \
    | sed -E 's/,+/,/g; s/^,+//; s/,+$//'
}

validate_choice() {
  local name="$1"
  local value="$2"
  shift 2
  local allowed=("$@")
  local ok=0
  local x
  for x in "${allowed[@]}"; do
    if [[ "$value" == "$x" ]]; then
      ok=1
      break
    fi
  done
  [[ "$ok" -eq 1 ]] || err "Invalid $name: '$value' (allowed: ${allowed[*]})"
}

prepend_comment() {
  local file="$1"
  local text="$2"
  local tmp
  tmp="$(mktemp)"
  {
    printf '# %s\n' "$text"
    cat "$file"
  } > "$tmp"
  mv "$tmp" "$file"
}

glob_to_ere() {
  local glob="$1"
  local out=""
  local i c
  for ((i=0; i<${#glob}; i++)); do
    c="${glob:i:1}"
    case "$c" in
      '*') out+='.*' ;;
      '?') out+='.' ;;
      '.'|'\'|'+'|'('|')'|'['|']'|'{'|'}'|'^'|'$'|'|')
        out+="\\$c"
        ;;
      *) out+="$c" ;;
    esac
  done
  printf '%s' "$out"
}

remove_lines_matching() {
  local file="$1"
  local mode="$2"      # glob | regex
  local pattern="$3"
  local tmp
  tmp="$(mktemp)"

  if [[ "$mode" == "glob" ]]; then
    while IFS= read -r line || [[ -n "$line" ]]; do
      [[ "$line" == $pattern ]] && continue
      printf '%s\n' "$line"
    done < "$file" > "$tmp"
  else
    awk -v pat="$pattern" '
      $0 ~ pat { next }
      { print }
    ' "$file" > "$tmp"
  fi

  mv "$tmp" "$file"
}

append_array_block_if_missing() {
  local file="$1"
  local key="$2"
  local enabled_csv="$3"

  if grep -Eq "^[[:space:]]*${key}[[:space:]]*=" "$file"; then
    return 0
  fi

  local IFS=,
  local vals=($enabled_csv)
  {
    echo
    echo "${key} = ["
    local v
    for v in "${vals[@]}"; do
      [[ -n "$v" ]] && printf '    "%s",\n' "$v"
    done
    echo "]"
  } >> "$file"
}

update_array_block() {
  local file="$1"
  local key="$2"
  local enabled_csv="$3"
  local tmp
  tmp="$(mktemp)"

  KEY="$key" ENABLED_CSV="$enabled_csv" awk '
    function trim(s) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
      return s
    }

    BEGIN {
      key = ENVIRON["KEY"]
      n = split(ENVIRON["ENABLED_CSV"], raw, ",")
      m = 0
      for (i = 1; i <= n; i++) {
        v = trim(raw[i])
        if (v != "") {
          enabled[v] = 1
          order[++m] = v
        }
      }
      in_block = 0
      found = 0
    }

    {
      if (!in_block && $0 ~ "^[[:space:]]*" key "[[:space:]]*=[[:space:]]*\\[[[:space:]]*$") {
        in_block = 1
        found = 1
        print
        next
      }

      if (in_block) {
        if ($0 ~ "^[[:space:]]*\\][[:space:]]*$") {
          for (i = 1; i <= m; i++) {
            v = order[i]
            if (!(v in seen)) {
              printf "    \"%s\",\n", v
            }
          }
          print
          in_block = 0
          next
        }

        line = $0
        if (match(line, /^[[:space:]]*(#[[:space:]]*)?"([^"]+)",[[:space:]]*$/, a)) {
          v = a[2]
          seen[v] = 1
          if (v in enabled) {
            printf "    \"%s\",\n", v
          } else {
            printf "    # \"%s\",\n", v
          }
        } else {
          print line
        }
        next
      }

      print
    }
  ' "$file" > "$tmp"
  mv "$tmp" "$file"

  if ! grep -Eq "^[[:space:]]*${key}[[:space:]]*=" "$file"; then
    append_array_block_if_missing "$file" "$key" "$enabled_csv"
  fi
}

name_to_config_path() {
  local test_name="$1"

  [[ "$test_name" == tests_integration_* ]] || return 1
  local rel="${test_name#tests_integration_}"

  # Try all combinations of separators between parts:
  #   /   (new path segment)
  #   -   (same segment joined by hyphen)
  #   _   (same segment joined by underscore)
  # Return first existing config.toml.

  local IFS=_
  local parts=($rel)
  local n="${#parts[@]}"
  (( n >= 1 )) || return 1

  local candidates=("${ROOT}/${parts[0]}")
  local i base
  for ((i=1; i<n; i++)); do
    local next=()
    for base in "${candidates[@]}"; do
      next+=("${base}/${parts[i]}")
      next+=("${base}-${parts[i]}")
      next+=("${base}_${parts[i]}")
    done
    candidates=("${next[@]}")
  done

  local fallback="${candidates[0]}/config.toml"
  local c
  for c in "${candidates[@]}"; do
    if [[ -f "${c}/config.toml" ]]; then
      printf '%s\n' "${c}/config.toml"
      return 0
    fi
  done

  # None of the options worked.
  printf '%s\n' "$fallback"
}

is_glob_pattern() {
  local s="$1"
  [[ "$s" == *[\*\?\[]* ]]
}

config_path_to_glob_key() {
  local config_path="$1"
  local rel="${config_path#${ROOT}/}"
  rel="${rel%/config.toml}"
  rel="${rel//\//_}"
  rel="${rel//-/_}"
  printf 'tests_integration_%s\n' "$rel"
}

resolve_test_input_paths() {
  local input="$1"

  if is_glob_pattern "$input"; then
    [[ "$input" == tests_integration_* ]] || return 1
    local p key
    for p in "${ALL_CONFIGS[@]}"; do
      key="$(config_path_to_glob_key "$p")"
      if [[ "$key" == $input ]]; then
        printf '%s\n' "$p"
      fi
    done
    return 0
  fi

  name_to_config_path "$input"
}

tests_file=""
solvers_csv=""
comment_text=""
remove_pattern=""
remove_pattern_mode=""
remove_pattern_value=""
parser_opt=""
rewriter_opt=""
ce_opt=""
dry_run="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tests)
      [[ $# -ge 2 ]] || err "--tests requires a file path"
      tests_file="$2"
      shift 2
      ;;
    --solvers)
      [[ $# -ge 2 ]] || err "--solvers requires a value"
      solvers_csv="$(normalize_csv "$2")"
      shift 2
      ;;
    --comment)
      [[ $# -ge 2 ]] || err "--comment requires text"
      comment_text="$2"
      shift 2
      ;;
    --remove-lines-matching)
      [[ $# -ge 2 ]] || err "--remove-lines-matching requires a pattern"
      remove_pattern="$2"
      shift 2
      ;;
    --parser)
      [[ $# -ge 2 ]] || err "--parser requires a value"
      parser_opt="$(printf '%s' "$2" | tr '[:upper:]' '[:lower:]')"
      validate_choice "parser" "$parser_opt" "tree-sitter" "via-conjure"
      shift 2
      ;;
    --rewriter)
      [[ $# -ge 2 ]] || err "--rewriter requires a value"
      rewriter_opt="$(printf '%s' "$2" | tr '[:upper:]' '[:lower:]')"
      validate_choice "rewriter" "$rewriter_opt" "naive" "morph"
      shift 2
      ;;
    --comprehension-expander)
      [[ $# -ge 2 ]] || err "--comprehension-expander requires a value"
      ce_opt="$(printf '%s' "$2" | tr '[:upper:]' '[:lower:]')"
      validate_choice "comprehension-expander" "$ce_opt" "native" "via-solver" "via-solver-ac"
      shift 2
      ;;
    --dry-run)
      dry_run="true"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      err "Unknown option: $1"
      ;;
  esac
done

if [[ -n "$remove_pattern" ]]; then
  if [[ "$remove_pattern" == re:* ]]; then
    remove_pattern_mode="regex"
    remove_pattern_value="${remove_pattern#re:}"
    awk -v pat="$remove_pattern_value" 'BEGIN { _ = ("" ~ pat) }' </dev/null \
      || err "Invalid regex for --remove-lines-matching: $remove_pattern"
  else
    remove_pattern_mode="glob"
    remove_pattern_value="$remove_pattern"
  fi
fi

cleanup_tmp=""
if [[ -z "$tests_file" ]]; then
  cleanup_tmp="$(mktemp)"
  "${EDITOR:-vi}" "$cleanup_tmp"
  tests_file="$cleanup_tmp"
fi

if [[ "$tests_file" != /* ]]; then
  tests_file="$ORIG_CWD/$tests_file"
fi

if [[ ! -f "$tests_file" ]]; then
  err "Tests file not found: $tests_file"
fi

if [[ -n "$cleanup_tmp" ]]; then
  trap 'rm -f "$cleanup_tmp"' EXIT
fi

cd "$REPO_ROOT"

mapfile -t ALL_CONFIGS < <(find "$ROOT" -type f -name config.toml | sort)

mapfile -t test_names < <(sed '/^[[:space:]]*$/d' "$tests_file")
[[ "${#test_names[@]}" -gt 0 ]] || err "No test names found in: $tests_file"

updated=0
skipped=0
declare -A seen_paths=()

for t in "${test_names[@]}"; do
  mapfile -t resolved_paths < <(resolve_test_input_paths "$t" || true)

  if [[ "${#resolved_paths[@]}" -eq 0 ]]; then
    if is_glob_pattern "$t"; then
      echo "WARN: No matches for glob: $t" >&2
    else
      echo "WARN: Invalid test name format (expected tests_integration_...): $t" >&2
    fi
    ((skipped+=1))
    continue
  fi

  for config_path in "${resolved_paths[@]}"; do
    if [[ -n "${seen_paths[$config_path]:-}" ]]; then
      continue
    fi
    seen_paths["$config_path"]=1

    if [[ ! -f "$config_path" ]]; then
      echo "WARN: Missing config: $config_path (from $t)" >&2
      ((skipped+=1))
      continue
    fi

    work_file="$(mktemp)"
    cp "$config_path" "$work_file"

    if [[ -n "$comment_text" ]]; then
      prepend_comment "$work_file" "$comment_text"
    fi

    if [[ -n "$remove_pattern_mode" ]]; then
      remove_lines_matching "$work_file" "$remove_pattern_mode" "$remove_pattern_value"
    fi


    if [[ -n "$solvers_csv" ]]; then
      update_array_block "$work_file" "solver" "$solvers_csv"
    fi

    if [[ -n "$parser_opt" ]]; then
      update_array_block "$work_file" "parser" "$parser_opt"
    fi

    if [[ -n "$rewriter_opt" ]]; then
      update_array_block "$work_file" "rewriter" "$rewriter_opt"
    fi

    if [[ -n "$ce_opt" ]]; then
      update_array_block "$work_file" "comprehension-expander" "$ce_opt"
    fi

    if cmp -s "$config_path" "$work_file"; then
      rm -f "$work_file"
      continue
    fi

    if [[ "$dry_run" == "true" ]]; then
      echo "Would update: $config_path"
      diff --color -u --label "$config_path" --label "$config_path (updated)" "$config_path" "$work_file" || true
      echo
      rm -f "$work_file"
    else
      mv "$work_file" "$config_path"
      echo "Updated: $config_path"
    fi

    ((updated+=1))
  done
done

echo "Done. updated=$updated skipped=$skipped"
