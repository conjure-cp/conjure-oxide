# SAT backend: known issues and disabled tests

Inventory date: 2026-10-04, branch `sat-ir` after ordinary objective tightening was connected.

388 of 635 runnable integration fixtures have SAT enabled, exercising 21,613 SAT portfolios. Full acceptance for the objective-tightening stage passed all 1,573 workspace tests (14 skipped), including workspace doctests. The earlier integer relation migration also passed full normal golden verification. The remaining 247 fixtures have SAT disabled. Disabled does not establish that a fixture still fails on today's code: the exhaustive screen predates the weighted PB and signed arithmetic changes.

This list filters the [coverage CSV](sat_coverage_survey.csv) against current test configurations, excluding cases that have since been enabled. The CSV records observations rather than independently diagnosed root causes. No remaining solution mismatch is recorded in that survey; the three former mismatches have been fixed and enabled.

Scalar integer/Boolean allDifferent now passes pairwise and eligible value-AMO portfolios. allDifferentExcept and compound-valued operands remain gaps; the dedicated fixture also covers sparse domains, repeated operands, constants, negation and reification. No new upstream library bug was confirmed.

Constant-row positive/negative tables now pass tuple and MDD portfolios, including reification. Short tables, binary-support encodings and non-constant row relations remain gaps. A fresh compact screen of `savilerow/sportsScheduling2` lowers its tables but leaves `flatten` inside allDifferent; that fixture remains disabled.

Scalar element definitions now pass implication/support portfolios across all five integer representations and five PB providers. Internal scalar index-membership guards lower through existing numeric relations, retaining definedness for nested, negated, reified and masked lookups. Twelve existing fixtures are enabled; `sat-ir/element` adds sparse and masked regression coverage. Identity-default ElementId and scalar lexicographic comparisons now pass full uniform portfolios in eight newly enabled existing fixtures. The new non-involutive, sparse and Boolean-conversion fixture checks 100 portfolios against 80 Conjure reference solutions. Constant scalar function-domain IndexOf now reuses element selection. Compound/non-constant inverse lookups and remaining arithmetic strategies are still SAT gaps. Mixed representations and channelling remain deferred.

## Modulo-stage function portfolio screen

`SafeMod` now reuses the restoring division circuit and RustSAT Boolean gate decisions. All five integer representations and five PB providers match Conjure for signed, zero-divisor masking and negated comparisons. Domain inference and constant evaluation now share exact floor arithmetic, including machine-boundary remainders.

Full uniform acceptance enabled `basic/mod/{01,03,04}`, four `basic/function/apply-*` fixtures, and `conjure/function/function_total_{bool_01,bool_02,bool_smoke,int_01}`. The new `sat-ir/modulo` regression includes all 81 signed operand pairs with `catchUndef` and 100 SAT portfolios. Trials used `TEST_CASE_TIMEOUT=120`.

Two function fixtures remained disabled after that screen; the asserted-conjunction stage below updates their outcomes:

- `basic/function/sparse-partial`: the FunctionAsRelation occurrence representation leaves `sum(toInt(occurrence)) >= 0` and `<= 2` unencoded. The run failed after about 70 seconds; this is an Oxide lowering gap before library encoding.
- `conjure/function/function_total_int_02`: the full uniform SAT trial exceeded 120 seconds. This establishes a bounded timeout, not a semantic mismatch.

A separate large sparse-domain probe, `find a : int(-2147483647 - 1, 2147483647)`, stalls at representation selection before modulo lowering. Its owned probes were stopped; machine-boundary modulo is checked directly through semantic circuit evaluation. Neither this observation nor the function failures establishes a RustSAT/Pindakaas bug.

Explicit function lookup also needs a guard on its original argument domain: an absent argument can equal a valid internal position through inverse identity padding. The guard is retained separately from the inverse lookup and partial-function flags, so `catchUndef` can select its fallback.

## Asserted conjunction connection

Native AMO/cardinality and PB selection now searches literal conjunctions beneath Root while preserving their grouping. Only asserted conjuncts are extracted; negation, disjunction, implication and reification are not asserted contexts. This avoids globally flattening the evaluator worklist. Both cardinality providers and all five PB providers match Conjure across all five integer representations in 50 explicit checks.

