# SAT adaptor

The adaptor owns the mapping from model names to SAT literals, clause generation and solution decoding. `SatEncodingDecision` records assertions, Boolean definitions, cardinality and weighted constraints, integer relations, objectives, allDifferent, tables and element operations. Algorithm selection lives in the rule engine, separate from those data types.

## Library providers

The `--sat-encoding-*` options pin an encoding family. Unpinned families are selected by the modelling heuristic. Exact labels and ordering are defined alongside the IR enums in `ast/sat_decision.rs`.

| Family | Providers and algorithms |
| --- | --- |
| AMO | RustSAT pairwise, ladder, bitwise, commander, bimander and two-product; Pindakaas pairwise, ladder and bitwise |
| Cardinality | RustSAT totalizer; Pindakaas sorting network |
| Pseudo-Boolean | RustSAT generalized totalizer, binary adder and dynamic poly watchdog; Pindakaas BDD and sequential weight counter |
| allDifferent | Oxide pairwise numeric disequalities or per-value AMO compositions |
| Table | Oxide tuple, shared-suffix MDD or binary-support compositions |
| Element | Oxide implication or support compositions |

The last three families compose library-backed numeric comparisons and Boolean operations. Structured Pindakaas inputs preserve guaranteed choice, chain and bounded-binary groups. Oxide validates those groups and normalises constants, signed coefficients, repeated literals and complements before calling a provider. A guarded equality path splits structured Pindakaas equality into two inequalities because direct equality can reject valid assignments in Pindakaas 0.5.1.

## Shared state and incremental solving

One `EncodingCache` belongs to a loaded solver. It retains Boolean gates and aliases, occurrence inputs, native cardinality and weighted counters, and fully equivalent threshold predicates. Incremental dominance constraints and objective expressions share that cache and the solver's allocation frontier. Repeated or opposite-polarity occurrences receive distinct equivalent variables when a provider requires unique inputs; their multiplicity is preserved.

An objective owns its native upper-bound encoder separately. Tightening extends that encoder and uses its enforcement literals as transient solve assumptions. In particular, dynamic poly watchdog control literals must not be permanently asserted across bounds. Pindakaas's one-shot objective constraints still share Boolean compilation state.

Stateless compiler contexts are retained inside certain cached-counter fallbacks to prevent recursively selecting the same caching path. A fresh cache is appropriate only when compiling an independent solver instance.
