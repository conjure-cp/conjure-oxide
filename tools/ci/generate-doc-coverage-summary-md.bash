#!/usr/bin/env bash
#
# Prints a per-crate summary of documentation coverage in markdown, for consumption by Github Actions.
#
# Author: niklasdewally
# Date: 2024/12/04

CRATES="conjure-cp minion-sys tree-morph"
JQ_SCRIPT=$(cat <<- 'EOF'
# add percentages to each file

[keys[] as $k
  | .[$k]
  | .file = $k
  | .percentage = ((.with_docs / .total)*100 | round)
  | .percentage_examples = ((.with_examples / .total)*100 | round)
 ] as $coverage_info
 | ($coverage_info | [.[].percentage] | add / length | round) as $avg
 | ($coverage_info | [.[].percentage_examples] | add / length | round) as $avg_examples
 | ($coverage_info | [.[].with_examples] | add) as $with_examples
 | ($coverage_info | [.[].with_docs] | add) as $with_docs
 | ($coverage_info | [.[].total] | add) as $total
 | "\($avg_examples)% with examples, \($avg)% documented -- \($with_examples)/\($with_docs)/\($total)"
EOF)

for crate in ${CRATES}; do
  echo -n "**${crate}:** "
  RUSTDOCFLAGS='-Z unstable-options --show-coverage --output-format=json' cargo +nightly doc -p ${crate} --no-deps | jq -r "${JQ_SCRIPT}"
done
