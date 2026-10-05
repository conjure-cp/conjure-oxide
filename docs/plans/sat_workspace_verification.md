# SAT workspace verification, 2026-10-05

This normal golden verification follows the latest SAT/allDifferent/lookup work
and the arbitrary-index scalar lex fix. It supplements the earlier isolated SAT
coverage survey; it does not replace that survey with a directly comparable
performance measurement.

`make test` ran all workspace doctests and release Nextest tests, with four
workers, `MAX_EXPECTED_TIME=0`, a 600-second integration fixture timeout and a
4 GiB process-tree RSS ceiling. Acceptance was disabled. The existing backend
profiles were retained, including compact SAT for large Savile Row fixtures and
uniform SAT channelling. The run finished in about 50 minutes including builds.

The workspace result was **1,586 passed, 42 failed, 14 skipped**. All failures
were integration fixtures: **626 passed and 42 failed out of 668 attempted**.
The integration environment check also passed. All other workspace tests and
doctests passed. There are 673 single-source integration fixtures configured for
SAT, including five globally skipped fixtures. Two multi-source directories do
not generate integration tests.

| First failure in normal verification | Fixtures |
| --- | ---: |
| Missing SAT expected files | 22 |
| Outdated rule traces | 9 |
| Outdated solution snapshot | 1 |
| Process-tree memory over 4 GiB | 5 |
| Fixture timeout at 600 seconds | 3 |
| Panics | 2 |

The accompanying [CSV](sat_workspace_verification.csv) records every attempted
fixture and its first outcome. A missing expected file means that at least the
first SAT portfolio solved before verification stopped; it does not establish
completion of the remaining portfolio. Many such fixtures exceeded resources
in the earlier acceptance survey. Normal verification stops at its first
missing or changed snapshot and cannot remeasure that whole portfolio.

## Snapshot follow-up

All 14 lex fixtures pass acceptance and normal verification. The zero-based
strict/non-strict fixtures have 36/45 solutions and the offset fixtures have
28/45. Comparisons use element order, including unequal lengths, and reuse
integer comparisons for Direct, Order, BinaryValue, BinaryOffset and BinaryRank.

The five known injective-function fixtures were refreshed and pass normal
verification. The full run found nine more changed traces: six permutation
fixtures, two injective/bijective sequence fixtures and
`sat-ir/function-compound-domain`. Their fresh snapshots are recorded separately
from source changes.

`conjure/function/function_complex_01` also needed its old zero-solution Minion
snapshot corrected to six. A fresh Conjure reference produces the same six
functions when entry order is ignored. Its configured reference skip concerns
printed set-key order; an independent unordered-entry comparison checks the
assignments. All-choice SAT and Minion profiles are retained.

All ten additional snapshot refreshes passed complete acceptance and subsequent
normal verification. Combining those checks with the unchanged full-run passes
gives **636 passing fixtures and 32 remaining failures**. This is a targeted
follow-up to the full run, not a second full workspace run. The source fix passes
all 179 rule tests and `make check`.

## Remaining work

The full run confirmed two panics: `basic/lettings/04-domain` (SAT loading expects
a ground domain) and `savilerow/const_matrix_test` (undefined index zero is
accessed during rewriting). The domain-letting panic is subsequently fixed:
SAT loading resolves domain references, including chained aliases, and reports
resolution failures as model errors. All 65 SAT portfolios return five
solutions, and all five `basic/lettings` fixtures pass normal verification.
The constant-matrix indexing panic remains a suitable next correctness fix.

Memory limits were reached by `basic/comprehension/dependent-domains`,
`conjure/relation/relation04_param`, `savilerow/carSequencing`,
`savilerow/magicSequence` and `savilerow/pegSolitaireTable`. Timeouts were
`conjure/function/function_total_bool_06`, `mildly-interesting/gchq-2016` and
`savilerow/lee-distance`. These are bounded performance observations, not proof
of unsupported constraints. `plotting` and `solitaire_battleship` passed in about
239 and 103 seconds under their current compact profiles.

The 22 missing SAT snapshots need a bounded acceptance survey of their complete
portfolios before support or resource outcomes can be claimed. The five global
skips were not attempted here: two `tableshort` parser cases, two pegSolitaire
reference/resource cases and `grocery`. Machine-overflowing power ranges remain
explicitly unsupported. Mixed/non-uniform SAT channelling remains deferred.

No new RustSAT or Pindakaas defect was established by this run. The earlier
masked-power reference bug remains separate; Oxide's guarded Minion lowering
has already been fixed. Existing timing baselines are preserved when refreshing
snapshots.
