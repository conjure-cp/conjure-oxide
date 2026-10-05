# Full SAT integration coverage survey

Survey date: 2026-10-05. Complete.

667 of 667 discovered fixtures have completed. SAT is configured for all 667 fixtures: 662 normally runnable and five globally skipped fixtures explicitly attempted by this survey. Two multi-source directories have no generated integration test and are excluded: `cnf/cnf2` and `bugs/experiment/wrong-json-model`.

SAT uses `heuristic="x"` for 622 fixtures and `heuristic="c"` for 45 fixtures. Where other backend profiles exist, SAT follows whether they enumerate choices; SAT-only fixtures retain their configured all-choice profile. Every SAT run retains uniform channelling. Compact selects one representation and one option per encoding family using its normal policy; no SAT options are pinned. Existing Minion/Z3 choices are preserved through `[solver-options.sat]`. Four workers run the primary release integration survey. After the constant-bit power fix, all 15 power-containing fixtures are rerun with one worker under the same profiles and limits; their latest outcomes supersede the earlier attempts. A further four-worker follow-up reruns all 14 occurrence-constraint fixtures after accepting flattened matrix operands; its latest outcomes also supersede the earlier attempts. All runs use a 600-second fixture timeout and a 4 GiB resident-memory ceiling for each fixture and its descendants. A failed portfolio stops its modelling-choice sequence; the harness may continue with another configured parser/rewriter unless the failure panics. The survey continues with other fixtures. Tests run in an isolated fixture copy with `ACCEPT=true`; original goldens and timing baselines are protected.

| Outcome | All fixtures | Normally runnable |
| --- | ---: | ---: |
| frontend | 2 | 0 |
| memory_limit | 31 | 30 |
| panic | 2 | 2 |
| pass | 602 | 602 |
| reference_memory_limit | 2 | 0 |
| residual_constraints | 12 | 12 |
| timeout | 16 | 16 |

Successful SAT portfolios in complete passing fixtures: 61913. Successful portfolio prefixes in failed fixtures: 5363.

Of the completed passes, 563 performed SAT solution validation against Conjure, 36 omit the Conjure reference by explicit configuration (including the independently audited masked-power regression), and 3 are rewrite-only fixtures. Rewrite-only passes do not establish SAT solver support.

A timeout or memory limit is a bounded performance observation, not a confirmed unsupported constraint. Reference/frontend failures may prevent the SAT backend from being reached. The CSV records every fixture, its first failure, resource budget, prior SAT enablement and successful portfolio prefix. Acceptance clears old snapshots before executing a fixture, so successful prefix counts are from this survey. A successful prefix does not establish complete coverage of a failed fixture.

## Changes since the first survey

The first survey passed 565 of 664 fixtures. This rerun includes three new regressions. The per-fixture CSV records the previous outcome and current SAT heuristic so profile changes can be distinguished from backend fixes.

Occurrence membership now guards its lookup domain, making out-of-domain membership false under negation and reification. Boolean gates accept only Boolean atoms, allowing compound equality to reach representation-specific lowering. Native atMost/atLeast and global cardinality accept direct or flattened matrix operands and lower to equality indicators and shared cardinality/PB decisions. SafePow uses exponentiation by squaring through existing Boolean circuit decisions; negative exponents and zero-to-zero remain governed by bubbling and catchUndef. Machine-overflowing power ranges are explicitly declined rather than silently wrapped.

35 previously failing fixtures and all three new regressions now complete. One previously passing fixture, `conjure/function/function_total_bool_06`, exceeded 600 seconds in this run; this does not establish a semantic regression. All 427 fixtures that had SAT enabled before the first survey still pass. Profile changes and backend fixes both contribute to these results; this is not a controlled performance comparison.

| Outcome | First survey (664) | Latest survey (667) |
| --- | ---: | ---: |
| frontend | 2 | 2 |
| memory_limit | 39 | 31 |
| panic | 2 | 2 |
| pass | 565 | 602 |
| reference_memory_limit | 2 | 2 |
| residual_constraints | 28 | 12 |
| sat_loading_or_encoding | 4 | 0 |
| solution_mismatch | 3 | 0 |
| timeout | 19 | 16 |

## Validation and reference issue

Workspace library tests and doctests passed after the main fixes (508 library tests). After the flattened-occurrence follow-up, all 174 rule tests and `make check` pass. Normal golden verification passes for 22 focused SAT fixtures, the five passing occurrence follow-up fixtures, and 72 affected Minion/Z3 fixtures; the updated flattened-occurrence regression also passes a fresh Minion golden check. Successful complete fixtures have their accepted snapshots recorded; failed portfolio prefixes are counted only in this report. Original timing fields are retained for existing run identities.

