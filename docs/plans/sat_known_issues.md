# SAT backend: known issues and disabled tests

Inventory date: 2026-10-03, branch `sat-ir` after constant-row table decisions were connected.

343 of 626 runnable integration fixtures have SAT enabled. All 343 passed full acceptance, exercising 12,473 SAT portfolios. The earlier integer relation migration also passed full normal golden verification. The remaining 283 fixtures have SAT disabled. Disabled does not establish that a fixture still fails on today's code: the exhaustive screen predates the weighted PB and signed arithmetic changes.

This list filters the [coverage CSV](sat_coverage_survey.csv) against current test configurations, excluding cases that have since been enabled. The CSV records observations rather than independently diagnosed root causes. No remaining solution mismatch is recorded in that survey; the three former mismatches have been fixed and enabled.

Scalar integer/Boolean allDifferent now passes pairwise and eligible value-AMO portfolios. allDifferentExcept and compound-valued operands remain gaps; the dedicated fixture also covers sparse domains, repeated operands, constants, negation and reification. No new upstream library bug was confirmed.

Constant-row positive/negative tables now pass tuple and MDD portfolios, including reification. Short tables, binary-support encodings and non-constant row relations remain gaps. A fresh compact screen of `savilerow/sportsScheduling2` lowers its tables but leaves `flatten` inside allDifferent; that fixture remains disabled.

## Summary of last recorded outcomes

| Outcome | Fixtures | Evidence |
| --- | ---: | --- |
| Residual constraints in initial screen | 206 | Compact/first CLI screen failed to finish lowering. |
| Residual constraints in full uniform portfolio | 29 | Initial screen passed, but another representation/portfolio failed. |
| Panics | 3 | Two reconfirmed today; one historical full-portfolio failure needs retesting. |
| Model-loading errors | 5 | All five reconfirmed today. |
| Initial screen timeouts | 37 | Eight-second compilation / twelve-second solve-process limits. |
| Full uniform portfolio timeouts | 3 | 120-second per-fixture limit. |
| Total SAT-disabled runnable fixtures | 283 | Current configurations matched to survey records. |

Timeouts are performance observations under those limits, not proof of unsupported semantics. The 235 residual-constraint cases were not all rerun after the PB changes. Residual constraints identify incomplete lowering; they do not by themselves identify the missing rule or representation.

## Concrete crashes and loading errors

The current release binary was checked with the tree-sitter parser, uniform channelling, compact and first heuristics, and one requested solution. Checks left fixture configurations and goldens unchanged.

| Fixture | Observation | Current status |
| --- | --- | --- |
| `basic/lettings/04-domain` | SAT adaptor panics: `Domain should be ground`. | Reconfirmed with compact and first. |
| `savilerow/const_matrix_test` | Indexing panics: `0 is not a valid index for dimension 0`. | Reconfirmed with compact and first; not established as exclusive to SAT. |
| `basic/abs/03-nested` | Historical Direct division lookup panic: index underflow during nested arithmetic. | Compact and first now succeed; full uniform portfolio still needs a retest. Do not count this as a reconfirmed crash. |
| `smt/matrix/2d-eq` | `Non-Boolean SAT reference: m`. | Reconfirmed with compact and first. |
| `smt/matrix/overlapping-eq` | `Non-Boolean SAT reference: a`. | Reconfirmed with compact and first. |
| `smt/matrix/overlapping-neq` | `Non-Boolean SAT reference: a`. | Reconfirmed with compact and first. |
| `smt/matrix/overlapping-neq-wrapped` | `Non-Boolean SAT reference: a`. | Reconfirmed with compact and first. |
| `smt/matrix/scalar-neq` | `Non-Boolean SAT reference: m`. | Reconfirmed with compact and first. |

The five matrix cases compile but fail when the SAT adaptor loads the terminal model. This points to matrix equality/disequality lowering leaving composite references where Boolean operands are required; it does not mean all matrix operations are unsupported.

Example reproduction from the repository root:

```sh
target/release/conjure-oxide solve --solver=sat --channelling=uniform \
  --heuristic=c --parser=tree-sitter --number-of-solutions=1 \
  test-suite/tests/integration/smt/matrix/2d-eq/input.essence
```

## Residual constraints: full-portfolio failures (29)

These passed the initial screen but failed a different uniform portfolio. They include partial/total functions, tuple and record operations, multisets, sequences, sets and matrices containing sets. These are observed fixture families, not a blanket claim that every operation of those types fails.

