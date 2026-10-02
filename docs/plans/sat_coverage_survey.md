# SAT coverage survey, 2026-10-02

SAT is enabled in **327 of 622 runnable integration fixtures**, up from 322 of 620 at the cardinality stage. This stage enables the three former solution-mismatch fixtures and adds `sat-ir/signed-arithmetic` and `sat-ir/weighted-linear`. All enabled fixtures have successful SAT run records. There are **2,663 SAT solution portfolios**; **2,568** use uniform channelling across **239 fixtures**. The cardinality-stage totals were 1,931 SAT portfolios and 1,836 uniform portfolios across 234 fixtures.

## Survey of the 489 previously disabled fixtures

| Final outcome | Fixtures |
| --- | ---: |
| Enabled after full uniform-portfolio verification | 194 |
| Initial CLI error | 225 |
| Initial CLI timeout | 37 |
| Full portfolio failed | 30 |
| Full portfolio timed out | 3 |
| Total | 489 |

The [per-fixture CSV](sat_coverage_survey.csv) records the initial screen, portfolio outcome and failure category. A timeout does not establish unsupported semantics.

The initial screen used four workers, an eight-second compilation limit and a twelve-second solve-process limit. It tried compact and first heuristics, with the configured parsers and one requested solution. A successful compilation alone did not count as success. The screen found 227 candidates, 225 errors and 37 timeouts. Of the errors, 218 left residual constraints, two panicked elsewhere and five returned another error.

Candidates were temporarily enabled with `channelling="uniform"` and `heuristic="x"`. The bounded integration run used four test threads and `TEST_CASE_TIMEOUT=120`, exercising every available model portfolio and the original solution limits. It passed 182 candidates and the new cardinality fixture; 45 candidates failed. Failed configurations and partial goldens were restored.

Nine failures exposed a BinaryValue division-bound panic when a divisor domain included zero. Bounds now consider nonzero endpoints and denominators nearest zero, with checked conversion of quotient extrema. All nine fixtures passed the subsequent full, unbounded acceptance run. After the signed arithmetic fixes, 33 candidates remain disabled: 29 left residual constraints in another portfolio, one panicked elsewhere and three timed out.

The former mismatches in `basic/weighted-sum/04-needs-normalising`, `cnf/neg-div` and `smt/int/simple_negative_mult` are fixed and enabled. BinaryValue multiplication now includes sign extension; division bounds and circuits now floor correctly. `cnf/integer/10-div`, which previously timed out, now passes the full uniform portfolio.

## Verification and limits

The full acceptance workflow, `NEXTEST_TEST_THREADS=4 make test-accept` with nextest's no-fail-fast option, passed **1,505 tests**, with **14 skipped**. Workspace doctests passed. Production Clippy and formatting checks passed. Core and rule libraries contain 225 and 134 unit tests respectively. Weighted PB uses RustSAT GTE, RustSAT binary adder and Pindakaas BDD, with exhaustive signed-weight, constant, complement, bound and allocation checks.

Uniform all-mode exercises the currently implemented SAT integer representations: Direct, Order and BinaryValue (`IntLog`), together with available composite layouts and encoding-algorithm choices. BinaryOffset and BinaryRank remain planned IR kinds. Representation selection is shared across each type family throughout a model.

Acceptance uses the configured Conjure reference checks. Existing search limits compare counts when results are truncated. Seven newly enabled fixtures already exempt Conjure validation: intermediate optimisation solutions, two decision-dependent domains, three dominance fixtures and a type-annotation fixture. Their outputs were additionally checked against the existing Minion/Z3 portfolios: complete output sets agreed where complete search was configured; the remaining cases use their configured limits and allow different subsets.

For this stage, the contribution guide's cleanup script restored 300 timing-only files. Existing timing fields were preserved in 325 files with semantic changes; new run entries retain their measurements. The signed-division fixture budget is deliberately raised from one to ten seconds for complete uniform SAT solution enumeration. No changes outside SAT-enabled fixture directories remain in the integration artefacts. Artefacts and configurations are committed separately from implementation code.

Weighted-stage normal golden verification passed **781 tests**, covering all 327 enabled SAT fixtures and the core/rule tests.

The signed-arithmetic fixture verifies all 48 operand pairs against Conjure and Minion, including minimum signed magnitudes and negative divisors. Z3 is excluded from this fixture because its existing Euclidean division differs from Essence floor division (for example, `-4 / -3` yields 2 instead of 1). The sparse weighted fixture checks all three SAT integer representations and all three PB providers against both Minion and Z3; every portfolio returns the same five solutions.