The three-element occurrence set (`maxSize 2`, with `1 in s`) now has exactly the three expected solutions through native cardinality decisions. `sat-ir/asserted-conjunctions` adds nested cardinality and weighted bounds plus a reified scalar comparison. The subsequent guarded-count connection below supports direct reified counts such as `sum(toInt(...)) <= 1 <-> p` through dedicated AMO/cardinality equivalence decisions.

`conjure/function/function_total_int_02` passed all 1,000 SAT portfolios with `TEST_CASE_TIMEOUT=600`, taking about 168 seconds in that trial. It is enabled under full uniform selection; the earlier 120-second timeout was a performance observation, not unsupported semantics.

`basic/function/sparse-partial` remains disabled. Its 120-second fresh trial timed out after the cardinality connection. The 600-second retry completed 600 SAT portfolios before receiving SIGKILL after about 472 seconds. No residual-constraint error was observed in that retry. The attempted process sample arrived after the process had exited, so the termination stage and cause are unconfirmed. The failed configuration and generated trial artefacts were restored. Code inspection identifies a possible resource stressor: the packed partial function has radix seven over six positions, yielding 117,649 integer codes. Direct then has 117,649 indicators, and RustSAT pairwise AMO would emit 6,920,584,776 clauses. This is a candidate to isolate, not confirmation that the killed trial reached that combination. No RustSAT/Pindakaas bug was established.

## Guarded counts and bounded function probes

Ready Boolean counts beneath negation, disjunction, implication, equivalence and `toInt` now become output-bearing `CountRelation` decisions. Both truth values are encoded by the selected cardinality provider, with the selected AMO provider handling upper thresholds of one. Asserted bounds retain native AMO/cardinality selection; asserted count disequality uses an equivalence decision. The selector traverses borrowed scalar/Boolean contexts, preserving conjunctions and avoiding binders and undefined-value bubbles. It does not add a clause encoder.

Three hundred explicit portfolios across all five integer representations, all six AMO encodings, both cardinality providers and all five PB providers agree with nine independent and Conjure reference assignments. `sat-ir/guarded-counts` checks all six comparisons, reversed bounds, duplicates, negated operands, constants, guards and `toInt`; its full uniform sweep has 300 SAT portfolios. Dedicated AMO/cardinality-provider reification is now connected. The Boolean-only `sat-ir/reified-cardinality` fixture exercises all twelve provider pairs without numeric or weighted constraints, matching 64 Conjure and independently enumerated assignments. All five normal golden checks pass; existing timing budgets are preserved and the new fixture has a deliberate five-second budget.

The original sparse-partial fixture passes Packed + BinaryValue with exactly five expected assignments. Bounded rewrite-only probes show Packed + Direct costs growing with the packed integer domain: three-by-three (64 codes) takes about 0.09 seconds, four-by-four (625) 0.32 seconds and five-by-five (7,776) 3.56 seconds. Six-by-six (117,649) exceeds the 15-second rewrite limit; BinaryValue finishes that rewrite in about 1.76 seconds. Pairwise CNF probes for the first two cases pass with 40,339 and 348,922 clauses respectively. These isolate representation-size growth before or during encoding, but do not establish the cause of the earlier SIGKILL or a library bug. The original full fixture stays disabled.

A three-by-three full uniform trial also exceeded 120 seconds after 150 successful SAT portfolios. Reducing the codomain to two values keeps the third argument absent while making the packed function domain 27 codes and the packed relation/set domain 64 codes. `sat-ir/sparse-partial-small` then passed all 800 SAT portfolios in about 110 seconds under `TEST_CASE_TIMEOUT=120`.

Fresh full uniform trials also enable `basic/toInt/{01,02-flatten}` and `basic/abs/03-nested`, each with 50 SAT portfolios. These rechecks establish current support; they do not attribute the fixes to the new guarded-count selector. No new upstream bug was confirmed. Mixed representations/channelling and the Minion masked function lookup discrepancy remain deferred.

## Summary of last recorded outcomes

