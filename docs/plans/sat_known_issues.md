# SAT backend: known issues and coverage failures

## Latest compact acceptance (5-6 October 2026)

The [full acceptance report](sat_compact_acceptance.md) supersedes the outcome
counts below: 661 of 668 attempted integration fixtures pass after compact
follow-ups. Five passes are rewrite-only. Seven failures remain: the constant
matrix index panic, four memory limits and two SAT rewriting timeouts. All other
workspace tests and doctests pass. Slow SAT portfolios use compact, with uniform
channelling retained; all 32 further compact-profile changes pass normal golden
verification. The [CSV](sat_compact_acceptance.csv) records every fixture.

Subsequent constant-matrix follow-up: `const_matrix_test` now passes all 25 SAT
portfolios with five solutions. Shared matrix-domain and bubbling fixes pass
437 core/rule tests, `make check` and 87 nearby normal integration checks. The
six resource outcomes above had not been remeasured after this fix.

Subsequent compact-policy follow-up: `function_partial_smoke` now passes with all
201 assignments matching Conjure. Correcting packed set/relation SAT scores to
count the full stored mask domain selects explicit storage; SAT translation and
enumeration took 0.063 and 0.044 seconds in the bounded recording run. The five
other resource failures have not been remeasured. See the
[compact policy audit](compact_modelling_choices.md) for changes and measurements.

## Latest workspace verification (2026-10-05)

See [the workspace verification report](sat_workspace_verification.md) for the
full normal test run after the later allDifferent, lookup and lex fixes. The run
passed 626 of 668 integration fixtures; its 42 failures split into 22 missing SAT
snapshots, ten outdated snapshots, five memory limits, three timeouts and two
panics. All other workspace tests and doctests passed. Scalar lex now accepts any
one-dimensional matrix index domain and unequal lengths across all five integer
representations. The ten outdated snapshots have since been refreshed and pass
normal verification: 636 fixtures pass and 32 failures remain. The survey below
is historical, including its residual counts.

Subsequent fix: `basic/lettings/04-domain` now passes all 65 SAT portfolios with
five solutions. SAT loading resolves domain lettings instead of requiring their
stored domain to be ground. The constant-matrix indexing panic remains open.

## Full integration coverage sweep (2026-10-05)

The latest complete coverage report passes **602 of 667 fixtures**, compared with 565 of 664 in the first survey. SAT is configured for every discovered fixture: 662 normally runnable and five globally skipped fixtures explicitly attempted. SAT uses `heuristic="x"` with uniform channelling in 622 fixtures and compact's normal representation and SAT encoding choices in 45 fixtures whose other backend profiles do not enumerate choices. No SAT encoding options are pinned; existing Minion/Z3 profiles are preserved.

See [the full coverage report](sat_full_coverage_survey.md) and [the exhaustive per-fixture CSV](sat_full_coverage_survey.csv). The survey uses four workers, a 600-second fixture timeout and a 4 GiB process-tree RSS ceiling. Fifteen power-containing fixtures and fourteen occurrence-constraint fixtures have targeted follow-up runs after the final fixes, with the same profiles and limits. Their latest outcomes supersede their earlier attempts.

Occurrence membership now guards the lookup domain, making out-of-domain membership false under negation and reification. Boolean gate checks preserve compound equality for representation lowering. Power uses exponentiation by squaring through existing circuit decisions, including constant-folded exponent bits. Native `atMost`/`atLeast` and global cardinality accept direct and flattened matrix operands and reuse cardinality/PB decisions. No solution mismatch or SAT loading error remains in this survey.

`make check`, workspace library tests/doctests, all 174 final rule tests, 22 focused SAT golden checks, five passing occurrence follow-up golden checks, and 72 affected Minion/Z3 golden checks pass. The updated flattened-occurrence regression also passes a fresh Minion check. Timing baselines are preserved for existing run identities.

Masked power matches all 30 independently enumerated assignments across 130 SAT portfolios. Conjure/Savile Row drops six fallback assignments because its native power constraint is unconditional; a report is saved locally in `bug-reports/savilerow-masked-power/README.md` and remains uncommitted. The regression explicitly skips that reference. No new RustSAT/Pindakaas defect was established. Oxide's Minion backend had the same bug and is fixed: `pow` is guarded with `reifyimply` where the power can be undefined, so Minion also returns all 30 solutions. Machine-overflowing SAT power ranges remain unsupported.