- [basic/function/apply-partial](../../test-suite/tests/integration/basic/function/apply-partial/input.essence)
- [basic/function/apply-partial-out-of-domain](../../test-suite/tests/integration/basic/function/apply-partial-out-of-domain/input.essence)
- [basic/tuples/01-bool-int](../../test-suite/tests/integration/basic/tuples/01-bool-int/input.essence)
- [basic/tuples/03-equality](../../test-suite/tests/integration/basic/tuples/03-equality/input.essence)
- [basic/tuples/04-inequality](../../test-suite/tests/integration/basic/tuples/04-inequality/input.essence)
- [conjure/function/function_total_bool_01](../../test-suite/tests/integration/conjure/function/function_total_bool_01/input.essence)
- [conjure/function/function_total_bool_02](../../test-suite/tests/integration/conjure/function/function_total_bool_02/input.essence)
- [conjure/function/function_total_bool_smoke](../../test-suite/tests/integration/conjure/function/function_total_bool_smoke/input.essence)
- [conjure/function/function_total_int_01](../../test-suite/tests/integration/conjure/function/function_total_int_01/input.essence)
- [conjure/function/function_total_int_02](../../test-suite/tests/integration/conjure/function/function_total_int_02/input.essence)
- [conjure/function/function_total_record_01_find](../../test-suite/tests/integration/conjure/function/function_total_record_01_find/input.essence)
- [conjure/function/function_total_tuple_01_find](../../test-suite/tests/integration/conjure/function/function_total_tuple_01_find/input.essence)
- [conjure/matrix/matrix_of_set_01](../../test-suite/tests/integration/conjure/matrix/matrix_of_set_01/input.essence)
- [conjure/mset/mset01_find](../../test-suite/tests/integration/conjure/mset/mset01_find/input.essence)
- [conjure/mset/mset01_param_p1](../../test-suite/tests/integration/conjure/mset/mset01_param_p1/input.essence)
- [conjure/mset/mset01_param_p4](../../test-suite/tests/integration/conjure/mset/mset01_param_p4/input.essence)
- [conjure/mset/mset01_param_p7](../../test-suite/tests/integration/conjure/mset/mset01_param_p7/input.essence)
- [conjure/mset/mset03_1](../../test-suite/tests/integration/conjure/mset/mset03_1/input.essence)
- [conjure/mset/mset03_2](../../test-suite/tests/integration/conjure/mset/mset03_2/input.essence)
- [conjure/record/record00](../../test-suite/tests/integration/conjure/record/record00/input.essence)
- [conjure/sequence/sequence04](../../test-suite/tests/integration/conjure/sequence/sequence04/input.essence)
- [conjure/sequence/sequence05](../../test-suite/tests/integration/conjure/sequence/sequence05/input.essence)
- [conjure/sequence/sequence_injective_variable](../../test-suite/tests/integration/conjure/sequence/sequence_injective_variable/input.essence)
- [conjure/sequence/sequence_subseq_dups](../../test-suite/tests/integration/conjure/sequence/sequence_subseq_dups/input.essence)
- [conjure/sequence/sequence_subseq_nodups](../../test-suite/tests/integration/conjure/sequence/sequence_subseq_nodups/input.essence)
- [conjure/sequence/sequence_substr](../../test-suite/tests/integration/conjure/sequence/sequence_substr/input.essence)
- [conjure/set/set_card_01](../../test-suite/tests/integration/conjure/set/set_card_01/input.essence)
- [conjure/tuple/tuple_packed_nested_tuple_smoke](../../test-suite/tests/integration/conjure/tuple/tuple_packed_nested_tuple_smoke/input.essence)
- [sets/intersect2](../../test-suite/tests/integration/sets/intersect2/input.essence)

## Full uniform portfolio timeouts (3)

Last tested with a 120-second per-fixture bound.

- [basic/comprehension/dependent-domains](../../test-suite/tests/integration/basic/comprehension/dependent-domains/input.essence)
- [hakank-eprime/xkcd](../../test-suite/tests/integration/hakank-eprime/xkcd/xkcd.essence)
- [savilerow/absBug](../../test-suite/tests/integration/savilerow/absBug/input.essence)

## Initial-screen residual constraints (206)

Last recorded in the compact/first screen, before the weighted PB changes. Each fixture below remains SAT-disabled; a fresh screen may move cases out of this list.

