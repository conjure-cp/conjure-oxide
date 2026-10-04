# SAT coverage survey, 2026-10-04

SAT is enabled in **384 of 632 runnable integration fixtures**. All enabled fixtures have successful SAT run records, exercising **21,423 SAT solution portfolios**; **21,286** use uniform channelling across **296 fixtures**. The element stage enables twelve existing fixtures and adds `sat-ir/element`, with implication/support compositions, actual sparse index labels and scalar definedness guards.

The structured-input stage had 328 successful SAT fixtures and 4,333 portfolios. BinaryOffset and BinaryRank expanded the previous 3,355 portfolios and added `sat-ir/unsigned-integers`. The weighted stage enabled three former solution-mismatch fixtures and added signed-arithmetic and weighted-linear fixtures; the cardinality-stage totals were 322 of 620 fixtures, 1,931 SAT portfolios and 1,836 uniform portfolios across 234 fixtures.

The current filtered failure list and targeted rechecks are recorded in [SAT known issues](sat_known_issues.md).

## Survey of the 489 previously disabled fixtures

| Final outcome | Fixtures |
| --- | ---: |
| Currently SAT-enabled | 241 |
| Initial CLI error | 185 |
| Initial CLI timeout | 37 |
| Full portfolio failed to lower | 22 |
| Full portfolio timed out | 3 |
| Full portfolio externally terminated | 1 |
| Total | 489 |

The [per-fixture CSV](sat_coverage_survey.csv) records the initial screen, portfolio outcome and failure category. A timeout does not establish unsupported semantics.

The initial screen used four workers, an eight-second compilation limit and a twelve-second solve-process limit. It tried compact and first heuristics, with the configured parsers and one requested solution. A successful compilation alone did not count as success. The screen found 227 candidates, 225 errors and 37 timeouts. Of the errors, 218 left residual constraints, two panicked elsewhere and five returned another error.

Candidates were temporarily enabled with `channelling="uniform"` and `heuristic="x"`. The bounded integration run used four test threads and `TEST_CASE_TIMEOUT=120`, exercising every available model portfolio and the original solution limits. It passed 182 candidates and the new cardinality fixture; 45 candidates failed. Failed configurations and partial goldens were restored.

Nine failures exposed a BinaryValue division-bound panic when a divisor domain included zero. Bounds now consider nonzero endpoints and denominators nearest zero, with checked conversion of quotient extrema. All nine fixtures passed the subsequent full, unbounded acceptance run. After the signed arithmetic fixes, 33 candidates remain disabled: 29 left residual constraints in another portfolio, one panicked elsewhere and three timed out.

The former mismatches in `basic/weighted-sum/04-needs-normalising`, `cnf/neg-div` and `smt/int/simple_negative_mult` are fixed and enabled. BinaryValue multiplication now includes sign extension; division bounds and circuits now floor correctly. `cnf/integer/10-div`, which previously timed out, now passes the full uniform portfolio.

## Verification and limits

The full acceptance workflow, `NEXTEST_TEST_THREADS=4 make test-accept` with nextest's no-fail-fast option, passed **1,535 tests**, with **14 skipped**. Workspace doctests passed. Production Clippy and formatting checks passed. Core and rule libraries contain 243 and 142 unit tests respectively. Weighted PB uses RustSAT GTE, RustSAT binary adder, Pindakaas BDD, RustSAT DPW and Pindakaas SWC, with exhaustive signed-weight, constant, complement, bound and allocation checks.

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

allDifferent stage: scalar integer and Boolean decisions support pairwise disequalities across every integer representation and value-AMO for compatible Direct/Boolean views. Asserted value-AMO composes the selected RustSAT AMO encoder; negated/reified groups compose the selected PB provider and library Boolean gates. Six formerly disabled fixtures now pass full uniform portfolios; `sat-ir/alldifferent` checks sparse domains, constants, repeated operands and both truth values, with 80 SAT portfolios each matching the same 48 Conjure/Minion/Z3 solutions. Two existing matrix fixtures also expand from 50 to 80 portfolios.