Current failures: 12 residual constraints (allDifferent input forms and compound lexicographic comparison), 31 SAT/model memory limits, 16 timeouts, two panics, two globally skipped frontend failures, and two globally skipped reference memory limits. Resource limits do not establish unsupported semantics. Complete passes include three rewrite-only fixtures, which do not establish SAT solving support.

The sections below record historical screens; their disabled counts and outcome claims are superseded by the current report.

## Historical screens before the full coverage sweep

Inventory date: 2026-10-04, branch `sat-ir` after variable exceptions and compound allDifferent operands were connected.

413 of 649 runnable integration fixtures have SAT enabled, exercising 23,536 SAT portfolios. Full acceptance passes all 1,602 workspace tests (14 skipped), plus workspace doctests. All 18 focused checks, independent audits and 27 cleaned normal golden checks pass. The preceding constant-exception stage passed all 1,592 workspace tests (14 skipped), with workspace doctests also passing. The earlier integer relation migration also passed full normal golden verification. The remaining 236 fixtures have SAT disabled. Disabled does not establish that a fixture still fails on today's code: the exhaustive screen predates the weighted PB and signed arithmetic changes.

This list filters the [coverage CSV](sat_coverage_survey.csv) against current test configurations, excluding cases that have since been enabled. The CSV records observations rather than independently diagnosed root causes. No remaining solution mismatch is recorded in that survey; the three former mismatches have been fixed and enabled.

Scalar integer/Boolean allDifferent and variable integer allDifferentExcept now pass pairwise and eligible value-AMO portfolios. Compound allDifferent/allDifferentExcept reuse whole-value equality through pairwise semantic decisions, with verified tuple, record, set, matrix and sequence portfolios. No new upstream library bug was confirmed.

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
| Residual constraints in initial screen | 166 | Compact/first CLI screen failed to finish lowering. |
| Residual constraints in full uniform portfolio | 22 | Historical full uniform lowering failures; fresh passing and resource-limited trials are separated below. |
| Panics | 2 | Both reconfirmed with compact and first; the historical nested-absolute case now passes a full uniform portfolio. |
| Model-loading errors | 5 | All five reconfirmed in the earlier targeted recheck. |
| Initial screen timeouts | 37 | Eight-second compilation / twelve-second solve-process limits. |
| Full uniform portfolio timeouts | 3 | 120-second per-fixture limit. |
| External termination | 1 | Fresh sparse-partial trial received SIGKILL after about 472 seconds; cause unconfirmed. |
| Total SAT-disabled runnable fixtures | 236 | Current configurations matched to survey records. |

Timeouts are performance observations under those limits, not proof of unsupported semantics. The 188 residual-constraint cases were not all rerun after the PB changes. Residual constraints identify incomplete lowering; they do not by themselves identify the missing rule or representation.

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
- [basic/pow/01-simple](../../test-suite/tests/integration/basic/pow/01-simple/input.essence)
- [basic/pow/02-exponent-zero](../../test-suite/tests/integration/basic/pow/02-exponent-zero/input.essence)
- [basic/pow/03-negative-exponent](../../test-suite/tests/integration/basic/pow/03-negative-exponent/input.essence)
- [basic/pow/04-flatten](../../test-suite/tests/integration/basic/pow/04-flatten/input.essence)
- [basic/pow/05-negative-base](../../test-suite/tests/integration/basic/pow/05-negative-base/input.essence)
- [basic/sequence/apply-out-of-range](../../test-suite/tests/integration/basic/sequence/apply-out-of-range/input.essence)
- [bugs/treemorph-misses-node-01](../../test-suite/tests/integration/bugs/treemorph-misses-node-01/input.essence)
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
- [conjure/set/finiteGivens_set03](../../test-suite/tests/integration/conjure/set/finiteGivens_set03/input.essence)
- [conjure/set/finiteGivens_set05_p1](../../test-suite/tests/integration/conjure/set/finiteGivens_set05_p1/input.essence)
- [conjure/set/finiteGivens_set05_p2](../../test-suite/tests/integration/conjure/set/finiteGivens_set05_p2/input.essence)
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
- [sets/difference](../../test-suite/tests/integration/sets/difference/input.essence)
- [sets/equals2](../../test-suite/tests/integration/sets/equals2/input.essence)
- [sets/explicit-large-inner-domain](../../test-suite/tests/integration/sets/explicit-large-inner-domain/input.essence)
- [sets/neq-marker](../../test-suite/tests/integration/sets/neq-marker/input.essence)
- [sets/occurrence-large-inner-domain](../../test-suite/tests/integration/sets/occurrence-large-inner-domain/input.essence)
- [sets/occurrence-set-of-set-membership](../../test-suite/tests/integration/sets/occurrence-set-of-set-membership/input.essence)
- [sets/strict-subset-marker](../../test-suite/tests/integration/sets/strict-subset-marker/input.essence)
- [sets/subsetEq2](../../test-suite/tests/integration/sets/subsetEq2/input.essence)
- [sets/subseteq-marker-marker](../../test-suite/tests/integration/sets/subseteq-marker-marker/input.essence)
- [sets/union-comprehension](../../test-suite/tests/integration/sets/union-comprehension/input.essence)
- [sets/union2](../../test-suite/tests/integration/sets/union2/input.essence)
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

