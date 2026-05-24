#!/usr/bin/env bash
#
# Prints a per-crate summary of RustDoc coverage in markdown, for consumption by Github Actions.
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

merge_reports as $coverage_by_file
| ($coverage_by_file | to_entries | map(.value + {file: .key})) as $coverage_info
| ($coverage_info | map(.with_examples) | add // 0) as $with_examples
| ($coverage_info | map(.with_docs) | add // 0) as $with_docs
| ($coverage_info | map(.total) | add // 0) as $total
| (if $total > 0 then ((100 * $with_examples / $total) | round) else 0 end) as $percentage_examples
| (if $total > 0 then ((100 * $with_docs / $total) | round) else 0 end) as $percentage_docs
| "\($percentage_examples)% with examples, \($percentage_docs)% documented -- \($with_examples)/\($with_docs)/\($total)"
EOF)

while IFS=$'\t' read -r crate flag1 flag2; do
  flags=()
  if [[ -n "${flag1:-}" ]]; then
    flags+=("${flag1}")
  fi
  if [[ -n "${flag2:-}" ]]; then
    flags+=("${flag2}")
  fi

  echo -n "**${crate}:** "
  RUSTDOCFLAGS='-Z unstable-options --show-coverage --output-format=json' \
    cargo +nightly doc -p "${crate}" "${flags[@]}" --no-deps | \
    jq -s -r "${JQ_SCRIPT}"
done < <(bash "${SCRIPT_DIR}/list-doc-coverage-targets.bash")