| Outcome | Fixtures | Evidence |
| --- | ---: | --- |
| Residual constraints in initial screen | 177 | Compact/first CLI screen failed to finish lowering. |
| Residual constraints in full uniform portfolio | 22 | Historical full uniform lowering failures; fresh passing and resource-limited trials are separated below. |
| Panics | 2 | Both reconfirmed with compact and first; the historical nested-absolute case now passes a full uniform portfolio. |
| Model-loading errors | 5 | All five reconfirmed in the earlier targeted recheck. |
| Initial screen timeouts | 37 | Eight-second compilation / twelve-second solve-process limits. |
| Full uniform portfolio timeouts | 3 | 120-second per-fixture limit. |
| External termination | 1 | Fresh sparse-partial trial received SIGKILL after about 472 seconds; cause unconfirmed. |
| Total SAT-disabled runnable fixtures | 247 | Current configurations matched to survey records. |

Timeouts are performance observations under those limits, not proof of unsupported semantics. The 199 residual-constraint cases were not all rerun after the PB changes. Residual constraints identify incomplete lowering; they do not by themselves identify the missing rule or representation.

## Concrete crashes and loading errors

Before the guarded-count stage, targeted rechecks used the tree-sitter parser, uniform channelling, compact and first heuristics, and one requested solution. Those checks left fixture configurations and goldens unchanged.

| Fixture | Observation | Current status |
| --- | --- | --- |
| `basic/lettings/04-domain` | SAT adaptor panics: `Domain should be ground`. | Reconfirmed with compact and first. |
| `savilerow/const_matrix_test` | Indexing panics: `0 is not a valid index for dimension 0`. | Reconfirmed with compact and first; not established as exclusive to SAT. |
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

## Residual constraints: full-portfolio failures (22)

These failed a historical full uniform portfolio; most passed the initial screen. The fresh sparse-partial resource outcome is recorded above. They include partial/total functions, tuple and record operations, multisets, sequences, sets and matrices containing sets. These are observed fixture families, not a blanket claim that every operation of those types fails.

- [basic/tuples/01-bool-int](../../test-suite/tests/integration/basic/tuples/01-bool-int/input.essence)
- [basic/tuples/03-equality](../../test-suite/tests/integration/basic/tuples/03-equality/input.essence)
- [basic/tuples/04-inequality](../../test-suite/tests/integration/basic/tuples/04-inequality/input.essence)
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

## Initial-screen residual constraints (177)

Last recorded in the compact/first screen, before the weighted PB changes. Each fixture below remains SAT-disabled; a fresh screen may move cases out of this list.

- [antichain](../../test-suite/tests/integration/antichain/antichain.essence)
- [basic/comprehension/set-bounded-image-member](../../test-suite/tests/integration/basic/comprehension/set-bounded-image-member/input.essence)
- [basic/comprehension/set-bounded-size-member](../../test-suite/tests/integration/basic/comprehension/set-bounded-size-member/input.essence)
- [basic/comprehension/set-fixed-size-member](../../test-suite/tests/integration/basic/comprehension/set-fixed-size-member/input.essence)
- [basic/comprehension/set-tuple-member](../../test-suite/tests/integration/basic/comprehension/set-tuple-member/input.essence)
- [basic/exists/simple/04](../../test-suite/tests/integration/basic/exists/simple/04/input.essence)
- [basic/finite-givens/set03](../../test-suite/tests/integration/basic/finite-givens/set03/finite-givens-set03.essence)
- [basic/finite-givens/set05](../../test-suite/tests/integration/basic/finite-givens/set05/finite-givens-set05.essence)
- [basic/matrix/02-2d-slicing](../../test-suite/tests/integration/basic/matrix/02-2d-slicing/input.essence)
- [basic/matrix/03-domain-letting](../../test-suite/tests/integration/basic/matrix/03-domain-letting/input.essence)
- [basic/mod/05](../../test-suite/tests/integration/basic/mod/05/mod-05.essence)
- [basic/mod/06](../../test-suite/tests/integration/basic/mod/06/mod-06.essence)
- [basic/pow/01-simple](../../test-suite/tests/integration/basic/pow/01-simple/input.essence)
- [basic/pow/02-exponent-zero](../../test-suite/tests/integration/basic/pow/02-exponent-zero/input.essence)
- [basic/pow/03-negative-exponent](../../test-suite/tests/integration/basic/pow/03-negative-exponent/input.essence)
- [basic/pow/04-flatten](../../test-suite/tests/integration/basic/pow/04-flatten/input.essence)
- [basic/pow/05-negative-base](../../test-suite/tests/integration/basic/pow/05-negative-base/input.essence)
- [basic/sequence/apply-out-of-range](../../test-suite/tests/integration/basic/sequence/apply-out-of-range/input.essence)
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
- [savilerow/problem110](../../test-suite/tests/integration/savilerow/problem110/input.essence)
- [savilerow/quantification_over_matrix_doms_4](../../test-suite/tests/integration/savilerow/quantification_over_matrix_doms_4/input.essence)
- [savilerow/quasiGroup5NonIdempotent](../../test-suite/tests/integration/savilerow/quasiGroup5NonIdempotent/input.essence)
- [savilerow/sendMoreMoney](../../test-suite/tests/integration/savilerow/sendMoreMoney/input.essence)
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
- [savilerow/test_power_raw](../../test-suite/tests/integration/savilerow/test_power_raw/input.essence)
- [savilerow/tictactoe](../../test-suite/tests/integration/savilerow/tictactoe/input.essence)
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

