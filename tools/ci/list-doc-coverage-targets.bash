#!/usr/bin/env bash

set -euo pipefail

cargo metadata --format-version=1 --no-deps | jq -r '
  [
    .packages[]
    | select(.name != "tests-integration")
    | {
        name: .name,
        has_lib: any(.targets[]; (.kind | index("lib")) or (.kind | index("proc-macro"))),
        has_bin: any(.targets[]; .kind | index("bin"))
      }
    | select(.has_lib or .has_bin)
  ]
  | sort_by(.name)
  | .[]
  | [
      .name,
      (if .has_lib then "--lib" else empty end),
      (if .has_bin then "--bins" else empty end)
    ]
  | @tsv
'