The masked-power regression has 30 expected assignments, checked independently across 130 SAT portfolios. Conjure/Savile Row instead returns 24 because an unconditional native `pow` constraint excludes six negative-exponent assignments whose `catchUndef` fallback should be selected. The local, uncommitted report is `bug-reports/savilerow-masked-power/README.md`. That fixture explicitly skips Conjure validation. Oxide's default Minion backend also returns 24 solutions and emits the same unconditional-power restriction. This is a confirmed lowering bug in both Minion compilation paths, pending an Oxide fix; no new RustSAT/Pindakaas bug was established. The SAT backend returns all 30 solutions.

The remaining residual constraints are compound lexicographic comparisons and allDifferent input forms. Two earlier atLeast residuals now lower successfully but encounter the resource outcomes recorded below. Machine-overflowing power ranges remain explicitly unsupported.

## Follow-up after the survey (2026-10-05)

These changes postdate the tables below, which are kept as surveyed.

- **allDifferent lowering.** SAT allDifferent now accepts any matrix index domain and `flatten(...)` operands. All 11 allDifferent residuals now lower: `nqueens-4`, `n_queens1`, `n_queens2`, `n_queens_new`, `nqueens-8`, `quasiGroup4Idempotent`, `blackhole`, `knights` and `sportsScheduling`/`2`/`3` pass.
- **Compact SAT for large savilerow fixtures.** 36 savilerow fixtures that timed out, exceeded memory or took at least 60 seconds with `heuristic = "x"` now use `heuristic = "c"` for SAT, and `rule-trace = "aggregate"`. Under `x`, rewriting dominated: sportsScheduling spent 484 s rewriting and 52 s solving across 420 SAT runs. With compact, 31 pass, including these former resource failures: `absBug`, `diet`, `efpa`, `golomb`, `golomb2`, `knapsack`, `langfordN`, `magicSquare`, `molnars`, `multiDimensionArray`, `opd`, `plotting`, `quasiGroup3NonIdempotent`, `semigroup`, `solitaire_battleship` and `test-branchingon2`. Still failing: `carSequencing`, `pegSolitaireTable`, `magicSequence` (over 4 GiB) and `lee-distance` (600 s timeout); these keep full rule traces. `grocery` is globally skipped.
- **Rule-trace cost.** Full rule traces cost about a third of integration run time (`nurse`: 20.2 s full, 13.8 s aggregate, 13.5 s untraced). Fixtures can now choose `rule-trace = "aggregate"`, which records the same per-rule counts in `stats.toml` without trace files.

## First failures, smallest sources first

