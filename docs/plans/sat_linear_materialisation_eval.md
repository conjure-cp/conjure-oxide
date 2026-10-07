# Library-backed materialised linear arithmetic: evaluation

Baseline: `79f8fc3832`. Both sides use the same model and uniform integer representation policy.

Linear comparisons already used PB encoders. The change hoists ready sums, differences, negations and constant products only when a nonlinear operation needs an encoded operand. It creates an ordinary auxiliary integer plus a defining equality; the usual representation rules and selected PB provider handle that equality. Direct/Order negation keeps its clause-free representation rewrite. Standalone binary sum/negation and Direct sum materialisers are removed; multiplication/division/modulo/power still need their internal arithmetic circuits.

The regression model has four nonzero sparse values for x and three values for y: 12 assignments. Product, quotient, absolute value, minimum, maximum and caught-division outputs are independently computed in the benchmark script. All before/after solutions matched these assignments.

## Method

- 31 configurations: compact defaults plus all six integer representations crossed with all five PB providers.
- Three paired, sequential CLI runs per configuration; table values are medians. No other tests or builds ran during the clean timing benchmark.
- No rule tracing or DIMACS export during timing. Clause/variable counts come from separate DIMACS exports of the corresponding configurations.
- For the pinned cases AMO is pairwise and cardinality is RustSAT totalizer, with the same seeds on both sides.
- Rewriter time and solver-call wall time come from execution JSON. End-to-end wall time is measured around the CLI subprocess and includes startup, backend loading, reconstruction and output.
- The full integration portfolio also checks Minion/Conjure agreement and agreement across modelling choices. The new fixture uses aggregate rule tracing to avoid approximately 100 MiB of expected full traces.

## Results

These measurements are a functionality and reuse evaluation, not a general performance claim. Most combinations became slower because fresh result declarations and their structural constraints require more rewriting. Some encodings generate substantially fewer clauses.

| Representation | PB provider | Rewrite before/after (ms) | Solver call before/after (ms) | CLI before/after (ms) | Variables before/after | Clauses before/after |
|---|---|---:|---:|---:|---:|---:|
| compact | compact-auto | 13.75 / 18.12 | 13.79 / 13.25 | 37.39 / 40.02 | 5167 / 4370 | 10839 / 9390 |
| direct | rustsat-generalized-totalizer | 18.97 / 29.79 | 18.50 / 15.89 | 47.97 / 53.83 | 5759 / 2635 | 13038 / 7742 |
| direct | rustsat-binary-adder | 19.00 / 29.75 | 17.28 / 22.37 | 46.78 / 60.31 | 5311 / 1922 | 11958 / 6524 |
| direct | pindakaas-bdd | 18.88 / 29.62 | 17.05 / 8.36 | 46.31 / 46.66 | 5432 / 1590 | 12059 / 5654 |
| direct | rustsat-dynamic-poly-watchdog | 18.90 / 29.60 | 19.72 / 17.20 | 48.51 / 55.02 | 5949 / 2665 | 12995 / 7407 |
| direct | pindakaas-swc | 18.86 / 29.67 | 22.76 / 10.58 | 52.95 / 49.04 | 8844 / 2632 | 18399 / 8296 |
| twos_complement | rustsat-generalized-totalizer | 13.55 / 18.18 | 13.67 / 12.86 | 36.83 / 39.63 | 5167 / 4370 | 10839 / 9390 |
| twos_complement | rustsat-binary-adder | 13.66 / 18.08 | 13.97 / 13.65 | 36.88 / 40.71 | 4501 / 3300 | 9753 / 7761 |
| twos_complement | pindakaas-bdd | 13.59 / 18.09 | 13.37 / 12.01 | 36.46 / 39.21 | 4735 / 3698 | 10024 / 8113 |
| twos_complement | rustsat-dynamic-poly-watchdog | 13.52 / 18.11 | 15.74 / 14.58 | 38.83 / 41.26 | 5904 / 5453 | 11720 / 10596 |
| twos_complement | pindakaas-swc | 13.72 / 17.90 | 21.39 / 24.36 | 45.97 / 53.38 | 9478 / 10578 | 18324 / 20143 |
| offset | rustsat-generalized-totalizer | 19.30 / 29.88 | 14.44 / 13.87 | 43.63 / 52.90 | 5578 / 4528 | 11489 / 10006 |
| offset | rustsat-binary-adder | 19.31 / 30.06 | 14.34 / 14.21 | 43.29 / 53.17 | 5040 / 3708 | 10589 / 8769 |
| offset | pindakaas-bdd | 19.56 / 30.60 | 14.35 / 13.49 | 44.37 / 53.13 | 5236 / 4023 | 10778 / 8962 |
| offset | rustsat-dynamic-poly-watchdog | 19.58 / 30.11 | 15.95 / 15.99 | 45.25 / 55.55 | 6073 / 5281 | 12004 / 10740 |
| offset | pindakaas-swc | 19.48 / 30.12 | 22.00 / 23.79 | 53.16 / 65.13 | 9537 / 10010 | 18397 / 19574 |
| order | rustsat-generalized-totalizer | 91.01 / 172.02 | 21.09 / 25.01 | 122.53 / 208.77 | 6034 / 5642 | 13062 / 13400 |
| order | rustsat-binary-adder | 91.12 / 172.23 | 20.24 / 26.37 | 121.85 / 209.19 | 5542 / 4768 | 11998 / 11766 |
| order | pindakaas-bdd | 91.47 / 171.86 | 18.10 / 19.35 | 120.76 / 202.51 | 5793 / 5047 | 12178 / 11614 |
| order | rustsat-dynamic-poly-watchdog | 93.06 / 174.58 | 22.03 / 27.26 | 126.02 / 213.96 | 6556 / 6284 | 13709 / 14103 |
| order | pindakaas-swc | 91.89 / 173.57 | 24.06 / 25.94 | 128.52 / 212.14 | 9236 / 8926 | 18631 / 19435 |
| rank | rustsat-generalized-totalizer | 16.44 / 34.60 | 15.73 / 17.29 | 43.43 / 62.97 | 5692 / 6335 | 11796 / 12988 |
| rank | rustsat-binary-adder | 15.36 / 33.17 | 15.01 / 17.67 | 40.12 / 60.78 | 5284 / 5684 | 11070 / 12002 |
| rank | pindakaas-bdd | 15.35 / 33.38 | 14.32 / 16.22 | 39.54 / 60.06 | 5383 / 5804 | 11095 / 11853 |
| rank | rustsat-dynamic-poly-watchdog | 15.83 / 36.02 | 16.49 / 19.00 | 45.37 / 66.83 | 6000 / 6634 | 12041 / 13118 |
| rank | pindakaas-swc | 15.71 / 33.60 | 21.50 / 27.34 | 48.64 / 73.19 | 9244 / 11372 | 18074 / 21968 |
| sign_magnitude | rustsat-generalized-totalizer | 50.99 / 93.46 | 18.34 / 20.29 | 79.95 / 125.33 | 6430 / 7840 | 13150 / 15776 |
| sign_magnitude | rustsat-binary-adder | 50.76 / 93.59 | 16.61 / 21.10 | 78.41 / 126.04 | 5765 / 6790 | 12065 / 14239 |
| sign_magnitude | pindakaas-bdd | 50.36 / 93.27 | 16.31 / 18.60 | 78.00 / 123.81 | 5874 / 6918 | 12086 / 14066 |
| sign_magnitude | rustsat-dynamic-poly-watchdog | 50.44 / 93.47 | 19.84 / 22.15 | 80.98 / 127.04 | 7168 / 8931 | 14032 / 17058 |
| sign_magnitude | pindakaas-swc | 51.29 / 94.44 | 24.74 / 31.44 | 97.27 / 139.60 | 10623 / 13852 | 20393 / 26205 |

