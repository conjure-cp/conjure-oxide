# Native SAT integer numeric views: evaluation

Baseline: `f9d82ebfcf`; implementation: `66c8ec627d`. RustSAT 0.7.5 and Pindakaas 0.5.1 remain unchanged.

## Refinements implemented

- Dense BinaryRank is affine: `value = minimum + unsigned_code`. It now supplies the same weighted bits and bounded-binary group as BinaryOffset, without adding the minimum through a two's-complement circuit. Sparse rank is deliberately excluded: code 1 in `{-3, 2}` denotes 2, not -2.
- SignMagnitude now supplies `value = sum(2^i * magnitude_i) - sum(2^(i+1) * (sign AND magnitude_i))`. RustSAT atomic gate helpers encode/cache the conjunctions; the selected RustSAT/Pindakaas PB provider encodes the numeric relation. Literal gates simplify, including machine-integer extremes. The combined dependent terms are not tagged as a single bounded-binary group.
- Representation references expose these views before SATInt materialisation. Linear comparisons, materialised linear definitions, allDifferent, tables, element, objectives and weighted dominance can consume them directly. All five connected PB providers remain selectable.

No new clause encoder, public option or algorithm family was introduced. BinaryValue already is two's complement by design; these changes remove temporary conversions from other representations. Uniform representation selection for declarations and auxiliaries is unchanged.

## Library audit and remaining candidates

The public PB APIs are the reusable numerical interface. RustSAT's binary-adder PB encoder encodes bounds on a weighted sum; it does not expose an arbitrary integer result-bit circuit. Pindakaas's `integer::log_enc_add`, integer variables and ternary linear encoder are private (`pub(crate)`), so Oxide cannot call them. The pinned public APIs do not provide native sign-magnitude/rank arithmetic, multiplication, division, modulo or power output encoders.

| Operation/input | Current path | Next native refinement to evaluate |
|---|---|---|
| Linear constraints, all representations except sparse Rank | Library PB relations on actual numeric views | Preserve useful structure for the two sign-magnitude weighted components; benchmark SWC |
| Sparse Rank numeric view | Interval decoding into temporary actual-value bits | Interval predicates encoded by library reified PB thresholds; retain code plus weighted gap corrections |
| Comparison of two Rank values with identical canonical ranges | Sparse values still decode | Compare unsigned codes directly using library PB relations; require identical mappings |
| SignMagnitude absolute value | Actual-value binary abs circuit | Reuse magnitude bits directly as an unsigned value; handle unrepresentable `abs(i32::MIN)` explicitly |
| min/max and general abs | Actual-value binary circuits (Direct has its own abs lowering) | Represented result plus guarded library equalities/bounds; benchmark against existing circuits |
| Direct/Order negation | Clause-free representation views | Retain these views |
| Nonlinear multiplication, division, modulo, power | Existing binary arithmetic circuits | No matching public library output encoder; retain pending a measured replacement |

The sparse-rank proposal is a separate implementation task: predicates must be explicitly refined/connected to library decisions, not hidden unlowered integer comparisons inside Boolean PB terms. Native rank comparisons must not confuse differently mapped codes. None of the remaining candidates above was implemented in this change.

## Measurement

The existing `sat-ir/materialised-linear` model covers sparse input values, materialised sums/differences/scaling, product, quotient, abs, min/max and caught division. Every configuration returned the independently enumerated 12 assignments.

31 configurations: compact defaults, then six representations crossed with five PB providers. Each has three sequential paired before/after CLI runs; results are medians. The final timing run had no competing builds/tests. It uses no rule tracing or DIMACS export. Separate exports supply variable/clause counts and also validate solutions. AMO pairwise and RustSAT cardinality totalizer are pinned in the interactive representation cases. Timing units in the accompanying CSV are seconds.

This is one small workload, not a general heuristic tuning result. Rank improvements include dense auxiliary values: the sparse source variable still needs interval decoding for nonlinear operations. SignMagnitude improves translation substantially but exposes a large SWC encoding cost. Other representations are controls and produce identical variable/clause counts before/after; their small timing changes are noise.

| Representation | PB | Rewrite before/after (ms) | Solver before/after (ms) | CLI before/after (ms) | Variables before/after | Clauses before/after |
|---|---|---:|---:|---:|---:|---:|
| compact | compact-auto | 18.88 / 18.88 | 13.49 / 13.65 | 41.33 / 41.56 | 4370 / 4370 | 9390 / 9390 |
| rank | rustsat-generalized-totalizer | 34.67 / 28.82 | 17.62 / 16.58 | 62.50 / 55.44 | 6335 / 5583 | 12988 / 11738 |
| rank | rustsat-binary-adder | 34.81 / 28.80 | 18.37 / 16.88 | 63.23 / 55.55 | 5684 / 4953 | 12002 / 10773 |
| rank | pindakaas-bdd | 34.66 / 29.04 | 16.50 / 15.79 | 62.25 / 55.15 | 5804 / 5157 | 11853 / 10788 |
| rank | rustsat-dynamic-poly-watchdog | 34.88 / 28.81 | 18.74 / 17.96 | 63.21 / 56.15 | 6634 / 5892 | 13118 / 11870 |
| rank | pindakaas-swc | 34.25 / 28.82 | 28.16 / 25.83 | 76.26 / 67.38 | 11372 / 10531 | 21968 / 20517 |
| sign_magnitude | rustsat-generalized-totalizer | 95.49 / 51.58 | 20.80 / 21.56 | 127.32 / 82.61 | 7840 / 5881 | 15776 / 13133 |
| sign_magnitude | rustsat-binary-adder | 96.41 / 51.54 | 21.81 / 22.09 | 129.22 / 83.25 | 6790 / 4460 | 14239 / 10888 |
| sign_magnitude | pindakaas-bdd | 95.59 / 51.07 | 19.22 / 18.79 | 125.80 / 79.73 | 6918 / 4714 | 14066 / 11028 |
| sign_magnitude | rustsat-dynamic-poly-watchdog | 95.18 / 50.97 | 22.75 / 24.00 | 129.23 / 84.31 | 8931 / 7174 | 17058 / 14468 |
| sign_magnitude | pindakaas-swc | 95.56 / 51.11 | 31.71 / 47.95 | 140.31 / 114.71 | 13852 / 25293 | 26205 / 51163 |

SWC sign-magnitude clauses increase from 26,205 to 51,163; solver time also increases. This is a composition/performance tradeoff, not evidence of a library correctness bug. No new upstream bug was found. Compact's representation/provider selection was not changed. All configurations, including SWC, agree on solutions.

## Verification

- `make check` passed.
- All 291 rule/representation tests passed, including exhaustive small sign-magnitude code/scale checks, dense rank weighted views, sparse rank rejection, and literal machine-integer extremes through semantic consumers.
- All 51 SAT-IR integration fixtures passed with three workers (141.341 s); portfolios cover reification, all representations, all connected providers, tables, compound element/allDifferent operands, objectives and dominance. Recorded artefacts were updated separately from code; timing fields were retained.
- Paired CLI timings: 186 successful runs. Separate CNF exports: 62 successful runs.
- The full workspace integration suite was not rerun for this evaluation.

All 31 measured configurations are in [the CSV](sat_native_numeric_views_eval.csv). Scratch scripts, JSON/solution files, DIMACS exports and logs are under `target/native-value-eval/`.
