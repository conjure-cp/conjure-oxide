#!/usr/bin/env bash
#
# Calculates RustDoc coverage, pretty printing the resulting tables as Github
# Flavoured Markdown for consumption by Github Actions.
#
# Author: niklasdewally
# Date: 2024/12/04

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
JQ_SCRIPT=$(cat <<- 'EOF'
def merge_reports:
  reduce .[] as $report ({};
    reduce ($report | to_entries[]) as $entry (.;
      .[$entry.key] = if has($entry.key) then
        {
          total: (.[$entry.key].total + $entry.value.total),
          with_docs: (.[$entry.key].with_docs + $entry.value.with_docs),
          with_examples: (.[$entry.key].with_examples + $entry.value.with_examples)
        }
      else
        $entry.value
      end
    )
  );

merge_reports
| to_entries
| map(
    {
      file: .key,
      with_docs: .value.with_docs,
      with_examples: .value.with_examples,
      total: .value.total,
      percentage: (if .value.total > 0 then ((100 * .value.with_docs / .value.total) | round) else 0 end),
      percentage_examples: (if .value.total > 0 then ((100 * .value.with_examples / .value.total) | round) else 0 end)
    }
    | .emoji = (if .percentage < 90 then "❌" else "✅" end)
    | .emoji_examples = (if .percentage_examples < 90 then "❌" else "✅" end)
  )
| sort_by(.percentage, .file)
| . as $coverage_info

# pretty print each row as csv

# column names 
| ["File","Percentage Documented","Percentage with examples"] as $cols   

# rows
| $coverage_info
| map( 
  ["\(.file)", 
      "\(.emoji) \(.percentage)% *(\(.with_docs)/\(.total))*",
      "\(.emoji_examples) \(.percentage_examples)% *(\(.with_examples)/\(.total))*"
  ]) as $rows
| $cols, $rows[] | @csv

EOF)

while IFS=$'\t' read -r crate flag1 flag2; do
  flags=()
  if [[ -n "${flag1:-}" ]]; then
    flags+=("${flag1}")
  fi
  if [[ -n "${flag2:-}" ]]; then
    flags+=("${flag2}")
  fi

  echo "## RustDoc coverage for \`${crate}\`"
  echo ""
  RUSTDOCFLAGS='-Z unstable-options --show-coverage --output-format=json' \
    cargo +nightly doc -p "${crate}" "${flags[@]}" --no-deps |\
    jq -s -r "${JQ_SCRIPT}" |\
    pandoc -f csv -t gfm |\
    # pandoc escapes ` in generated markdown, but we want to use it as formatting
    sed 's/\\`/`/g' |\
    sed 's/\\\*/\*/g' 

  echo ""
done < <(bash "${SCRIPT_DIR}/list-doc-coverage-targets.bash")
