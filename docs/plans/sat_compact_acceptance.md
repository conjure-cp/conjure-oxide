# Full acceptance with compact SAT for slow portfolios

Run: 5-6 October 2026, branch `sat-ir`, after merging `origin/main`.

The requested `NEXTEST_TEST_THREADS=4 make test-accept` ran all normally runnable
workspace tests and doctests. Nextest continued after failures. Integration
fixtures used a 600-second timeout and a 4 GiB process-tree RSS ceiling, recorded
in their configurations. Five globally skipped integration fixtures and nine
other ignored tests were not attempted. Default SAT solving uses CaDiCaL.

The full run completed in about 33.5 minutes including builds: **1,617 passed,
12 failed, 14 skipped**. All failures were integration fixtures. Five function
fixtures had started with all-choice SAT before their profiles were switched to
compact; their compact retries all passed in 1-2 seconds. With those follow-ups,
**661 of 668 attempted integration fixtures pass; seven remain failing**.
All other workspace tests and doctests passed.

Of the 661 passes, 656 actually solve with SAT and five are rewrite-only.
615 validate their solutions against Conjure; 41 solver fixtures explicitly skip
that reference. Passing solver fixtures exercise 50,725 SAT portfolios.
Acceptance also checks agreement between complete modelling portfolios and
backends. These totals describe the current profiles, not every encoding option
on every fixture.

## Profile changes and validation

Existing user changes were retained. Eleven further SAT `x` portfolios with
recorded SAT work over 60 seconds were switched to `c`. Another 21 fixtures with
prior SAT memory/timeout outcomes were switched to `c`; five already-started
attempts were subsequently rerun. Two long all-choice attempts were stopped for
those retries, rather than occupying workers until the timeout. These stopped
attempts are not counted as current failures.

Relative to the starting commit, 45 fixtures now use compact SAT instead of
all-choice SAT, including the user's changes. All 32 profiles changed during
this task also pass normal golden verification. No completed SAT `x` portfolio
spent over 60 seconds in SAT translation plus solving in this run. The attempted
integration profiles are 539 `x` and 129 `c`, all retaining uniform channelling.
Compact chooses one representation per type and its normal encoding options;
no SAT encoder overrides were pinned.

Existing run timing baselines are retained. New compact profiles keep their
measured timings and revised fixture budgets. Timing-only churn was discarded.
Fresh measurements are summarised in the [per-fixture CSV](sat_compact_acceptance.csv).
Failed or interrupted acceptance attempts had their pre-run expected files and
statistics restored; incomplete outputs are not recorded as passing goldens.

## Seven remaining failures

| Fixture | Outcome | Observed cause or stage |
| --- | --- | --- |
| `conjure/function/function_partial_smoke` | Memory over 4 GiB | SAT rewriting of a packed partial-function representation expands repeated division/modulo bit extraction into hundreds of thousands of auxiliaries. |
| `savilerow/carSequencing` | Memory over 4 GiB | SAT rewriting/model construction; the trace still contains unlowered comprehensions and cardinality constraints when the limit is reached. |
| `savilerow/magicSequence` | Memory over 4 GiB | SAT phase after Minion completes; precise allocation hotspot needs profiling. |
| `savilerow/pegSolitaireTable` | Memory over 4 GiB | SAT rewriting of the large table model; the rewrite trace is about 136 MiB at termination. |
| `mildly-interesting/gchq-2016` | Timeout at 600 seconds | Minion completes, but SAT rewriting is still emitting auxiliary definitions and PB relations at termination. |
| `savilerow/lee-distance` | Timeout at 600 seconds | SAT rewriting. A live stack sample shows Direct integer addition and cloning large symbol tables, before SAT search. |
| `savilerow/const_matrix_test` | Panic | Undefined constant matrix index zero is accessed during SAT rewriting: `0 is not a valid index for dimension 0`. |

Six failures already use compact SAT. The index panic still uses `x`, but is a
correctness issue rather than evidence of slow portfolio enumeration. Resource
outcomes do not establish unsupported semantics or an upstream solver defect.
No new RustSAT or Pindakaas bug was established.

The smallest resource reproducer is:

```essence
find f : function (maxSize 2) int(1..10) --> bool
```

It has 201 semantic assignments. Compact lowers it through a relation and a
packed set integer in `0..1048575`. Bit membership uses `(packed / 2^k) % 2`,
which currently reaches general arithmetic circuits rather than direct bit
access. The failed trace reaches auxiliary `__484148` and is about 47 MiB.
This is a concrete remaining connection/efficiency gap in Oxide's lowering.

## Slow passes and unmeasured cases

`blackhole` passes in 590 seconds for the whole fixture, close to the limit.
Measured SAT work is about 446 seconds. Other slow but passing compact SAT
profiles include `plotting` (179 seconds of SAT work), `test-branchingon2` (149),
`knights` (113) and `solitaire_battleship` (89). These are observations under
four-worker load, not a controlled comparison with older timings.

Five passes are rewrite-only: `multiDimensionArray`,
`peaceableArmyOfQueens2-failing`, `quasiGroup3Idempotent`,
`quasiGroup3NonIdempotent` and `quasiGroup4NonIdempotent`. They do not demonstrate
SAT solving support.

The five global skips remain `shorttable-assigntest`, `shorttable-smalltest`,
`pegSolitaireAction`, `pegSolitaireState` and `grocery`. Their existing reasons
include missing `tableshort` parsing, expensive reference/model processing and
Minion domain bounds. They were not remeasured here. Machine-overflowing power
ranges remain explicitly unsupported, and non-uniform SAT channelling remains
deferred.