1. Keep compound comparison representation costs visible: packed sequence comparisons expand substantially and multiple padding codes can reconstruct the same sequence. Variable integer exceptions and compound allDifferent/allDifferentExcept are connected; continue checking complete semantic solution sets.
2. Complete remaining modelling/encoding families and missing library connections, then continue fresh uniform rechecks of the smallest disabled fixtures. Feature completeness takes priority over measurement and heuristic tuning.
3. Fix the ground-domain crash and determine whether constant-matrix indexing is a shared frontend issue.
4. Repair matrix equality/disequality lowering for the five reconfirmed loading errors.
5. Retest the smallest remaining disabled arithmetic fixture; `basic/abs/03-nested` now passes all 50 uniform portfolios.
6. Re-screen residual-constraint cases after the weighted PB changes, then group remaining failures by their actual residual ASTs.
7. After feature completeness, measure compilation and solving separately, tune cost-aware selection and further isolate the original sparse-partial termination; bounded probes already show packed Direct growth. Mixed representations/channelling remain deferred.
8. Revisit bounded timeouts with longer limits and separate compilation cost from solving cost.

## Passing performance follow-ups

The new integer relation decisions expand PB provider selection. `basic/weighted-sum/05-flattening` and `savilerow/quantification_over_matrix_doms_2` both pass, with portfolios growing from 10 to 50; their recorded acceptance budgets are now 240 and 270 seconds respectively. Profile these cases when tuning the integer strategies. Integer relations now preserve compatible representation groups through native asserted PB calls and both implication directions for Pindakaas BDD/SWC. Ready guarded/reified counts now use dedicated AMO/cardinality decisions. Weighted and linear comparisons retain PB-backed equivalence decisions. PB objective tightening now reuses native RustSAT state. Cardinality bounds now reuse state across dominance updates. Weighted dominance sharing now retains GTE/adder counters and bound-specific DPW/Pindakaas predicates; sparse Rank projection sharing is now connected.

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


## Reusable cardinality bounds and dominance counts

Initial model compilation and dominance updates now share solver-local counters by input polarity and multiplicity. RustSAT totalizers incrementally extend upper/lower structural encodings; only enforcement is guarded. Pindakaas sorting networks retain one-shot threshold predicates for repeated/complementary bounds, with separate native networks for different thresholds. The selected AMO provider still handles upper thresholds of one.

The two-count Pareto regression matches 20 independently enumerated nondominated assignments under both cardinality providers. Runtime probes record 86 count-compilation calls, 84 reusing an existing input set. Repeated bounds allocate no new auxiliaries in exhaustive multi-batch projection tests. This establishes cardinality sharing; it does not establish reuse of general weighted PB dominance counters or incremental construction of Pindakaas sorting networks. Those remain separate follow-ups. No new upstream library bug was confirmed.