- [antichain](../../test-suite/tests/integration/antichain/antichain.essence)
- [basic/comprehension/set-bounded-image-member](../../test-suite/tests/integration/basic/comprehension/set-bounded-image-member/input.essence)
- [basic/comprehension/set-bounded-size-member](../../test-suite/tests/integration/basic/comprehension/set-bounded-size-member/input.essence)
- [basic/comprehension/set-fixed-size-member](../../test-suite/tests/integration/basic/comprehension/set-fixed-size-member/input.essence)
- [basic/comprehension/set-tuple-member](../../test-suite/tests/integration/basic/comprehension/set-tuple-member/input.essence)
- [basic/exists/simple/04](../../test-suite/tests/integration/basic/exists/simple/04/input.essence)
- [basic/finite-givens/set03](../../test-suite/tests/integration/basic/finite-givens/set03/finite-givens-set03.essence)
- [basic/finite-givens/set05](../../test-suite/tests/integration/basic/finite-givens/set05/finite-givens-set05.essence)
- [basic/function/apply-in-domain](../../test-suite/tests/integration/basic/function/apply-in-domain/input.essence)
- [basic/function/apply-out-of-domain](../../test-suite/tests/integration/basic/function/apply-out-of-domain/input.essence)
- [basic/function/sparse-partial](../../test-suite/tests/integration/basic/function/sparse-partial/input.essence)
- [basic/lex/long-leq-short](../../test-suite/tests/integration/basic/lex/long-leq-short/input.essence)
- [basic/lex/long-lt-short](../../test-suite/tests/integration/basic/lex/long-lt-short/input.essence)
- [basic/lex/short-leq-long](../../test-suite/tests/integration/basic/lex/short-leq-long/input.essence)
- [basic/lex/short-lt-long](../../test-suite/tests/integration/basic/lex/short-lt-long/input.essence)
- [basic/matrix/02-2d-slicing](../../test-suite/tests/integration/basic/matrix/02-2d-slicing/input.essence)
- [basic/matrix/03-domain-letting](../../test-suite/tests/integration/basic/matrix/03-domain-letting/input.essence)
- [basic/matrix/08-index-is-expr](../../test-suite/tests/integration/basic/matrix/08-index-is-expr/input.essence)
- [basic/matrix/09-index-is-expr-offset](../../test-suite/tests/integration/basic/matrix/09-index-is-expr-offset/input.essence)
- [basic/matrix/10-value-letting-index-is-expr](../../test-suite/tests/integration/basic/matrix/10-value-letting-index-is-expr/input.essence)
- [basic/matrix/11-index-matrix-literal](../../test-suite/tests/integration/basic/matrix/11-index-matrix-literal/input.essence)
- [basic/matrix/13-index-matrix-literal-2d](../../test-suite/tests/integration/basic/matrix/13-index-matrix-literal-2d/input.essence)
- [basic/matrix/14-matrix-index-matrix](../../test-suite/tests/integration/basic/matrix/14-matrix-index-matrix/matrix-index-matrix.essence)
- [basic/matrix/15-matrix-index-matrix-with-offset](../../test-suite/tests/integration/basic/matrix/15-matrix-index-matrix-with-offset/matrix-index-matrix.essence)
- [basic/matrix/16-matrix-out-bounds](../../test-suite/tests/integration/basic/matrix/16-matrix-out-bounds/input.essence)
- [basic/matrix/17-matrix-out-bounds-reify](../../test-suite/tests/integration/basic/matrix/17-matrix-out-bounds-reify/input.essence)
- [basic/matrix/18-matrix-out-bounds-possibly-undef](../../test-suite/tests/integration/basic/matrix/18-matrix-out-bounds-possibly-undef/input.essence)
- [basic/mod/01](../../test-suite/tests/integration/basic/mod/01/input.essence)
- [basic/mod/03](../../test-suite/tests/integration/basic/mod/03/input.essence)
- [basic/mod/04](../../test-suite/tests/integration/basic/mod/04/input.essence)
- [basic/mod/05](../../test-suite/tests/integration/basic/mod/05/mod-05.essence)
- [basic/mod/06](../../test-suite/tests/integration/basic/mod/06/mod-06.essence)
- [basic/pow/01-simple](../../test-suite/tests/integration/basic/pow/01-simple/input.essence)
- [basic/pow/02-exponent-zero](../../test-suite/tests/integration/basic/pow/02-exponent-zero/input.essence)
- [basic/pow/03-negative-exponent](../../test-suite/tests/integration/basic/pow/03-negative-exponent/input.essence)
- [basic/pow/04-flatten](../../test-suite/tests/integration/basic/pow/04-flatten/input.essence)
- [basic/pow/05-negative-base](../../test-suite/tests/integration/basic/pow/05-negative-base/input.essence)
- [basic/sequence/apply-out-of-range](../../test-suite/tests/integration/basic/sequence/apply-out-of-range/input.essence)
- [basic/toInt/01](../../test-suite/tests/integration/basic/toInt/01/input.essence)
- [basic/toInt/02-flatten](../../test-suite/tests/integration/basic/toInt/02-flatten/input.essence)
- [bugs/treemorph-misses-node-01](../../test-suite/tests/integration/bugs/treemorph-misses-node-01/input.essence)
- [conjure/all_diff/all_diff_except_comprehension_smoke](../../test-suite/tests/integration/conjure/all_diff/all_diff_except_comprehension_smoke/input.essence)
- [conjure/function/function_complex_01](../../test-suite/tests/integration/conjure/function/function_complex_01/input.essence)
- [conjure/function/function_partial_int_set_01](../../test-suite/tests/integration/conjure/function/function_partial_int_set_01/input.essence)
- [conjure/function/function_record_injective_attr](../../test-suite/tests/integration/conjure/function/function_record_injective_attr/input.essence)
- [conjure/function/function_total_bool_03](../../test-suite/tests/integration/conjure/function/function_total_bool_03/input.essence)
- [conjure/function/function_total_bool_04](../../test-suite/tests/integration/conjure/function/function_total_bool_04/input.essence)
- [conjure/function/function_total_bool_05](../../test-suite/tests/integration/conjure/function/function_total_bool_05/input.essence)
- [conjure/function/function_total_bool_06](../../test-suite/tests/integration/conjure/function/function_total_bool_06/input.essence)
- [conjure/function/function_total_int_03](../../test-suite/tests/integration/conjure/function/function_total_int_03/input.essence)
- [conjure/function/function_total_int_04](../../test-suite/tests/integration/conjure/function/function_total_int_04/input.essence)
- [conjure/function/function_total_int_05](../../test-suite/tests/integration/conjure/function/function_total_int_05/input.essence)
- [conjure/function/function_total_int_06](../../test-suite/tests/integration/conjure/function/function_total_int_06/input.essence)
- [conjure/function/function_total_int_set_01](../../test-suite/tests/integration/conjure/function/function_total_int_set_01/input.essence)
- [conjure/function/function_tuple_injective_attr](../../test-suite/tests/integration/conjure/function/function_tuple_injective_attr/input.essence)
- [conjure/matrix/matrix_atmost_atleast](../../test-suite/tests/integration/conjure/matrix/matrix_atmost_atleast/input.essence)
- [conjure/mset/mset02](../../test-suite/tests/integration/conjure/mset/mset02/input.essence)
- [conjure/mset/mset04](../../test-suite/tests/integration/conjure/mset/mset04/input.essence)
- [conjure/mset/mset05](../../test-suite/tests/integration/conjure/mset/mset05/input.essence)
- [conjure/mset/mset06_1](../../test-suite/tests/integration/conjure/mset/mset06_1/input.essence)
- [conjure/mset/mset06_2](../../test-suite/tests/integration/conjure/mset/mset06_2/input.essence)
- [conjure/mset/mset07](../../test-suite/tests/integration/conjure/mset/mset07/input.essence)
- [conjure/partition/partition_01](../../test-suite/tests/integration/conjure/partition/partition_01/input.essence)
- [conjure/partition/partition_02](../../test-suite/tests/integration/conjure/partition/partition_02/input.essence)
- [conjure/partition/partition_03](../../test-suite/tests/integration/conjure/partition/partition_03/input.essence)
- [conjure/permutation/perm_repr_0010](../../test-suite/tests/integration/conjure/permutation/perm_repr_0010/input.essence)
- [conjure/permutation/perm_repr_0011](../../test-suite/tests/integration/conjure/permutation/perm_repr_0011/input.essence)
- [conjure/permutation/permutation_as_function_smoke](../../test-suite/tests/integration/conjure/permutation/permutation_as_function_smoke/input.essence)
- [conjure/primitive/int_param_domain_04](../../test-suite/tests/integration/conjure/primitive/int_param_domain_04/input.essence)
- [conjure/record/record01](../../test-suite/tests/integration/conjure/record/record01/input.essence)
- [conjure/relation/binrel01](../../test-suite/tests/integration/conjure/relation/binrel01/input.essence)
- [conjure/relation/binrel04](../../test-suite/tests/integration/conjure/relation/binrel04/input.essence)
- [conjure/relation/reflexive_rel](../../test-suite/tests/integration/conjure/relation/reflexive_rel/input.essence)
- [conjure/relation/relation01](../../test-suite/tests/integration/conjure/relation/relation01/input.essence)
- [conjure/relation/relation02](../../test-suite/tests/integration/conjure/relation/relation02/input.essence)
- [conjure/relation/relation03_2](../../test-suite/tests/integration/conjure/relation/relation03_2/input.essence)
- [conjure/relation/relation05_set_fixed_direct](../../test-suite/tests/integration/conjure/relation/relation05_set_fixed_direct/input.essence)
- [conjure/relation/relation05_set_fixed_setty](../../test-suite/tests/integration/conjure/relation/relation05_set_fixed_setty/input.essence)
- [conjure/relation/relation06_set_bounded_direct](../../test-suite/tests/integration/conjure/relation/relation06_set_bounded_direct/input.essence)
- [conjure/relation/relation06_set_bounded_setty](../../test-suite/tests/integration/conjure/relation/relation06_set_bounded_setty/input.essence)
- [conjure/sequence/sequence_bijective](../../test-suite/tests/integration/conjure/sequence/sequence_bijective/input.essence)
- [conjure/sequence/sequence_injective_fixed](../../test-suite/tests/integration/conjure/sequence/sequence_injective_fixed/input.essence)
- [conjure/sequence/sequence_surjective](../../test-suite/tests/integration/conjure/sequence/sequence_surjective/input.essence)
- [conjure/set/cut_01_off](../../test-suite/tests/integration/conjure/set/cut_01_off/input.essence)
- [conjure/set/finiteGivens_set03](../../test-suite/tests/integration/conjure/set/finiteGivens_set03/input.essence)
- [conjure/set/finiteGivens_set05_p1](../../test-suite/tests/integration/conjure/set/finiteGivens_set05_p1/input.essence)
- [conjure/set/finiteGivens_set05_p2](../../test-suite/tests/integration/conjure/set/finiteGivens_set05_p2/input.essence)
- [conjure/set/set01_1](../../test-suite/tests/integration/conjure/set/set01_1/input.essence)
- [conjure/set/set01_2](../../test-suite/tests/integration/conjure/set/set01_2/input.essence)
- [conjure/set/set01_3](../../test-suite/tests/integration/conjure/set/set01_3/input.essence)
- [conjure/set/set02](../../test-suite/tests/integration/conjure/set/set02/input.essence)
- [conjure/set/set03](../../test-suite/tests/integration/conjure/set/set03/input.essence)
- [conjure/set/set04](../../test-suite/tests/integration/conjure/set/set04/input.essence)
- [conjure/set/set05](../../test-suite/tests/integration/conjure/set/set05/input.essence)
- [conjure/set/set06](../../test-suite/tests/integration/conjure/set/set06/input.essence)
- [conjure/set/set07](../../test-suite/tests/integration/conjure/set/set07/input.essence)
- [conjure/set/set08](../../test-suite/tests/integration/conjure/set/set08/input.essence)
- [conjure/set/set09](../../test-suite/tests/integration/conjure/set/set09/input.essence)
- [conjure/set/setOfSet01](../../test-suite/tests/integration/conjure/set/setOfSet01/input.essence)
- [conjure/set/setOfSet02](../../test-suite/tests/integration/conjure/set/setOfSet02/input.essence)
- [conjure/set/setOfSet03](../../test-suite/tests/integration/conjure/set/setOfSet03/input.essence)
- [conjure/set/setOfSet04](../../test-suite/tests/integration/conjure/set/setOfSet04/input.essence)
- [conjure/set/set_card_00](../../test-suite/tests/integration/conjure/set/set_card_00/input.essence)
- [conjure/set/set_card_02](../../test-suite/tests/integration/conjure/set/set_card_02/input.essence)
- [conjure/set/set_subseteq_constant_smoke](../../test-suite/tests/integration/conjure/set/set_subseteq_constant_smoke/input.essence)
- [conjure/tuple/tuple01_bool_int](../../test-suite/tests/integration/conjure/tuple/tuple01_bool_int/input.essence)
- [conjure/tuple/tuple02_nested](../../test-suite/tests/integration/conjure/tuple/tuple02_nested/input.essence)
- [conjure/variant/variant01](../../test-suite/tests/integration/conjure/variant/variant01/input.essence)
- [dominance/rfc_example](../../test-suite/tests/integration/dominance/rfc_example/input.essence)
- [dominance/rfc_example_future](../../test-suite/tests/integration/dominance/rfc_example_future/input.essence)
- [dominance/subset_pareto_01_future](../../test-suite/tests/integration/dominance/subset_pareto_01_future/input.essence)
- [eprime-minion/nqueens-4](../../test-suite/tests/integration/eprime-minion/nqueens-4/input.essence)
- [eprime-minion/partial-eval-03](../../test-suite/tests/integration/eprime-minion/partial-eval-03/input.essence)
- [hakank-eprime/quasigroup-completion/01](../../test-suite/tests/integration/hakank-eprime/quasigroup-completion/01/input.essence)
- [hakank-eprime/quasigroup-completion/02](../../test-suite/tests/integration/hakank-eprime/quasigroup-completion/02/input.essence)
- [mildly-interesting/subset-sum](../../test-suite/tests/integration/mildly-interesting/subset-sum/subsetSum.essence)
- [minion-constraints/modulo-undefzero-01-simple](../../test-suite/tests/integration/minion-constraints/modulo-undefzero-01-simple/input.essence)
- [minion-constraints/modulo-undefzero-02-zero](../../test-suite/tests/integration/minion-constraints/modulo-undefzero-02-zero/input.essence)
- [minion-constraints/modulo-undefzero-03-nested](../../test-suite/tests/integration/minion-constraints/modulo-undefzero-03-nested/input.essence)
- [minion-constraints/modulo-undefzero-04-nested-neq](../../test-suite/tests/integration/minion-constraints/modulo-undefzero-04-nested-neq/input.essence)
- [minion-constraints/modulo-undefzero-05-nested-noteq](../../test-suite/tests/integration/minion-constraints/modulo-undefzero-05-nested-noteq/input.essence)
- [optimisations/implies-tautologies-cse](../../test-suite/tests/integration/optimisations/implies-tautologies-cse/input.essence)
- [savilerow/alldiff_except](../../test-suite/tests/integration/savilerow/alldiff_except/input.essence)
- [savilerow/alldiff_except_nest](../../test-suite/tests/integration/savilerow/alldiff_except_nest/input.essence)
- [savilerow/atleast-test](../../test-suite/tests/integration/savilerow/atleast-test/input.essence)
- [savilerow/atmost-test](../../test-suite/tests/integration/savilerow/atmost-test/input.essence)
- [savilerow/bibd](../../test-suite/tests/integration/savilerow/bibd/input.essence)
- [savilerow/bibd-implied](../../test-suite/tests/integration/savilerow/bibd-implied/input.essence)
- [savilerow/bugVariableArray1](../../test-suite/tests/integration/savilerow/bugVariableArray1/input.essence)
- [savilerow/catchundef-saad1](../../test-suite/tests/integration/savilerow/catchundef-saad1/input.essence)
- [savilerow/discreteTomography](../../test-suite/tests/integration/savilerow/discreteTomography/input.essence)
- [savilerow/futoshiki](../../test-suite/tests/integration/savilerow/futoshiki/input.essence)
- [savilerow/gcctest](../../test-suite/tests/integration/savilerow/gcctest/input.essence)
- [savilerow/graphColouring](../../test-suite/tests/integration/savilerow/graphColouring/input.essence)
- [savilerow/killer](../../test-suite/tests/integration/savilerow/killer/input.essence)
- [savilerow/knapsack](../../test-suite/tests/integration/savilerow/knapsack/input.essence)
- [savilerow/knights](../../test-suite/tests/integration/savilerow/knights/input.essence)
- [savilerow/langford](../../test-suite/tests/integration/savilerow/langford/langford.essence)
- [savilerow/langfordN](../../test-suite/tests/integration/savilerow/langfordN/input.essence)
- [savilerow/magicSquare](../../test-suite/tests/integration/savilerow/magicSquare/input.essence)
- [savilerow/matrix-test](../../test-suite/tests/integration/savilerow/matrix-test/input.essence)
- [savilerow/molnars](../../test-suite/tests/integration/savilerow/molnars/input.essence)
- [savilerow/n_queens1](../../test-suite/tests/integration/savilerow/n_queens1/input.essence)
- [savilerow/n_queens2](../../test-suite/tests/integration/savilerow/n_queens2/input.essence)
- [savilerow/n_queens_new](../../test-suite/tests/integration/savilerow/n_queens_new/input.essence)
- [savilerow/nqueens-8](../../test-suite/tests/integration/savilerow/nqueens-8/nqueens.essence)
- [savilerow/nurse](../../test-suite/tests/integration/savilerow/nurse/input.essence)
- [savilerow/opd](../../test-suite/tests/integration/savilerow/opd/input.essence)
- [savilerow/opt-matrixderef](../../test-suite/tests/integration/savilerow/opt-matrixderef/input.essence)
- [savilerow/peaceableArmyOfQueens-old1](../../test-suite/tests/integration/savilerow/peaceableArmyOfQueens-old1/input.essence)
- [savilerow/peaceableArmyOfQueens-old3](../../test-suite/tests/integration/savilerow/peaceableArmyOfQueens-old3/input.essence)
- [savilerow/peaceableArmyOfQueens1](../../test-suite/tests/integration/savilerow/peaceableArmyOfQueens1/input.essence)
- [savilerow/peaceableArmyOfQueens2](../../test-suite/tests/integration/savilerow/peaceableArmyOfQueens2/input.essence)
- [savilerow/peaceableArmyOfQueens2-failing](../../test-suite/tests/integration/savilerow/peaceableArmyOfQueens2-failing/input.essence)
- [savilerow/peacefulArmyQueens3](../../test-suite/tests/integration/savilerow/peacefulArmyQueens3/input.essence)
- [savilerow/permMultElementId](../../test-suite/tests/integration/savilerow/permMultElementId/input.essence)
- [savilerow/problem110](../../test-suite/tests/integration/savilerow/problem110/input.essence)
- [savilerow/problem51](../../test-suite/tests/integration/savilerow/problem51/input.essence)
- [savilerow/quantification_over_matrix_doms_4](../../test-suite/tests/integration/savilerow/quantification_over_matrix_doms_4/input.essence)
- [savilerow/quasiGroup5NonIdempotent](../../test-suite/tests/integration/savilerow/quasiGroup5NonIdempotent/input.essence)
- [savilerow/sendMoreMoney](../../test-suite/tests/integration/savilerow/sendMoreMoney/input.essence)
- [savilerow/simpleElementId](../../test-suite/tests/integration/savilerow/simpleElementId/input.essence)
- [savilerow/sonet2](../../test-suite/tests/integration/savilerow/sonet2/input.essence)
- [savilerow/sonet_problem](../../test-suite/tests/integration/savilerow/sonet_problem/input.essence)
- [savilerow/sportsScheduling](../../test-suite/tests/integration/savilerow/sportsScheduling/input.essence)
- [savilerow/sportsScheduling2](../../test-suite/tests/integration/savilerow/sportsScheduling2/input.essence)
- [savilerow/sportsScheduling3](../../test-suite/tests/integration/savilerow/sportsScheduling3/input.essence)
- [savilerow/sudoku-comprehension](../../test-suite/tests/integration/savilerow/sudoku-comprehension/input.essence)
- [savilerow/sudoku3](../../test-suite/tests/integration/savilerow/sudoku3/input.essence)
- [savilerow/test-holey-matrix](../../test-suite/tests/integration/savilerow/test-holey-matrix/input.essence)
- [savilerow/test-power](../../test-suite/tests/integration/savilerow/test-power/input.essence)
- [savilerow/test_comprehension_functions](../../test-suite/tests/integration/savilerow/test_comprehension_functions/input.essence)
- [savilerow/test_element_simplify](../../test-suite/tests/integration/savilerow/test_element_simplify/input.essence)
- [savilerow/test_power_raw](../../test-suite/tests/integration/savilerow/test_power_raw/input.essence)
- [savilerow/tictactoe](../../test-suite/tests/integration/savilerow/tictactoe/input.essence)
- [savilerow/valsymElementId](../../test-suite/tests/integration/savilerow/valsymElementId/input.essence)
- [savilerow/varsymElementId](../../test-suite/tests/integration/savilerow/varsymElementId/input.essence)
- [sets/MinMax](../../test-suite/tests/integration/sets/MinMax/input.essence)
- [sets/constant-eval-set-tests/Intersect](../../test-suite/tests/integration/sets/constant-eval-set-tests/Intersect/input.essence)
- [sets/constant-eval-set-tests/Union](../../test-suite/tests/integration/sets/constant-eval-set-tests/Union/input.essence)
- [sets/difference](../../test-suite/tests/integration/sets/difference/input.essence)
- [sets/equals2](../../test-suite/tests/integration/sets/equals2/input.essence)
- [sets/explicit-large-inner-domain](../../test-suite/tests/integration/sets/explicit-large-inner-domain/input.essence)
- [sets/in](../../test-suite/tests/integration/sets/in/input.essence)
- [sets/neq-marker](../../test-suite/tests/integration/sets/neq-marker/input.essence)
- [sets/occurrence-large-inner-domain](../../test-suite/tests/integration/sets/occurrence-large-inner-domain/input.essence)
- [sets/occurrence-set-of-set-membership](../../test-suite/tests/integration/sets/occurrence-set-of-set-membership/input.essence)
- [sets/strict-subset-marker](../../test-suite/tests/integration/sets/strict-subset-marker/input.essence)
- [sets/subsetEq2](../../test-suite/tests/integration/sets/subsetEq2/input.essence)
- [sets/subseteq-marker-marker](../../test-suite/tests/integration/sets/subseteq-marker-marker/input.essence)
- [sets/union-comprehension](../../test-suite/tests/integration/sets/union-comprehension/input.essence)
- [sets/union2](../../test-suite/tests/integration/sets/union2/input.essence)
- [smt/int/to-int](../../test-suite/tests/integration/smt/int/to-int/input.essence)
- [smt/matrix/bibd](../../test-suite/tests/integration/smt/matrix/bibd/input.essence)
- [smt/matrix/flatten-no-depth](../../test-suite/tests/integration/smt/matrix/flatten-no-depth/input.essence)
- [smt/matrix/lex-eq](../../test-suite/tests/integration/smt/matrix/lex-eq/input.essence)
- [smt/matrix/lex-geq](../../test-suite/tests/integration/smt/matrix/lex-geq/input.essence)
- [smt/matrix/lex-gt](../../test-suite/tests/integration/smt/matrix/lex-gt/input.essence)
- [smt/matrix/lex-leq](../../test-suite/tests/integration/smt/matrix/lex-leq/input.essence)
- [smt/matrix/lex-lt](../../test-suite/tests/integration/smt/matrix/lex-lt/input.essence)
- [smt/matrix/magic-square-flatten](../../test-suite/tests/integration/smt/matrix/magic-square-flatten/input.essence)
- [smt/matrix/nqueens-4](../../test-suite/tests/integration/smt/matrix/nqueens-4/input.essence)
- [smt/matrix/undefined-index-var](../../test-suite/tests/integration/smt/matrix/undefined-index-var/input.essence)
- [smt/set/eq](../../test-suite/tests/integration/smt/set/eq/input.essence)
- [smt/set/eq_overlap](../../test-suite/tests/integration/smt/set/eq_overlap/input.essence)
- [smt/set/subset-sum](../../test-suite/tests/integration/smt/set/subset-sum/input.essence)