1. Connect general reusable cardinality bounds and repeated dominance-counter sharing; PB objective tightening and dedicated AMO/cardinality-provider reification are connected.
2. Tune cost-aware representation/encoding selection and further isolate the original sparse-partial termination; bounded probes already show packed Direct growth.
3. Fix the ground-domain crash and determine whether constant-matrix indexing is a shared frontend issue.
4. Repair matrix equality/disequality lowering for the five reconfirmed loading errors.
5. Retest the smallest remaining disabled arithmetic fixture; `basic/abs/03-nested` now passes all 50 uniform portfolios.
6. Re-screen residual-constraint cases after the weighted PB changes, then group remaining failures by their actual residual ASTs.
7. Revisit bounded timeouts with longer limits and separate compilation cost from solving cost.

## Passing performance follow-ups

The new integer relation decisions expand PB provider selection. `basic/weighted-sum/05-flattening` and `savilerow/quantification_over_matrix_doms_2` both pass, with portfolios growing from 10 to 50; their recorded acceptance budgets are now 240 and 270 seconds respectively. Profile these cases when tuning the integer strategies. Integer relations now preserve compatible representation groups through native asserted PB calls and both implication directions for Pindakaas BDD/SWC. Ready guarded/reified counts now use dedicated AMO/cardinality decisions. Weighted and linear comparisons retain PB-backed equivalence decisions. PB objective tightening now reuses native RustSAT state. General cardinality-bound reuse and repeated dominance-counter sharing remain separate library connections.

## Cross-backend masked lookup follow-up

Independent explicit-function probes expose a separate Minion discrepancy. The total model below has 24 assignments (four functions times six arguments); SAT returns exactly those assignments across 50 integer/element/PB portfolios. Minion with `--heuristic=i --responses=2 --channelling=uniform` returns eight assignments, all with `i` inside the original domain. Replacing `total` with `size 1` also gives eight rather than 24; Boolean-domain probes return the expected eight assignments. The original-domain guard is retained, but the Minion lookup path still appears to force validity of a masked lookup. Its precise cause remains to be isolated; this is not a diagnosed RustSAT/Pindakaas bug.

```essence
language Essence 1.3
find f : function (total) int(-2,3) --> int(10,20)
find i : int(-3,-2,0,1,3,4)
find y : int(-7,10,20)
such that y = catchUndef(f(i), -7)
```


## Ordinary optimisation and cross-backend objective handling

The production SAT adaptor previously enumerated feasible assignments while ignoring ordinary objectives. It now records an actual-value objective decision and strictly improves it until UNSAT proves optimality. RustSAT GTE/adder/DPW retain native bound state; Pindakaas BDD/SWC use one-shot structured numeric relations. The native projection tests caught permanent DPW enforcement units conflicting after tightening; replacing per-solve assumptions fixes that adaptor usage error. No new upstream library bug was confirmed.

The separate Z3 adaptor still ignores ordinary objectives. A CLI probe of `sat-ir/objective-min` returns `x=-3,y=0,g=false` (cost -2), although `x=-3,y=2,g=true` has the optimum cost -6. The new objective regressions therefore use SAT, Minion and Conjure references. Existing intermediate-optimisation fixtures keep their Conjure exemption because Conjure exposes the final result rather than the intermediate sequence.

General reusable cardinality bounds and repeated dominance-counter sharing remain deferred; this stage connects PB objective tightening. Mixed representations/channelling, cost-aware selection and the original sparse-partial packed-Direct growth remain separate work items.
