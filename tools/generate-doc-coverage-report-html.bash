#!/usr/bin/env bash
#
# Generates the documentation coverage report as a html file.
#
# Similar to ci/generate-doc-coverage-report-md, but generates a full report as
# html, not just the data tables.
#
# USAGE: ./generate-doc-coverage-report-html > report.html
#
# Author: niklasdewally
# Date: 2024/12/04

{
  echo "# Documentation coverage report for \`$(git rev-parse --short HEAD)\`"
  echo "" 
  bash $(dirname "$0")/ci/generate-doc-coverage-report-md.bash 
} | pandoc -t html5 --shift-heading-level-by=-1 --toc --standalone 