The new count-based Pareto model exposes a separate Oxide Minion gap: mid-search dominance rewriting leaves a sum in a scalar comparison position, and `parse_atom` rejects it with `expected atomic expression`. Removing dominance allows the same constraints to solve. Flattening the initial bound into six weighted Boolean terms still leaves the Pareto-sum injection failing. This is before native Minion encoding, not a confirmed upstream Minion bug. The regression therefore selects SAT and checks its 20 assignments by independent enumeration; repairing Minion compound dominance lowering remains separate work.

Full four-thread acceptance passed all 1,575 tests (14 skipped), including workspace doctests. All four final normal golden checks passed. Sixty explicit dominance portfolios and twelve Boolean-only reification portfolios passed; the sixty recorded dominance portfolios match the independently enumerated frontier. Coverage is 389/636 runnable fixtures and 21,673 SAT portfolios, with 247 fixtures still disabled. Existing timing budgets are preserved; the new regression has a deliberate five-second budget.


## Weighted dominance reuse and sparse Rank sharing gap

GTE/adder counters now share stable weighted inputs, with guarded enforcement and cached equivalent predicates. DPW predicates own separate state for distinct simultaneous bounds; Pindakaas BDD/SWC retain structured inputs through their public one-shot APIs. Repeated thresholds and complementary signed sums reuse complete equivalences.

The sparse `sat-ir/dominance-weighted` model has five independently enumerated nondominated assignments. All 25 explicit integer/PB combinations match that frontier; 50 uniform recorded portfolios agree. Direct, Order, BinaryValue and BinaryOffset show cache hits for every provider. Sparse BinaryRank has zero hits: each dominance rewrite creates fresh actual-value arithmetic bits, giving identical mathematical views different input literals. CDP traces confirm fresh auxiliary names. Sharing these projections is an Oxide arithmetic-view/Boolean-arena connection, not an upstream library bug. Keep it visible before declaring dominance reuse complete for every representation.

Multi-batch tests activate all old/new thresholds together and reject each incorrect output polarity. Structured tests reorder groups, reverse choice-member order, change constant spelling and negate complementary sums without allocating new auxiliaries. Chain implication order remains significant. Wide asserted and reified chains also preserve projected solutions.

Unrestricted native sharing caused the 2,000-input Order/GTE fixture to run for 706 seconds without completing; a second attempt stopped at 382 seconds after only reification was corrected. Both acceptance attempts were deliberately interrupted. Sampling placed the cost in CaDiCaL search, not rewriting. The final provisional policy limits native reified GTE sharing to 64 weighted inputs, retaining library implication transformations for larger predicates. Adder reification still uses native state. Unconditional assertions preserve pruning and retain the original BoundBoth construction above 64 remaining inputs for both providers. Algorithms are unchanged; cost-aware tuning remains deferred.

All six focused integration checks pass. The three complete wide-domain fixtures finish in 63-65 seconds, matching prior 65-72-second baselines; large Direct addition and the dominance RFC example finish in about nine seconds. CLI GTE/adder probes return the unique value 4 in 1.65/1.38 seconds. This is an encoding-construction performance observation, not a confirmed upstream correctness bug.

Full `NEXTEST_TEST_THREADS=4 make test-accept` passed all 1,580 tests (14 skipped), plus workspace doctests. Production Clippy and formatting passed; core/rule libraries have 259/160 unit tests. All 13 final normal golden checks passed, including the new weighted regression, the wide-domain cases and four restored capped-search samples. Independent auditing confirms all 50 weighted portfolios match the five-assignment frontier. Timing cleanup preserves prior fields in 639 files; existing budgets remain unchanged and the new fixture has a deliberate five-second budget. Coverage is 390/637 runnable fixtures and 21,723 SAT portfolios; 21,586 are uniform across 302 fixtures, with 247 fixtures still disabled.


## Shared sparse Rank projections

The solver-local encoding cache now retains equivalent Boolean gates and output aliases across decision batches. Gate keys use canonical solver inputs and ignore commutative input order. Aliases retain polarity and constants; the original equivalence clauses remain present for earlier uses and solution decoding. Rebuilt Rank decoders therefore expose the same physical inputs to weighted encoders even when dominance rewriting assigns fresh auxiliary names. Cache state resets with each loaded model. Clause generation still uses RustSAT atomics; no CNF or cache state enters the model AST.