| Fixture | Outcome | Reason |
| --- | --- | --- |
| `conjure/function/function_partial_smoke` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/partition/partition_02` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/partition/partition_03` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/function/function_total_bool_06` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `conjure/relation/binrel02` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `conjure/permutation/perm_repr_0010` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `conjure/permutation/perm_repr_0011` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/relation/binrel03` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `conjure/function/function_total_int_03` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/function/function_total_int_04` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/function/function_total_int_05` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/function/function_total_int_06` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/partition/partition_01` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/relation/binrel01` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/absBug` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/relation/binrel04` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/function/function_total_int_set_01` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `basic/comprehension/dependent-domains` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/relation/relation07_connex` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `conjure/permutation/permutation_as_function_smoke` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `conjure/set/set08` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/relation/relation04_find` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `basic/lettings/04-domain` | panic | Domain should be ground |
| `conjure/function/function_complex_01` | residual_constraints | Un-encoded constraints in the model: [or([__0,__52,and([__53,([x#as_relation_1#as_set_1#explicit_1#components_1#components_1#explicit_1#components_1,x#as_relation_1#as_set_1#explicit_1#components_1#components_1#explicit_1#components_2;int(1..)] <lex [x#as_relation_1#as_set_1#explicit_1#components_2#components_1#explicit_1#components_1,x#as_relation_1#as_set_1#explicit_1#components_2#components_1#explicit_1#components_2;int(1..)]);int(1..)]),__118;int(1..)]), or([__1,__62,and([__63,([x#as_relatio |
| `savilerow/magicSequence` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `conjure/relation/relation04_param` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/semigroup` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/shorttable-smalltest` | frontend | Error: Custom { kind: Other, error: "test_dir=tests/integration/savilerow/shorttable-smalltest, model=input.essence, parser=tree-sitter, rewriter=optimised, comprehension_expander=auto, heuristic=x, seed=0, channelling=uniform, solver=sat, solver_seed=0: tests/integration/savilerow/shorttable-smalltest/input.essence:10:1:\n  \|\n10 \| tableshort([x[1],x[2],x[3],x[4]], mycon)\n  \| ^\nThe identifier 'tableshort' is not defined" } |
| `savilerow/const_matrix_test` | panic | error: 0 is not a valid index for dimension 0 |
| `savilerow/shorttable-assigntest` | frontend | Error: Custom { kind: Other, error: "test_dir=tests/integration/savilerow/shorttable-assigntest, model=input.essence, parser=tree-sitter, rewriter=optimised, comprehension_expander=auto, heuristic=x, seed=0, channelling=uniform, solver=sat, solver_seed=0: tests/integration/savilerow/shorttable-assigntest/input.essence:10:1:\n  \|\n10 \| tableshort([x[1],x[2],x[3],x[4],x[5]], mycon),\n  \| ^\nThe identifier 'tableshort' is not defined" } |
| `savilerow/multiDimensionArray` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `basic/function/sparse-partial` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/n_queens_new` | residual_constraints | Un-encoded constraints in the model: [allDifferent([SATInt(Offset, [q1#components_1#int_offset_1,q1#components_1#int_offset_2,q1#components_1#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [q1#components_2#int_offset_1,q1#components_2#int_offset_2,q1#components_2#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [q1#components_3#int_offset_1,q1#components_3#int_offset_2,q1#components_3#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [q1#components_4#int_offset_1,q1#components_4#int_offset_2,q1#componen |
| `savilerow/n_queens2` | residual_constraints | Un-encoded constraints in the model: [allDifferent([SATInt(Direct, [queens#components_1#int_direct_1,queens#components_1#int_direct_2,queens#components_1#int_direct_3,queens#components_1#int_direct_4,queens#components_1#int_direct_5,queens#components_1#int_direct_6,queens#components_1#int_direct_7,queens#components_1#int_direct_8;int(1..)] [0, 7]),SATInt(Direct, [queens#components_2#int_direct_1,queens#components_2#int_direct_2,queens#components_2#int_direct_3,queens#components_2#int_direct_4,qu |
| `savilerow/nqueens-8` | residual_constraints | Un-encoded constraints in the model: [allDifferent([SATInt(Offset, [q1#components_1#int_offset_1,q1#components_1#int_offset_2,q1#components_1#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [q1#components_2#int_offset_1,q1#components_2#int_offset_2,q1#components_2#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [q1#components_3#int_offset_1,q1#components_3#int_offset_2,q1#components_3#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [q1#components_4#int_offset_1,q1#components_4#int_offset_2,q1#componen |
| `savilerow/golomb` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/golomb2` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `dominance/rfc_example_future` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/knapsack` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/langfordN` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `savilerow/lee-distance` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `dominance/rfc_example` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/grocery` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/opd` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `savilerow/efpa` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `eprime-minion/nqueens-4` | residual_constraints | Un-encoded constraints in the model: [allDifferent([SATInt(Offset, [row#components_1#int_offset_1,row#components_1#int_offset_2,row#components_1#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [row#components_2#int_offset_1,row#components_2#int_offset_2,row#components_2#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [row#components_3#int_offset_1,row#components_3#int_offset_2,row#components_3#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [row#components_4#int_offset_1,row#components_4#int_offset_2, |
| `savilerow/carSequencing` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/magicSquare` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `savilerow/n_queens1` | residual_constraints | Un-encoded constraints in the model: [allDifferent([SATInt(Offset, [q1#components_1#int_offset_1,q1#components_1#int_offset_2,q1#components_1#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [q1#components_2#int_offset_1,q1#components_2#int_offset_2,q1#components_2#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [q1#components_3#int_offset_1,q1#components_3#int_offset_2,q1#components_3#int_offset_3;int(1..)] [0, 7]),SATInt(Offset, [q1#components_4#int_offset_1,q1#components_4#int_offset_2,q1#componen |
| `savilerow/diet` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `savilerow/quasiGroup3NonIdempotent` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/quasiGroup4Idempotent` | residual_constraints | Un-encoded constraints in the model: [allDifferent([SATInt(Offset, [qgDiagonal#components_1#int_offset_1,qgDiagonal#components_1#int_offset_2,qgDiagonal#components_1#int_offset_3;int(1..)] [0, 6]),SATInt(Offset, [qgDiagonal#components_2#int_offset_1,qgDiagonal#components_2#int_offset_2,qgDiagonal#components_2#int_offset_3;int(1..)] [0, 6]),SATInt(Offset, [qgDiagonal#components_3#int_offset_1,qgDiagonal#components_3#int_offset_2,qgDiagonal#components_3#int_offset_3;int(1..)] [0, 6]),SATInt(Offset |
| `savilerow/knights` | residual_constraints | Un-encoded constraints in the model: [allDifferent([SATInt(Direct, [tour#components_1#int_direct_1,tour#components_1#int_direct_2,tour#components_1#int_direct_3,tour#components_1#int_direct_4,tour#components_1#int_direct_5,tour#components_1#int_direct_6,tour#components_1#int_direct_7,tour#components_1#int_direct_8,tour#components_1#int_direct_9,tour#components_1#int_direct_10,tour#components_1#int_direct_11,tour#components_1#int_direct_12,tour#components_1#int_direct_13,tour#components_1#int_dir |
| `savilerow/sportsScheduling` | residual_constraints | Un-encoded constraints in the model: [allDifferent(flatten([SATInt(Direct, [schedule#components_1#int_direct_1,schedule#components_1#int_direct_2,schedule#components_1#int_direct_3,schedule#components_1#int_direct_4,schedule#components_1#int_direct_5,schedule#components_1#int_direct_6,schedule#components_1#int_direct_7,schedule#components_1#int_direct_8;int(1..)] [1, 8]),SATInt(Direct, [schedule#components_2#int_direct_1,schedule#components_2#int_direct_2,schedule#components_2#int_direct_3,sched |
| `savilerow/sportsScheduling3` | residual_constraints | Un-encoded constraints in the model: [allDifferent(flatten([SATInt(Direct, [schedule#components_1#int_direct_1,schedule#components_1#int_direct_2,schedule#components_1#int_direct_3,schedule#components_1#int_direct_4,schedule#components_1#int_direct_5,schedule#components_1#int_direct_6,schedule#components_1#int_direct_7,schedule#components_1#int_direct_8;int(1..)] [1, 8]),SATInt(Direct, [schedule#components_2#int_direct_1,schedule#components_2#int_direct_2,schedule#components_2#int_direct_3,sched |
| `savilerow/sportsScheduling2` | residual_constraints | Un-encoded constraints in the model: [allDifferent(flatten([SATInt(Direct, [schedule#components_1#int_direct_1,schedule#components_1#int_direct_2,schedule#components_1#int_direct_3,schedule#components_1#int_direct_4,schedule#components_1#int_direct_5,schedule#components_1#int_direct_6,schedule#components_1#int_direct_7,schedule#components_1#int_direct_8;int(1..)] [1, 8]),SATInt(Direct, [schedule#components_2#int_direct_1,schedule#components_2#int_direct_2,schedule#components_2#int_direct_3,sched |
| `savilerow/molnars` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `mildly-interesting/gchq-2016` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `savilerow/pegSolitaireAction` | reference_memory_limit | Conjure reference did not complete; fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/pegSolitaireState` | reference_memory_limit | Conjure reference did not complete; fixture exceeded 4 GiB RSS; unsupported semantics not established |
| `savilerow/solitaire_battleship` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `savilerow/blackhole` | residual_constraints | Un-encoded constraints in the model: [allDifferent([SATInt(Offset, [cardSequence#components_1#int_offset_1,cardSequence#components_1#int_offset_2,cardSequence#components_1#int_offset_3,cardSequence#components_1#int_offset_4,cardSequence#components_1#int_offset_5,cardSequence#components_1#int_offset_6;int(1..)] [0, 51]),SATInt(Offset, [cardSequence#components_2#int_offset_1,cardSequence#components_2#int_offset_2,cardSequence#components_2#int_offset_3,cardSequence#components_2#int_offset_4,cardSeq |
| `savilerow/plotting` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `savilerow/test-branchingon2` | timeout | fixture exceeded 600 seconds; unsupported semantics not established |
| `savilerow/pegSolitaireTable` | memory_limit | fixture exceeded 4 GiB RSS; unsupported semantics not established |

The exhaustive per-fixture record is [sat_full_coverage_survey.csv](sat_full_coverage_survey.csv). Raw logs, diagnostics and the last failing trace remain under `target/sat-coverage-fixes/`; they are local survey artefacts. Generated files and failed-fixture prefix snapshots are discarded after completion to bound disk usage, retaining their validated counts in the survey records. These fixes are in Oxide; no new RustSAT/Pindakaas defect was established.
