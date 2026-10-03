# SAT coverage survey, 2026-10-03

SAT is enabled in **329 of 624 runnable integration fixtures**. All enabled fixtures have successful SAT run records, exercising **11,183 SAT solution portfolios**; **11,046** use uniform channelling across **241 fixtures**. The integer relation stage adds `sat-ir/integer-relations` and extends PB algorithm selection to numeric equality, disequality and comparisons, including nested Boolean uses.

The structured-input stage had 328 successful SAT fixtures and 4,333 portfolios. BinaryOffset and BinaryRank expanded the previous 3,355 portfolios and added `sat-ir/unsigned-integers`. The weighted stage enabled three former solution-mismatch fixtures and added signed-arithmetic and weighted-linear fixtures; the cardinality-stage totals were 322 of 620 fixtures, 1,931 SAT portfolios and 1,836 uniform portfolios across 234 fixtures.

The current filtered failure list and targeted rechecks are recorded in [SAT known issues](sat_known_issues.md).

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

The full acceptance workflow, `NEXTEST_TEST_THREADS=4 make test-accept` with nextest's no-fail-fast option, passed **1,524 tests**, with **14 skipped**. Workspace doctests passed. Production Clippy and formatting checks passed. Core and rule libraries contain 237 and 139 unit tests respectively. Weighted PB uses RustSAT GTE, RustSAT binary adder, Pindakaas BDD, RustSAT DPW and Pindakaas SWC, with exhaustive signed-weight, constant, complement, bound and allocation checks.

Uniform all-mode exercises Direct, Order, BinaryValue (`IntLog`), BinaryOffset (`IntOffset`) and BinaryRank (`IntRank`), together with available composite layouts and encoding-algorithm choices. Representation selection is shared across each type family throughout a model.

Acceptance uses the configured Conjure reference checks. Existing search limits compare counts when results are truncated. Seven newly enabled fixtures already exempt Conjure validation: intermediate optimisation solutions, two decision-dependent domains, three dominance fixtures and a type-annotation fixture. Their outputs were additionally checked against the existing Minion/Z3 portfolios: complete output sets agreed where complete search was configured; the remaining cases use their configured limits and allow different subsets.

For the initial weighted stage, the contribution guide's cleanup script restored 300 timing-only files. Existing timing fields were preserved in 325 files with semantic changes; new run entries retain their measurements. The signed-division fixture budget is deliberately raised from one to ten seconds for complete uniform SAT solution enumeration. No changes outside SAT-enabled fixture directories remain in the integration artefacts. Artefacts and configurations are committed separately from implementation code.

Weighted-stage normal golden verification passed **781 tests**, covering all 327 enabled SAT fixtures and the core/rule tests.

The signed-arithmetic fixture verifies all 48 operand pairs against Conjure and Minion, including minimum signed magnitudes and negative divisors. Z3 is excluded from this fixture because its existing Euclidean division differs from Essence floor division (for example, `-4 / -3` yields 2 instead of 1). The sparse weighted fixture now checks all five SAT integer representations and all five PB algorithms against both Minion and Z3; every portfolio returns the same five solutions.

DPW/SWC expansion: full workspace acceptance again passed 1,505 tests (14 skipped) and workspace doctests; normal golden verification passed 781 tests. Timing cleanup restored 578 timing-only files and preserved previous timings in the 44 files with semantic statistics changes. The expanded portfolios remain confined to SAT-enabled fixtures.

BinaryOffset/Rank expansion: the unsigned-integers fixture checks five integer representations, six AMO choices, two cardinality choices and five PB choices (300 SAT portfolios), each matching six reference solutions. Sparse, mixed-sign and singleton domains are covered. Unit tests also check unsigned code bounds and mapping at signed 32-bit endpoints. Free Boolean completions are streamed, avoiding eager exponential allocation, and dominance constraints trigger re-solving before further completions. Timing cleanup restored 299 timing-only files and retained prior measurements in 327 files with semantic changes.

BinaryOffset/Rank normal golden verification passed **787 tests**, covering every SAT-enabled fixture and the core/rule packages.

Structured-input stage: Direct choice groups, Order chains and compatible BinaryValue/Offset bounds now reach Pindakaas BDD/SWC. Exhaustive projection checks cover mixed-sign sparse choices, signed binary values, scaled bounds, repeated/complementary terms, all bound directions and the shared allocator. Structured equality uses both inequality directions because Pindakaas 0.5.1's direct choice equality path can reject valid assignments. A choice-bound regression also verifies that an already implied bound needs no extra clauses. Full acceptance passed 1,517 tests and workspace doctests; production Clippy and formatting passed. Coverage remains 328 successful SAT fixtures and 4,333 portfolios. Timing cleanup restored 625 timing-only files; no semantic statistics changed.

Structured-input normal golden verification passed **793 tests**, covering every enabled SAT fixture and all core/rule unit tests. Artefact changes are confined to 48 SAT-enabled directories: 2,295 traces expose the term groups, and 12 capped solution files retain 100 solutions while recording different search subsets.

Integer relation stage: semantic decisions replace the Direct, Order and BinaryValue equality/comparison rewrite circuits. Library PB implication helpers preserve both truth values; Direct choice equality uses library Boolean gates over the indicators. Exhaustive projection tests cover all five PB providers, all six relations, signed/repeated/complemented terms, constants, sparse choices, asserted outputs and allocation order. The new fixture exercises 50 SAT portfolios, each matching nine reference solutions. Full workspace acceptance passed 1,522 tests (14 skipped), workspace doctests, production Clippy and formatting. Timing cleanup restored 377 timing-only files and preserved prior timings in 247 mixed-change files, retaining 13 deliberate budget increases for expanded portfolios.

Integer relation normal golden verification passed **798 tests**, covering every enabled SAT fixture and all core/rule tests. Recorded test changes are confined to SAT-enabled directories; local bug reports remain uncommitted.

Structured integer relation follow-up: compatible Direct choices, Order chains and bounded binary groups now reach Pindakaas BDD/SWC for native assertions and both directions of reification. Exhaustive truth-table checks include asserted outputs, shared input/output variables and extreme bounds. A separate regression checks that choice bounds reduce auxiliary variables and that an impossible reverse implication only constrains its guard. Full acceptance passed 1,524 tests (14 skipped), with workspace doctests, production Clippy and formatting. Coverage remains 329/624 fixtures and 11,183 SAT portfolios.
Normal verification also passed four structured-relation checks and six fixtures containing the changed capped solution samples. Timing-only changes were discarded; the 22 changed SAT solution records and one non-time statistics change are recorded separately from source. No new upstream library bug was confirmed.