## Initial-screen timeouts (37)

Last recorded with eight seconds for compilation and twelve seconds for the solve process. Longer runs are needed before classifying these as unsupported.

- [conjure/function/function_partial_smoke](../../test-suite/tests/integration/conjure/function/function_partial_smoke/input.essence)
- [conjure/permutation/perm_card_0003](../../test-suite/tests/integration/conjure/permutation/perm_card_0003/input.essence)
- [conjure/permutation/perm_eq_0005](../../test-suite/tests/integration/conjure/permutation/perm_eq_0005/input.essence)
- [conjure/permutation/perm_inverse_0005](../../test-suite/tests/integration/conjure/permutation/perm_inverse_0005/input.essence)
- [conjure/permutation/perm_repr_0006](../../test-suite/tests/integration/conjure/permutation/perm_repr_0006/input.essence)
- [conjure/permutation/perm_repr_0008](../../test-suite/tests/integration/conjure/permutation/perm_repr_0008/input.essence)
- [conjure/permutation/perm_repr_0012](../../test-suite/tests/integration/conjure/permutation/perm_repr_0012/input.essence)
- [conjure/relation/binrel02](../../test-suite/tests/integration/conjure/relation/binrel02/input.essence)
- [conjure/relation/binrel03](../../test-suite/tests/integration/conjure/relation/binrel03/input.essence)
- [conjure/relation/relation04_find](../../test-suite/tests/integration/conjure/relation/relation04_find/input.essence)
- [conjure/relation/relation04_param](../../test-suite/tests/integration/conjure/relation/relation04_param/input.essence)
- [conjure/relation/relation07_connex](../../test-suite/tests/integration/conjure/relation/relation07_connex/input.essence)
- [mildly-interesting/gchq-2016](../../test-suite/tests/integration/mildly-interesting/gchq-2016/gchq.essence)
- [savilerow/blackhole](../../test-suite/tests/integration/savilerow/blackhole/input.essence)
- [savilerow/carSequencing](../../test-suite/tests/integration/savilerow/carSequencing/input.essence)
- [savilerow/cryptArithmetic](../../test-suite/tests/integration/savilerow/cryptArithmetic/input.essence)
- [savilerow/diet](../../test-suite/tests/integration/savilerow/diet/input.essence)
- [savilerow/efpa](../../test-suite/tests/integration/savilerow/efpa/input.essence)
- [savilerow/golomb](../../test-suite/tests/integration/savilerow/golomb/input.essence)
- [savilerow/golomb2](../../test-suite/tests/integration/savilerow/golomb2/input.essence)
- [savilerow/killer16](../../test-suite/tests/integration/savilerow/killer16/input.essence)
- [savilerow/lee-distance](../../test-suite/tests/integration/savilerow/lee-distance/input.essence)
- [savilerow/magicSequence](../../test-suite/tests/integration/savilerow/magicSequence/input.essence)
- [savilerow/multiDimensionArray](../../test-suite/tests/integration/savilerow/multiDimensionArray/input.essence)
- [savilerow/pegSolitaireTable](../../test-suite/tests/integration/savilerow/pegSolitaireTable/input.essence)
- [savilerow/plotting](../../test-suite/tests/integration/savilerow/plotting/input.essence)
- [savilerow/quasiGroup3Idempotent](../../test-suite/tests/integration/savilerow/quasiGroup3Idempotent/input.essence)
- [savilerow/quasiGroup3NonIdempotent](../../test-suite/tests/integration/savilerow/quasiGroup3NonIdempotent/input.essence)
- [savilerow/quasiGroup4Idempotent](../../test-suite/tests/integration/savilerow/quasiGroup4Idempotent/input.essence)
- [savilerow/quasiGroup4NonIdempotent](../../test-suite/tests/integration/savilerow/quasiGroup4NonIdempotent/input.essence)
- [savilerow/quasiGroup5Idempotent](../../test-suite/tests/integration/savilerow/quasiGroup5Idempotent/input.essence)
- [savilerow/quasiGroup6](../../test-suite/tests/integration/savilerow/quasiGroup6/input.essence)
- [savilerow/quasiGroup7](../../test-suite/tests/integration/savilerow/quasiGroup7/input.essence)
- [savilerow/semigroup](../../test-suite/tests/integration/savilerow/semigroup/input.essence)
- [savilerow/solitaire_battleship](../../test-suite/tests/integration/savilerow/solitaire_battleship/input.essence)
- [savilerow/test-branchingon2](../../test-suite/tests/integration/savilerow/test-branchingon2/input.essence)
- [savilerow/test-indexing-flatten](../../test-suite/tests/integration/savilerow/test-indexing-flatten/input.essence)