Compact: 37.39 -> 40.02 ms CLI wall time (+7.0%); 10,839 -> 9,390 clauses (-13.4%); 5,167 -> 4,370 variables (-15.4%). Direct with SWC improves from 52.95 -> 49.04 ms (-7.4%). Rank and sign-magnitude have the clearest regressions; their result-domain representation and actual-value conversions need further work.

The initial full-trace portfolio had the same 140 SAT models before and after. SAT translation increased from 6.816 to 12.256 seconds, while collection decreased from 3.525 to 3.008 seconds. This includes full trace-generation cost and precedes the terminal-node traversal shortcut; it must not be compared directly with the final aggregate-trace fixture timings.

## Verification

- Focused before baseline: 7 integration fixtures passed.
- Focused after implementation: 16 tests passed, including 7 integration fixtures and 9 rule tests.
- All 186 clean CLI benchmark runs matched the independently calculated 12 solutions.
- `make check` passed.
- Full three-core `make test-accept`: 1,674 passed, 12 skipped, 17 slow; 1,971.242 seconds (32m 51s). Doctests: 45 passed, 3 ignored.

## Full-suite timing context

The broad acceptance run was slower on several unchanged backends too. For example, blackhole's Minion time increased from 56.53 to 93.63 seconds (+65.6%) and SAT from 287.64 to 372.72 seconds (+29.6%), with no materialisations and unchanged rule application counts. Killer16's Minion, Z3 and SAT configurations increased by roughly 30-36%, also with unchanged rule application counts. These observations limit causal conclusions from full-suite timings.

Knights is directly affected: 92 old sum materialisations and 92 old negations are replaced by 92 new linear-operand materialisations. Its SAT total increased from 82.44 to 118.64 seconds (+43.9%); Minion increased 14.5% and reference-tool wall time increased 27.2%. This remains a performance follow-up, not evidence of a universal improvement.

## Remaining work

Keep the existing PB fast path for linear comparisons. The remaining implementation cost is materialising a result as a full ordinary declaration, including structural domain constraints and conversion of nonlinear integer representations. Some constraints may be redundant for a derived result, but removing them requires a separate correctness argument covering sparse codes and canonical decoding. Native helpers used inside genuinely nonlinear circuits are still needed; this change does not remove all in-house arithmetic.
