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

ROOT="tests-integration/tests"
ALLOWED_PATHS_DESC="tests-integration/tests/{integration,roundtrip}"

usage() {
  cat <<'EOF'
Usage:
  update-test-configs.sh [options]

Input:
  Test selectors are read from stdin, one per line, as either:
    - test names, e.g. tests_integration_basic_div_04
    - globbed test names, e.g. tests_integration_cnf*
    - paths (absolute or relative to repo root), e.g.
      ./tests-integration/tests/integration/basic/comprehension-01-1
  Paths outside <repo root>/tests-integration/tests/{integration,roundtrip} are ignored.
  If no input is given, your default editor will be opened so you can type in the paths;
  This behaviour is disabled when the script is not ran interactively (e.g input is piped from another command)

Options:
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

  --skip [true|false]
      true    - ensure "skip = true" exists  (default)
      false   - remove any "skip = ..." line
      omitted - do not touch skip

  --create-if-empty
      If the test name/path matches but the test has no config.toml, create an empty one

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

set_skip_true() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"
  awk '
    BEGIN { done=0 }
    /^[[:space:]]*skip[[:space:]]*=/ {
      if (!done) {
        print "skip = true"
        done=1
      }
      next
    }
    { print }
    END {
      if (!done) print "skip = true"
    }
  ' "$file" > "$tmp"
  mv "$tmp" "$file"
}

set_skip_false() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"
  awk '!/^[[:space:]]*skip[[:space:]]*=/' "$file" > "$tmp"
  mv "$tmp" "$file"
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

is_allowed_config_path() {
  local candidate
  candidate="$(realpath -m "$1")"
  case "$candidate" in
    "${REPO_ROOT}/${ROOT}/integration/config.toml" | \
    "${REPO_ROOT}/${ROOT}/integration/"*/config.toml | \
    "${REPO_ROOT}/${ROOT}/roundtrip/config.toml" | \
    "${REPO_ROOT}/${ROOT}/roundtrip/"*/config.toml)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

name_to_config_path() {
  local test_name="$1"
  local suite rel

  case "$test_name" in
    tests_integration_*)
      suite="integration"
      rel="${test_name#tests_integration_}"
      ;;
    tests_roundtrip_*)
      suite="roundtrip"
      rel="${test_name#tests_roundtrip_}"
      ;;
    *)
      return 1
      ;;
  esac

  # Try all combinations of separators between parts:
  #   /   (new path segment)
  #   -   (same segment joined by hyphen)
  #   _   (same segment joined by underscore)
  # Return first existing config.toml.

  local IFS=_
  local parts=($rel)
  local n="${#parts[@]}"
  (( n >= 1 )) || return 1

  local candidates=("${REPO_ROOT}/${ROOT}/${suite}/${parts[0]}")
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

  printf '%s\n' "$fallback"
}

is_glob_pattern() {
  local s="$1"
  [[ "$s" == *[\*\?\[]* ]]
}

is_path_input() {
  local s="$1"
  [[ "$s" == /* || "$s" == ./* || "$s" == ../* || "$s" == */* ]]
}

config_path_to_glob_key() {
  local config_path="$1"
  local rel="${config_path#${REPO_ROOT}/${ROOT}/}"
  [[ "$rel" != "$config_path" ]] || rel="${config_path#${ROOT}/}"
  rel="${rel%/config.toml}"

  local suite="${rel%%/*}"
  rel="${rel#*/}"

  case "$suite" in
    integration|roundtrip) ;;
    *) return 1 ;;
  esac

  rel="${rel//\//_}"
  rel="${rel//-/_}"
  printf 'tests_%s_%s\n' "$suite" "$rel"
}