## Coverage gaps outside these failure counts

- The current integer portfolio covers Direct, Order, BinaryValue (`IntLog`), BinaryOffset (`IntOffset`) and BinaryRank (`IntRank`). Sparse Rank maps codes to actual domain values before numeric encoding.
- Full lazy mixed-representation materialisation and channelling are not yet verified by this uniform campaign.
- `cnf/cnf2` has two Essence inputs in one directory and is not discovered as an integration fixture. This is a harness/discovery gap, not a recorded solver failure.
- Dedicated decisions for the remaining encoding families are still planned; absence of a selectable library encoding does not imply that every corresponding constraint fails, because existing decompositions may work.

## Suggested investigation order

1. Fix the ground-domain crash and determine whether constant-matrix indexing is a shared frontend issue.
2. Repair matrix equality/disequality lowering for the five reconfirmed loading errors.
3. Retest `basic/abs/03-nested` across the complete uniform portfolio; compact/first already succeed.
4. Re-screen residual-constraint cases after the weighted PB changes, then group remaining failures by their actual residual ASTs.
5. Revisit bounded timeouts with longer limits and separate compilation cost from solving cost.

## Passing performance follow-ups

The new integer relation decisions expand PB provider selection. `basic/weighted-sum/05-flattening` and `savilerow/quantification_over_matrix_doms_2` both pass, with portfolios growing from 10 to 50; their recorded acceptance budgets are now 240 and 270 seconds respectively. Profile these cases when tuning the integer strategies. Integer relations now preserve compatible representation groups through native asserted PB calls and both implication directions for Pindakaas BDD/SWC. General guarded/reified AMO/cardinality/PB remains open.