Multi-batch projection tests check all input assignments, negated alias chains, changing bounds, every PB provider and all old/new output truths together. Repeating a projection and bound adds no gate or PB auxiliaries beyond the new named outputs. A separate test retains assertions made before an alias and handles multiple definitions of the same output.

All 25 explicit integer/PB combinations of the existing weighted regression preserve its five-assignment frontier and confirm weighted reuse. `sat-ir/dominance-rank-views` adds unequal sparse gaps and multiple intervals, with three independently enumerated nondominated assignments. Its 25 explicit combinations preserve those assignments; sparse Rank reuses decoder gates with every provider. Distinct bounds still use the selected provider's established native or one-shot policy. No new upstream library bug was confirmed. Mixed representations/channelling, cost-aware selection and large packed-Direct growth remain deferred.

A fresh check of the smallest disabled model, `sets/in`, confirms a constant-set membership lowering gap: `find a: int(1..5); such that a in {1,2,3}` leaves membership over a ready Offset operand. The residual-model assertion fires before native SAT encoding. This is the next smallest-failing-fixture candidate, not a new upstream library bug.

Full `NEXTEST_TEST_THREADS=4 make test-accept` passed all 1,583 tests (14 skipped), plus workspace doctests. Production Clippy and formatting passed; core/rule libraries have 261/160 unit tests. All 44 SAT unit tests and 50 explicit integer/PB checks pass. Independent audits confirm both sets of 50 recorded weighted portfolios. All 16 final normal golden checks passed, including the wide-domain cases and six restored capped-search samples. Timing cleanup preserves prior fields in 639 files; existing budgets remain unchanged and the new fixture has a deliberate five-second budget. Coverage is 391/638 runnable fixtures and 21,773 SAT portfolios; 21,636 are uniform across 303 fixtures, with 247 fixtures still disabled.


## Constant-set membership

SAT now expands membership in literal integer sets through the existing disjunction-of-equalities helper. The Base rule still delegates these sets to Minion's native `w-inset`; the SAT rule fills the missing connection without emitting clauses or adding an encoding algorithm. Existing numeric relation and Boolean decisions handle assertions, negation, reification and compound member expressions across all five integer representations.

`sets/in` is enabled for uniform SAT portfolios while retaining Minion. Its complete expected assignments are `a = 1, 2, 3`. `sat-ir/set-membership` covers negative and zero values, sparse literal sets, negation, reification and a compound member. All 25 explicit integer/PB combinations match four independently enumerated assignments. The regression keeps Conjure validation. Unit checks cover empty/repeated sets and both AST literal forms, and retain Base delegation for Minion.

The compound-member probe also exposes an existing Oxide Minion gap: `x + 1 in {-2,0,3,4}` remains unlowered because native `w-inset` requires an atom. The new regression therefore selects SAT. The parser's set-literal grammar requires at least one element, rejecting source `{}`; empty-set semantics are checked directly through the AST. Both are separate Oxide gaps, with no new upstream library bug confirmed.

A fresh recheck also enables `conjure/set/cut_01_off` with 102 uniform SAT portfolios. Each matches all eight independently enumerated subsets of `{1,2,3}` and Conjure reference solutions. Its compact check already passed before this trial; this is a current-support recheck, not attribution to the membership fix. The next confirmed small failure is `smt/int/to-int`, whose Boolean-to-integer product remains unlowered before library encoding.

Full `NEXTEST_TEST_THREADS=4 make test-accept` passed all 1,585 tests (14 skipped), plus workspace doctests. Production Clippy and formatting passed. All 15 membership-related unit tests and 25 explicit integer/PB checks passed. Independent audits verify all 202 newly recorded SAT portfolios. All 16 final normal golden checks passed, including the changed fixtures and six restored capped-search samples. Timing cleanup preserves prior fields in 640 files; the expanded subset fixture budget rises from one to five seconds, the new regression has a five-second budget, and other existing budgets remain unchanged. Coverage is 394/639 runnable fixtures and 21,975 SAT portfolios; 21,838 are uniform across 306 fixtures, with 245 fixtures still disabled.


## Boolean-indicator products

Products of ready Boolean indicators lower to the indicator of their conjunction, preserving native Boolean/PB encoding. `smt/int/to-int` now passes five SAT portfolios; `sat-ir/boolean-products` checks repeated factors, negation, weighted sums, numeric output variables and reification across 50 portfolios. General integer factors still require the separate numeric-view connection described in the investigation order.

