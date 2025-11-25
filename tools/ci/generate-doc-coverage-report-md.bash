#!/usr/bin/env bash
#
# Calculates documentation coverage, pretty printing the resulting tables as Github
# Flavoured Markdown for consumption by Github Actions.
#
# Author: niklasdewally
# Date: 2024/12/04

CRATES="conjure-cp minion-sys tree-morph"
JQ_SCRIPT=$(cat <<- 'EOF'
# convert object {"file":{total:...,with_docs:....}}, to array [{file:...,total:...,}]
# add percentages and check / cross emojis to each file

[keys[] as $k
  | .[$k]
  | .file = $k
  | .percentage = ((.with_docs / .total)*100 | round)
  | .percentage_examples = ((.with_examples / .total)*100 | round)
  | .emoji = (if .percentage < 90 then "❌" else "✅" end) 
  | .emoji_examples = (if .percentage_examples < 90 then "❌" else "✅" end) 
 ]
| sort_by(.percentage)
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

for crate in ${CRATES}; do
  echo "## Documentation coverage for \`${crate}\`"
  echo ""
  RUSTDOCFLAGS='-Z unstable-options --show-coverage --output-format=json' cargo +nightly doc -p ${crate} --no-deps |\
    jq -r "${JQ_SCRIPT}" |\
    pandoc -f csv -t gfm |\
    # pandoc escapes ` in generated markdown, but we want to use it as formatting
    sed 's/\\`/`/g' |\
    sed 's/\\\*/\*/g' 

  echo ""
done