Full `NEXTEST_TEST_THREADS=4 make test-accept` passed 1,530 tests (14 skipped), including workspace doctests. Production Clippy and formatting passed. Normal golden verification passed all nine expanded fixtures and the three fixtures with changed capped-search samples. Coverage is 336/625 fixtures and 11,773 SAT portfolios; 11,636 portfolios use uniform channelling across 248 fixtures. Baseline timing fields were preserved in 625 files; measurements for added SAT portfolios were retained. Deliberate budgets rise for invalid-slice (16 to 30 seconds), 1d-alldiff (5 to 10) and 2d-slicing-alldiff (1 to 30); other existing budgets are preserved, and the new fixture has a 30-second budget. No new upstream library bug was confirmed. allDifferentExcept and compound-valued operands remain deferred; table is the next dedicated family.

Table stage: tuple and reduced layered MDD decisions share column/value numeric equalities and library Boolean gates. All seven expanded fixtures pass 100 SAT portfolios each; the new sparse/reified fixture returns the same 36 reference solutions in every portfolio. Row lettings, duplicates, repeated operands, out-of-domain cells, constants, both signs and both output truth values are covered. Empty relations and zero-column rows are checked exhaustively. The MDD regression verifies that identical suffixes reduce auxiliary variables.

Full acceptance passed 1,535 tests (14 skipped), plus workspace doctests. Strict production Clippy and formatting passed. Normal golden verification passed all seven table fixtures and three capped-search fixtures. The explicit negativeTable fixture already exempts Conjure validation; all 100 SAT portfolios independently match Minion's complete nine-solution set. Tuple/MDD CLI solution sets agree. The sportsScheduling2 compact screen now lowers its tables but still leaves flatten operations inside allDifferent, so its SAT configuration remains disabled. No new upstream library bug was confirmed.

Coverage is 343/626 fixtures, with 12,473 SAT portfolios; 12,336 portfolios use uniform channelling across 255 fixtures. Six existing fixtures gained SAT and the new fixture adds 100 portfolios. Timing baselines are preserved; the two negative-table fixtures receive deliberate one-to-five-second budgets for added portfolios, and the new fixture has a five-second budget. Table binary-support, short tables and non-constant rows remain follow-ups. Element is the next dedicated decision family; mixed representations and channelling remain deferred.


Element stage: safe scalar lookup definitions retain actual index/value/entry views before library generation. Implication/support compositions share library numeric equalities and Boolean gates; internal membership guards lower to interval/singleton relations. All thirteen expanded fixtures pass 100 SAT portfolios each. The new sparse, reified and masked fixture matches the same 48 reference solutions in every portfolio. The existing matrix-outbounds fixture expands from 22 to 46 portfolios.

Full `NEXTEST_TEST_THREADS=4 make test-accept` passed 1,542 tests (14 skipped), plus workspace doctests. Core/rule libraries contain 246/145 unit tests. Production Clippy and formatting passed. Explicit CLI checks for both element strategies and every PB provider return the same 48 solutions. Normal golden verification passes all expanded fixtures, the two changed existing traces, all nine remaining changed-statistics fixtures and all three changed capped-search fixtures (27 checks).

Timing cleanup preserved baseline fields in 629 files and retained SAT measurements for added portfolios. The 2D matrix-literal fixture deliberately raises its budget from one to five seconds; other existing budgets are preserved. The new element fixture has a five-second budget. No new upstream library bug was confirmed. The subsequent stage below connects ElementId and scalar lexicographic comparisons; compound-valued lookups, remaining arithmetic and guarded/reified encoder connections remain follow-ups; mixed representations and channelling remain deferred.


Identity-element stage: ElementId preserves forward selection with an index-valued fallback; internal inverse lookup is now the separate IndexOf node. Scalar lexicographic comparisons share existing numeric relations and Boolean gates. Bubble propagation respects catchUndef boundaries, and solver projection removes represented internal auxiliaries from user solution enumeration. Minion literal lookups include identity entries across finite bounds. Conjure's Boolean-matrix ElementId typing differs from Savile Row's numeric operation, so the reference fixture explicitly converts entries with toInt; separate unit and complete CLI checks cover direct Boolean entries.

Full acceptance passed 1,550 tests (14 skipped), plus the earlier workspace doctests. Core/rule libraries contain 247/151 unit tests. Production Clippy and formatting passed. Both element strategies and all five PB providers match the independently enumerated 80 solutions for both regression forms (20 explicit CLI checks); Minion matches the complete reference assignment set as well.