Fresh full uniform rechecks enable `conjure/set/setOfSet01`, `conjure/set/set01_1` and `sets/constant-eval-set-tests/Union`. These are recorded as coverage rechecks rather than attributed to the product fix. Independent audits verify all 309 newly recorded SAT portfolios: one assignment for the original product case, four for the new regression, all 16 sets of subsets of `{1,2}`, three size-two subsets of `{1,2,3}` and three union-membership values. Conjure validation is retained.

Full `NEXTEST_TEST_THREADS=4 make test-accept` passed 1,587 tests (14 skipped), plus workspace doctests. `make check` passed. All 12 targeted normal golden checks passed, covering the five changed/new fixtures, six retained capped-search samples and the affected `problem51` trace. Coverage is 399/640 runnable fixtures and 22,284 SAT portfolios; 22,147 are uniform across 311 fixtures, with 241 fixtures still disabled.

Timing cleanup preserves prior fields in 640 files. The four expanded fixtures deliberately increase their one-second budgets to five seconds, except sets of sets at ten seconds; the new regression has a five-second budget. Twenty-six changed capped-search solution samples were restored, and their normal checks passed. Meaningful rule-attempt/application counts and changed traces are retained. No new upstream library bug was confirmed. Mixed integer/indicator arithmetic is the next confirmed connection gap; performance measurement and heuristic tuning follow feature completeness, with mixed representations/channelling still deferred.


## Mixed integer/Boolean-indicator arithmetic

Ready Boolean indicators now expose temporary actual-value binary operands, with an explicit zero sign bit. The fallback waits for pending operands, preserving native cardinality selection. `sat-ir/mixed-indicators` passes 100 uniform portfolios with sixteen independently enumerated assignments, including signed multiplication, floor division/modulo and a compound reified count. Three partial-function regressions retain 800 portfolios each; premature conversion in the first attempt had removed 135 cardinality portfolios despite correct solutions.

Fresh coverage rechecks enable `basic/mod/05` and `basic/mod/06`, with 50 portfolios each and complete sets of 36 and 52 assignments. The latter allows zero divisors under negated equality; the former requires a defined remainder. Minion, Z3 and Conjure validation are retained. Coverage is 402/641 runnable fixtures and 22,484 SAT portfolios; 22,347 use uniform channelling across 314 fixtures, leaving 239 disabled fixtures.

Full `NEXTEST_TEST_THREADS=4 make test-accept` passed all 1,590 tests (14 skipped), plus workspace doctests. `make check` passed. All 15 targeted normal golden checks passed, covering the new/expanded fixtures, restored cardinality portfolios, affected indicator/count cases, sparse Rank dominance and five retained capped-search samples. Independent audits verify all 200 new SAT portfolios. Timing cleanup preserves prior fields in 643 files; the three new/expanded fixtures have deliberate five-second budgets and other existing budgets remain unchanged. No new upstream library bug was confirmed.

The next confirmed constraint-family gap is `allDifferentExcept([x,y], 0)` over small ready integer views. Performance measurement and heuristic tuning follow feature completeness; mixed representations/channelling remain deferred.


## Constant integer allDifferentExcept connection

The allDifferent decision retains an optional constant integer exception. Pairwise uses numeric equalities and Boolean disjunction; value-AMO omits the exception bucket. Selected library providers still own clause generation. Sparse Rank uses its existing actual-value projection, while Direct and Order retain structured views. Plain allDifferent retains its existing path.

The new sparse/reified/negated regression returns 21 independently enumerated assignments across 80 portfolios. The original comprehension regression adds 80 portfolios returning all 34 assignments. The two Savile Row cases each add 80 portfolios with 100 distinct valid capped assignments per result, including the constraint inside a disjunction. Conjure validation remains enabled. Variable exceptions and compound-valued operands remain gaps; the new rule unit test deliberately declines a non-constant exception.

Full `NEXTEST_TEST_THREADS=4 make test-accept` passed all 1,592 tests (14 skipped), plus workspace doctests. `make check` and all nine focused checks passed. All eleven final normal golden checks passed, including the existing allDifferent regression, sparse Rank dominance and five restored capped-search fixtures. Independent audits verify all 320 new SAT portfolios. The sparse regression retains 50 pairwise and 30 value-AMO portfolios, covering all six AMO algorithms and five PB providers. No new upstream library correctness bug was confirmed.

