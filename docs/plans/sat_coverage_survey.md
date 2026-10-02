# SAT coverage survey, 2026-10-02

SAT is enabled in **322 of 620 runnable integration fixtures**, up from 130 of 619. This adds 191 existing fixtures and the new `sat-ir/cardinality` fixture. All 322 have successful SAT run records. There are **1,931 SAT solution portfolios**; **1,836** use uniform channelling across **234 fixtures**. Previously there were 340 SAT portfolios, including 245 uniform portfolios across 42 fixtures.

## Survey of the 489 previously disabled fixtures

| Final outcome | Fixtures |
| --- | ---: |
| Enabled after full uniform-portfolio verification | 191 |
| Initial CLI error | 225 |
| Initial CLI timeout | 37 |
| Full portfolio failed | 33 |
| Full portfolio timed out | 3 |
| Total | 489 |

The [per-fixture CSV](sat_coverage_survey.csv) records the initial screen, portfolio outcome and failure category. A timeout does not establish unsupported semantics.

The initial screen used four workers, an eight-second compilation limit and a twelve-second solve-process limit. It tried compact and first heuristics, with the configured parsers and one requested solution. A successful compilation alone did not count as success. The screen found 227 candidates, 225 errors and 37 timeouts. Of the errors, 218 left residual constraints, two panicked elsewhere and five returned another error.

Candidates were temporarily enabled with `channelling="uniform"` and `heuristic="x"`. The bounded integration run used four test threads and `TEST_CASE_TIMEOUT=120`, exercising every available model portfolio and the original solution limits. It passed 182 candidates and the new cardinality fixture; 45 candidates failed. Failed configurations and partial goldens were restored.

Nine failures exposed a BinaryValue division-bound panic when a divisor domain included zero. Bounds now consider nonzero endpoints and denominators nearest zero, with checked conversion of quotient extrema. All nine fixtures passed the subsequent full, unbounded acceptance run. The other 36 candidates remain disabled: 29 left residual constraints in another portfolio, three produced reference-solution mismatches, one panicked elsewhere and three timed out.

Known mismatches remain in `basic/weighted-sum/04-needs-normalising`, `cnf/neg-div` and `smt/int/simple_negative_mult`. `cnf/integer/10-div`, which previously timed out, now passes the full uniform portfolio.

## Verification and limits

The full acceptance workflow, `NEXTEST_TEST_THREADS=4 make test-accept` with nextest's no-fail-fast option, passed **1,497 tests**, with **14 skipped**. Workspace doctests passed. Production Clippy and formatting checks passed. Core and rule libraries contain 222 and 131 unit tests respectively.

Uniform all-mode exercises the currently implemented SAT integer representations: Direct, Order and BinaryValue (`IntLog`), together with available composite layouts and encoding-algorithm choices. BinaryOffset and BinaryRank remain planned IR kinds. Representation selection is shared across each type family throughout a model.

Acceptance uses the configured Conjure reference checks. Existing search limits compare counts when results are truncated. Seven newly enabled fixtures already exempt Conjure validation: intermediate optimisation solutions, two decision-dependent domains, three dominance fixtures and a type-annotation fixture. Their outputs were additionally checked against the existing Minion/Z3 portfolios: complete output sets agreed where complete search was configured; the remaining cases use their configured limits and allow different subsets.

The contribution guide's cleanup script restored 402 timing-only files. Existing timing fields were preserved in 218 files with semantic changes; new run entries retain their measurements. Expected-time budgets were deliberately raised for 7 newly expanded fixtures that now take at least fifteen seconds and twice their old budget, using the maximum observed validation runtime. No changes outside SAT-enabled fixture directories remain in the integration artefacts. Artefacts and configurations are committed separately from implementation code.

Focused normal golden verification passed **770 tests**, covering all 322 SAT-enabled fixtures and the core/rule tests.