Eight existing fixtures gained SAT and the new fixture adds 100 SAT portfolios. Coverage is 365/628 fixtures, with 14,447 successful SAT portfolios; 14,310 portfolios use uniform channelling across 277 fixtures. The disabled inventory now contains 263 fixtures, whose historical failures still need fresh checks before diagnosis. No new upstream library bug was confirmed. SAT IndexOf/function-domain inverse lookup, compound operands and the previously deferred encoding families remain follow-ups; mixed representations and channelling remain deferred.

Normal golden verification passed all 28 changed integration fixtures and the changed custom rule-attempt-trace fixture (29 checks). Timing cleanup preserved baseline fields in 628 files. Added SAT portfolios deliberately raise simpleElementId/valsymElementId/varsymElementId budgets from one to 60/90/10 seconds; the new fixture has a ten-second budget. Other existing budgets remain unchanged.

## Modulo and full function portfolios

Floor modulo connects the three basic modulo fixtures and eight previously blocked function fixtures. The new masked signed regression adds 100 portfolios; total coverage is 377/629 fixtures and 19,073 SAT portfolios. Uniform portfolios account for 18,936 across 289 fixtures. Trials used `TEST_CASE_TIMEOUT=120`; sparse partial functions still fail on occurrence-cardinality lowering, and total surjective integer functions exceeded that limit. The 252 disabled fixtures include those fresh observations and the remaining historical failures. No new upstream library bug was confirmed.

Full acceptance passed 1,560 workspace tests (14 skipped) and all workspace doctests. Twelve new/expanded fixture goldens passed normal verification. Timing cleanup preserved baselines in 630 files and restored capped-search selection churn in three unrelated fixtures; these receive separate normal verification. The new modulo regression has a ten-second budget, and expanded function portfolios retain their deliberate budget increases. Production Clippy and formatting passed.

## Asserted conjunction stage

Native cardinality and PB bounds beneath asserted conjunctions now reach the existing library providers while retaining the evaluator's grouped worklist. The new regression has 100 SAT portfolios. Total surjective integer functions add 1,000 portfolios after a successful 600-second trial. Coverage at that stage was 379/630 fixtures, 20,173 SAT portfolios; 20,036 uniform portfolios across 291 fixtures. There are 251 disabled fixtures. Sparse partial functions passed 600 portfolios but were terminated by SIGKILL after about 472 seconds; the cause is unconfirmed, and their configuration and trial artefacts were restored. At that stage, direct reified counts remained a separate known connection gap. No new library bug was confirmed.

Full four-thread workspace acceptance passed all 1,563 tests (14 skipped); workspace doctests passed separately. Timing baselines were preserved in 631 files. The new function portfolio has a deliberate 210-second budget; the conjunction regression has a five-second budget. All ten normal golden checks passed, covering both new/expanded fixtures and eight changed or restored existing fixture goldens. Production Clippy and formatting passed.

## Guarded-count connection and small function portfolios

Guarded Boolean counts now use existing output-bearing numeric relation decisions and the selected PB library. Asserted counts retain AMO/cardinality selection. The 300-portfolio `sat-ir/guarded-counts` regression includes direct reification, implication, disjunction, negation, all six relations, reversed operands, duplicate inputs and `toInt`. Fifty explicit integer/cardinality/PB portfolios match nine independent and Conjure assignments.

The original sparse-partial fixture passes pinned Packed + BinaryValue, but its full uniform portfolio remains disabled. Direct representation grows to 117,649 codes for its packed function and exceeds a bounded rewrite-only probe; pairwise AMO would require nearly seven billion clauses. This does not confirm the cause of its earlier SIGKILL. A three-by-three full sweep exceeded 120 seconds; reducing the codomain to two values produces `sat-ir/sparse-partial-small`, which passes 800 SAT portfolios under the same bound.

Fresh uniform trials enable both basic Boolean-to-integer fixtures and nested absolute arithmetic, adding 150 portfolios. Coverage is now 384/632 runnable fixtures and 21,423 SAT portfolios, including 21,286 uniform portfolios across 296 fixtures. No new upstream bug was confirmed. Full four-thread workspace acceptance passed all 1,566 tests (14 skipped), including workspace doctests. All eight normal golden checks passed, covering five new/expanded fixtures and three restored capped-search samples. Timing baselines were preserved in 631 files. Existing budgets remain unchanged; the new guarded-count and small function fixtures have deliberate ten- and 180-second budgets. Production Clippy and formatting passed.