Timing cleanup preserves prior fields in 645 files and restores 24 capped-search solution samples. The new regression and expanded comprehension fixture have deliberate five-second budgets; the two expanded Savile Row fixtures have ten-second budgets. Other existing budgets remain unchanged. Coverage is 406/642 runnable fixtures and 22,804 SAT portfolios; 22,667 are uniform across 318 fixtures, with 236 fixtures still disabled. Exactly four fixtures add 80 portfolios each; no existing portfolios were lost.


## Variable exceptions and compound distinctness

Variable integer exceptions now retain actual-value numeric views in allDifferent decisions, including their declaration references and PB selection inputs. Pairwise shares operand/exception equalities; value-AMO guards native AMO buckets by exception/value equality. Reified uses retain full truth equivalences. Minion defines exact occurrence counts with an unconditional GCC and uses `(exception = value) or (count <= 1)` as the constraint truth; wide domains use pairwise comparisons instead of exhaustive domain enumeration.

Compound operands pass through a pending whole-value comparison node and existing equality rules. The terminal allDifferent decision keeps the pairwise encoding choice. Explicit value-AMO is rejected for compound values. No clauses are stored in the model. Tuple domains with different packed codes, records with reordered fields, sets, sparse-index matrices and variable-length sequences all match independent enumeration, including repeated operands, constant/variable exceptions, assertions, negation and reification.

Seven new fixtures add 732 SAT portfolios without removing existing portfolios. Conjure rejects compound exceptions, and Savile Row requires constant scalar exceptions; these fixtures record that limitation and use complete independent enumeration. Minion's variable integer regression agrees on all 29 assignments. Full `NEXTEST_TEST_THREADS=4 make test-accept` passes all 1,602 tests (14 skipped), plus workspace doctests. `make check`, all 18 focused checks, independent audits and 27 cleaned normal golden checks pass.

The parser now accepts compound exception literals and keeps record-field contexts independent. Constant matrix indexing now locates sparse labels without enumerating whole intervals. The five existing standalone matrix equality/disequality cases still fail to load with non-Boolean references in a fresh first-representation probe; they remain separate gaps. A tuple-valued non-constant indexed lookup used as the exception also leaves an unlowered whole-value equality, confirming that the existing compound-lookup gap still applies. A singleton compound allDifferent probe retains undefined-index truth correctly. Packed sequence padding permits duplicate decoded assignments, so sequence audits verify the complete semantic set and validity of every decoded assignment. A larger two-slot signed-integer sequence probe produced large packed lowering traces; the committed one-slot Boolean regression retains empty/inactive-padding coverage while keeping all representation choices manageable. Neither observation is a confirmed RustSAT/Pindakaas bug. Mixed representations/channelling and performance tuning remain deferred.


## Compound lookup and table workloop

Compound indexed values now pass through whole-value equality before representation lowering. Both element strategies remain selectable. General inverse lookup handles variable or compound entries, preserves actual sparse labels and selects the first match. Scalar absence retains the identity fallback; compound absence leaves the result guarded by the caller. Function-as-relation ordering now lowers flat lexicographic constraints. Compound defaults retain their return type.

Record domain membership matches field names rather than field positions. Independent lookup auditing found that positional membership could fold a valid reordered record comparison to false. This was an Oxide correctness bug, not an upstream encoder bug.

Numeric table decisions now retain variable row views. Tuple and MDD cover positive/negative, asserted/reified tables. Binary-support is available for constant two-column tables; incompatible explicit selections fail rather than changing algorithm. Short tables use explicit one-based position/value pairs, with omitted positions as wildcards. `shortTable` is an Oxide parser extension; Minion and SMT lowering is not added in this workloop.

Remaining scope still includes general symbolic matrix-literal equality and any compound lookup shapes not covered by the focused fixtures. The historical five standalone matrix failures need a fresh screen after component-reference equality support. Mixed representations/channelling remain deferred. Complete the public RustSAT/Pindakaas option audit before enabling all remaining integration fixtures. The existing disabled-fixture count is not a count of freshly confirmed bugs.