resolve_path_input_to_config() {
  local input="$1"
  local abs_input candidate base
  local has_trailing_slash=0
  [[ "$input" == */ ]] && has_trailing_slash=1

  # Handle absolute or relative path
  if [[ "$input" == /* ]]; then
    abs_input="$(realpath -m "$input")"
  else
    abs_input="$(realpath -m "${REPO_ROOT}/${input}")"
  fi

  candidate="$abs_input"
  base="${candidate##*/}"

  # If path ends in a file, replace it with config.toml
  # (useful so you can pipe in paths from find / ripgrep)
  if [[ -d "$candidate" || "$has_trailing_slash" -eq 1 ]]; then
    candidate="${candidate%/}/config.toml"
  elif [[ "$base" == "config.toml" ]]; then
    :
  elif [[ -f "$candidate" || "$base" == *.* ]]; then
    candidate="${candidate%/*}/config.toml"
  else
    candidate="${candidate%/}/config.toml"
  fi

  candidate="$(realpath -m "$candidate")"

  if is_allowed_config_path "$candidate"; then
    printf '%s\n' "$candidate"
  else
    return 1
  fi
}

resolve_test_input_paths() {
  local input="$1"

  if is_glob_pattern "$input"; then
    [[ "$input" == tests_integration_* || "$input" == tests_roundtrip_* ]] || return 1
    local p key
    for p in "${ALL_CONFIGS[@]}"; do
      key="$(config_path_to_glob_key "$p" || true)"
      [[ -n "$key" && "$key" == $input ]] && printf '%s\n' "$p"
    done
    return 0
  fi

  if is_path_input "$input"; then
    resolve_path_input_to_config "$input"
    return $?
  fi

  name_to_config_path "$input"
}

solvers_csv=""
comment_text=""
remove_pattern=""
remove_pattern_mode=""
remove_pattern_value=""
skip_opt=""
parser_opt=""
rewriter_opt=""
ce_opt=""
dry_run="false"
create_if_empty="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
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
    --skip)
      # Optional value: --skip => true, --skip false => false
      if [[ $# -ge 2 && "$2" != --* ]]; then
        skip_opt="$(printf '%s' "$2" | tr '[:upper:]' '[:lower:]')"
        [[ "$skip_opt" == "true" || "$skip_opt" == "false" ]] || err "--skip must be true or false"
        shift 2
      else
        skip_opt="true"
        shift
      fi
      ;;
    --create-if-empty)
      create_if_empty="true"
      shift
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

cd "$REPO_ROOT"

mapfile -t ALL_CONFIGS < <(
  {
    find "${REPO_ROOT}/${ROOT}/integration" -type f -name config.toml 2>/dev/null
    find "${REPO_ROOT}/${ROOT}/roundtrip" -type f -name config.toml 2>/dev/null
  } | sort
)

cleanup_tmp=""
test_names=()

if [[ -t 0 ]]; then
  cleanup_tmp="$(mktemp)"
  trap 'rm -f "$cleanup_tmp"' EXIT
  "${EDITOR:-vi}" "$cleanup_tmp"
  mapfile -t test_names < <(sed '/^[[:space:]]*$/d' "$cleanup_tmp")
else
  mapfile -t test_names < <(sed '/^[[:space:]]*$/d')
fi

if [[ "${#test_names[@]}" -eq 0 ]]; then
  # Non-interactive empty input: no-op
  [[ -t 0 ]] && err "No test names provided"
  exit 0
fi

updated=0
skipped=0
declare -A seen_paths=()

for t in "${test_names[@]}"; do
  mapfile -t resolved_paths < <(resolve_test_input_paths "$t" || true)

  if [[ "${#resolved_paths[@]}" -eq 0 ]]; then
    if is_glob_pattern "$t"; then
      echo "WARN: No matches for glob: $t" >&2
    elif is_path_input "$t"; then
      echo "WARN: Ignoring path outside ${ALLOWED_PATHS_DESC}: $t" >&2
    else
      echo "WARN: Invalid test selector (expected tests_integration_... / tests_roundtrip_... or /path/to/test...): $t" >&2
    fi
    ((skipped+=1))
    continue
  fi

  for config_path in "${resolved_paths[@]}"; do
    if [[ -n "${seen_paths[$config_path]:-}" ]]; then
      continue
    fi
    seen_paths["$config_path"]=1

    created_missing_config=0
    baseline_tmp=""

    if [[ ! -f "$config_path" ]]; then
      if [[ "$create_if_empty" == "true" && -d "$(dirname "$config_path")" ]]; then
        created_missing_config=1
        if [[ "$dry_run" == "true" ]]; then
          baseline_tmp="$(mktemp)"
          : > "$baseline_tmp"
        else
          : > "$config_path"
        fi
      else
        echo "WARN: Missing config: $config_path (from $t)" >&2
        ((skipped+=1))
        continue
      fi
    fi

    work_file="$(mktemp)"
    if [[ -f "$config_path" ]]; then
      cp "$config_path" "$work_file"
    else
      : > "$work_file"
    fi

    if [[ -n "$comment_text" ]]; then
      prepend_comment "$work_file" "$comment_text"
    fi

    if [[ -n "$remove_pattern_mode" ]]; then
      remove_lines_matching "$work_file" "$remove_pattern_mode" "$remove_pattern_value"
    fi

    if [[ -n "$skip_opt" ]]; then
      if [[ "$skip_opt" == "true" ]]; then
        set_skip_true "$work_file"
      else
        set_skip_false "$work_file"
      fi
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

    compare_from="$config_path"
    [[ -n "$baseline_tmp" ]] && compare_from="$baseline_tmp"

    if cmp -s "$compare_from" "$work_file"; then
      if [[ "$created_missing_config" -eq 1 ]]; then
        if [[ "$dry_run" == "true" ]]; then
          echo "Would create: $config_path"
        else
          echo "Created: $config_path"
        fi
        ((updated+=1))
      fi
      rm -f "$work_file"
      [[ -n "$baseline_tmp" ]] && rm -f "$baseline_tmp"
      continue
    fi

    if [[ "$dry_run" == "true" ]]; then
      if [[ "$created_missing_config" -eq 1 ]]; then
        echo "Would create: $config_path"
      else
        echo "Would update: $config_path"
      fi
      diff --color -u --label "$config_path" --label "$config_path (updated)" "$compare_from" "$work_file" || true
      echo
      rm -f "$work_file"
      [[ -n "$baseline_tmp" ]] && rm -f "$baseline_tmp"
    else
      mv "$work_file" "$config_path"
      if [[ "$created_missing_config" -eq 1 ]]; then
        echo "Created: $config_path"
      else
        echo "Updated: $config_path"
      fi
      [[ -n "$baseline_tmp" ]] && rm -f "$baseline_tmp"
    fi

    ((updated+=1))
  done
done

echo "Done. updated=$updated skipped=$skipped"