Boolean `catchUndef` remains a parser gap: the grammar classifies it as an arithmetic expression and rejects Boolean-context uses before operand typing. Direct `conjure type-check` accepts the same Boolean probe; the earlier reference-fetch error came from Oxide parsing the input before collecting Conjure solutions. The partial-function regression therefore retains numeric defaults and Conjure reference validation. This observation is separate from compound value defaults, which the new lookup fixtures exercise.

A fresh first-representation screen after the component-reference equality connection now loads `smt/matrix/2d-eq` and `smt/matrix/scalar-neq`. The three overlapping-index cases still fail with non-Boolean matrix references. Their unequal index domains need an explicit semantic lowering; matching flattened lengths alone is insufficient evidence to change equality semantics.

The existing overlapping-index Minion goldens contain zero equality assignments, eight direct-disequality assignments and sixteen negated-equality assignments. This is evidence that positional comparison or simple complementation would change current partial-expression behaviour. Establish actual-label and definedness semantics before lowering those three remaining cases.

The two Savile Row `tableshort` fixtures remain globally skipped. They use Essence Prime syntax and a three-dimensional matrix of position/value pairs, while the new native `shortTable` syntax uses sparse rows of tuple pairs. Supporting/importing that frontend shape is still a connection gap; the new wildcard regression does not establish that those existing fixtures now run.

Direct Conjure type-checking accepts the set default, but Conjure `3de9b2ef3` crashes during reference refinement with `Comprehension contains unsupported generators` over that set-valued `catchUndef`. The set fixture records the reference limitation and checks complete independently enumerated assignments. A reproducer/report is kept uncommitted under `bug-reports/conjure-set-catch-undef`; no upstream issue was submitted. This is not a RustSAT/Pindakaas encoder bug.

The general inverse rule still declines an empty compound matrix. The new masked function regression has a non-empty declared domain and covers absent queries/partial entries, not an empty compound domain. Keep empty-domain inverse handling on the feature frontier. Compound lookup types beyond the tuple/record/set/matrix/sequence regressions also require dedicated coverage.


## Confirmed Pindakaas 0.5.1 Tseitin constant-branch bug

The next library connection audit reproduces six wrong projected assignments through the public `TseitinEncoder` alone. Its Boolean-constant ITE simplifier reverses the remaining variable branch in three cases: true then/variable else, false then/variable else, and variable then/false else. Exhaustive enumeration of generated CNF agrees with `c or !b`, `!c and !b` and `c and !b`, respectively, rather than the original formulas. No Oxide adaptor or solver participates in the reproducer.

The report and standalone Cargo project are uncommitted under `bug-reports/pindakaas-0.5.1/tseitin-constant-branches`. No report has been submitted and no workaround introduced. This blocks the Pindakaas Tseitin provider; RustSAT Boolean generation remains active. It does not affect the newly connected Pindakaas AMO providers. Continue with other public-library options while preserving this blocker and the deferred mixed-representation/channelling work.

## Collection table frontend

`table` and `negativeTable` accept sets of ordered sequence rows. `shortTable` accepts a set of sparse rows, each a set of `(one-based position, value)` tuples. Matrices remain accepted for existing rectangular table models, but two-element matrices are not short-table pairs. Minion lowers ordinary tables to scalar inputs and numeric rows in `FlatTable`; sparse rows become Boolean combinations of specified equalities. SAT retains its table encoding decisions and variable row views.

The two legacy Savile Row short-table fixtures now use one set-valued `mycon` parameter and one `shortTable` call each. Independent enumeration of their original wildcard relations gives 6 and 92 solutions. Mixed SAT representations/channelling remain deferred.

## Native Minion short tables and canonical inputs

All modelling table constraints normalise to sequence inputs. Ordinary and negative relations use sets of sequence rows; short relations use sets of sets of tuple pairs. Rectangular matrix syntax is normalised before backend lowering. Minion's `FlatShortTable` uses zero-based positions and numeric values and maps to native `ShortStr2`. Variable-valued sparse cells become equality indicators appended to the native input list; constant sparse rows need no Boolean decomposition. Table inputs require discrete Minion variables. SAT retains the existing table encoding choices across every integer representation.
