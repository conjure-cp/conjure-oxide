# SAT encoder inventory and reuse decisions

Verified on 2026-10-02 against RustSAT **0.7.5** (the workspace dependency) and the published Pindakaas **0.5.1** source. The crates.io index lists 0.5.1 as its latest published, non-yanked release. This inventory separates public encoders, adapters, representation views, and private implementation machinery. Availability here does not mean an Oxide adapter is implemented.

Sources: [RustSAT 0.7.5 source](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/), [Pindakaas 0.5.1 source](https://docs.rs/crate/pindakaas/0.5.1/source/src/), [Pindakaas version index](https://index.crates.io/pi/nd/pindakaas). The release source was inspected directly, rather than relying on README claims or development-branch documentation.

## Architectural boundary

The model AST records mathematical operands, occurrence identities, representation choices, selected algorithms, options, and provenance. A terminal decision AST contains no SAT literals, clauses, or library objects. Compilation allocates and shares representations, then dispatches the already selected algorithms into a library-owned clause collector or solver. Generated CNF is an adaptor artifact and can be exported as DIMACS; it never becomes a model field or expression.

The new `ast::encoding_plan` nodes are the first implementation of this boundary. `Model::sat_encoding` owns the decision arena. The SAT adaptor accepts explicit Boolean decision ASTs and rejects mixed terminal payloads. Production gates, AMO, cardinality and weighted pseudo-Boolean constraints use `Model::sat_decisions`; all generated clauses belong to the adaptor. `CnfClause` and the model CNF fields have been removed.

## RustSAT: concrete public algorithms

| Constraint | Algorithm / public type | Capabilities and likely Oxide uses |
| --- | --- | --- |
| At most one | `am1::Pairwise` | Direct representation uniqueness; small AMO constraints. Compact selects it through five inputs. |
| At most one | `am1::Ladder` | Alternative for direct uniqueness and flattened AMO. |
| At most one | `am1::Bitwise` | Logarithmic selector alternative. This is an AMO algorithm, not BinaryValue integer representation. |
| At most one | `am1::Commander<N, Sub>` | Grouping algorithm; group size and sub-encoder must be recorded in the decision. |
| At most one | `am1::Bimander<N, Sub>` | Grouping/binary selector algorithm; record group size and sub-encoder. |
| At most one | `am1::TwoProduct<Sub>` | Two-product decomposition; record sub-encoder. |
| Cardinality | `card::Totalizer` | Upper, lower, both bounds; incremental bounds. Direct exactly-one combines AMO with at-least-one; general cardinality covers occurrence counts and set sizes. |
| Pseudo-Boolean | `pb::GeneralizedTotalizer` | Native upper bound; incremental upper bounds. Weighted sums of representation literals. |
| Pseudo-Boolean | `pb::BinaryAdder` | Upper, lower, both bounds and incremental bound interfaces. Alternative for larger weighted sums. |
| Pseudo-Boolean | `pb::DynamicPolyWatchdog` | Upper bounds, incremental bounds and precision. Adding inputs after encoding resets its encoded state; do not treat this as persistent input extension. |

Sources: [AMO module](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/am1.rs), [cardinality module](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/card.rs), [PB module](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/pb.rs), [DPW implementation](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/pb/dpw.rs). Exactly-one, lower-bound inversion, and equality composition are capabilities/compositions, not additional independent algorithms.

## RustSAT: adapters, atomic operations, certification

| Facility | Public entry points | Reuse / distinction |
| --- | --- | --- |
| Cardinality inversion | `card::simulators::Inverted<CE>` | Negated inputs turn upper bounding into lower bounding. |
| Cardinality two-sided composition | `card::simulators::Double<UBE,LBE>` | Builds both bounds from separate bound encoders. |
| PB inversion | `pb::simulators::Inverted<PBE>`; `InvertedGeneralizedTotalizer` alias | Lower-bound support around an upper-bound encoder. |
| PB two-sided composition | `pb::simulators::Double<UBE,LBE>`; `DoubleGeneralizedTotalizer` alias | Both bounds/equality from two encoders. Default PB both-bounding uses this around GTE. |
| PB via cardinality | `pb::simulators::Card<CE>` | Repeats a literal according to its weight. Record expansion cost; avoid enormous weights. |
| Implication primitives | `lit_impl_lit`, `lit_impl_clause`, `lit_impl_cube`; `cube_impl_lit`, `cube_impl_clause`, `cube_impl_cube`; `clause_impl_lit`, `clause_impl_clause`, `clause_impl_cube` | Nine `atomics` operations over a literal, conjunction, or disjunction. Reuse for gates and channels. The Boolean decision compiler now uses these helpers for AND/OR equivalences. |
| Guard / reverse implication | `lit_impl_card`, `card_impl_lit`, `lit_impl_pb`, `pb_impl_lit` | Four helpers transform an implication into a PB constraint. Full equivalence needs both directions with correct boundary cases. |
| Certified encodings | `card::cert`, `pb::cert`, gated by `proof-logging` | Totalizer and GTE certified bound interfaces, associated inversion/composition and default wrappers. These are proof-producing forms of existing algorithms, not a new encoder family. |
| Default wrappers | `encode_cardinality_constraint`, `default_encode_cardinality_constraint`, `encode_pb_constraint`, `default_encode_pb_constraint`; `new_default_*`, `Def*` aliases | Convenience dispatch, not new algorithms. Resolve defaults into explicit algorithm decisions for reproducibility. |
| Internal tree databases | `nodedb`, `totdb`, `_internals` | Construction machinery, not independent choices in a user-facing encoding portfolio. |

Sources: [cardinality simulators](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/card/simulators.rs), [PB simulators](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/pb/simulators.rs), [atomic operations](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/atomics.rs), [certified cardinality](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/card/cert.rs), [certified PB](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/pb/cert.rs).

## Pindakaas: concrete public algorithms

| Constraint | Algorithm / public type | Capabilities and likely Oxide uses |
| --- | --- | --- |
| Propositional logic | `propositional_logic::TseitinEncoder` | AND, OR, NOT, XOR, equivalence, implication, if-then-else; formulas over literals or Boolean constants. Alternative to our small Boolean traversal. |
| At most / exactly one | `cardinality_one::PairwiseEncoder` | Direct representation uniqueness and one-of constraints. |
| At most / exactly one | `cardinality_one::LadderEncoder` | Same constraint family, alternative algorithm. |
| At most / exactly one | `cardinality_one::BitwiseEncoder` | Same constraint family, binary selector algorithm. |
| Cardinality | `cardinality::SortingNetworkEncoder` | Adds a sorting-network alternative absent from the RustSAT 0.7.5 cardinality module. |
| Pseudo-Boolean | `bool_linear::AdderEncoder` | Binary arithmetic alternative; also usable through cardinality/AMO adapters. |
| Pseudo-Boolean | `bool_linear::BddEncoder` | Adds a BDD alternative; consistency and cutoff configuration. |
| Pseudo-Boolean | `bool_linear::SwcEncoder` | SWC (source describes a Sorted Weight Counter); consistency and cutoff configuration; see the private propagation-type limitation below. |
| Pseudo-Boolean | `bool_linear::TotalizerEncoder` | Generalized totalizer; consistency and cutoff configuration; see the private propagation-type limitation below. Distinct implementation from RustSAT GTE. |

Sources: [propositional logic](https://docs.rs/crate/pindakaas/0.5.1/source/src/propositional_logic.rs), [cardinality-one](https://docs.rs/crate/pindakaas/0.5.1/source/src/cardinality_one.rs), [cardinality](https://docs.rs/crate/pindakaas/0.5.1/source/src/cardinality.rs), [Boolean linear](https://docs.rs/crate/pindakaas/0.5.1/source/src/bool_linear.rs).

The repository README advertises **Product encoding** for AMO, but the 0.5.1 `cardinality_one` module has no public product encoder. Do not add a Pindakaas-product choice to the release-based registry until a usable upstream implementation is verified. RustSAT's `TwoProduct` is available now. [README](https://github.com/pindakaashq/pindakaas)

## Pindakaas: representation views and supporting machinery

- Public `IntEncoding::Direct { first, vals }` describes consecutive values. Sparse Direct must use explicitly weighted choices; passing a sparse domain as consecutive values is incorrect.
- Public `IntEncoding::Order { first, vals }` describes consecutive greater-than thresholds. Sparse Order needs weighted threshold gaps, with the polarity adapted to Oxide's selected convention.
- Public `IntEncoding::Log { signed, bits }` is actual-value binary semantics. Conversion code assigns ascending powers of two and negates the highest weight when signed; documentation wording about the sign-bit position is inconsistent, so test the adapter against the implementation. It supplies no independent BinaryOffset or BinaryRank representation. Offset needs an explicit additive constant; rank must never substitute for numeric value in arithmetic.
- `BoolLinExp::add_choice`, `add_chain`, and `add_bounded_log_encoding` attach AMO, chain, and domain information to weighted terms. Oxide records these groups in PB and integer relation decisions and forwards compatible groups to BDD/SWC, including both directions of integer relation reification, while retaining its own domain constraints and decoding.

Source: [public integer views](https://docs.rs/crate/pindakaas/0.5.1/source/src/lib.rs), [linear conversion and structured terms](https://docs.rs/crate/pindakaas/0.5.1/source/src/bool_linear.rs).

`BoolLinAggregator` normalizes and specializes linear constraints into `BoolLinVariant::{Linear,Cardinality,CardinalityOne,Trivial}` and can emit clauses during aggregation. `StaticLinEncoder` selects component encoders; its published defaults use the adder for both general linear and cardinality, and bitwise for AMO. `LinearEncoder` combines aggregation and encoding. This is a pipeline, not another algorithm. If used, its specialization policy must be recorded; an explicit BDD choice must not silently become an unrecorded adder choice. [Linear dispatch source](https://docs.rs/crate/pindakaas/0.5.1/source/src/bool_linear.rs)

`integer::IntVarEnc`, `TernLeEncoder`, and `ImplicationChainEncoder` are private implementation machinery in 0.5.1. The `sorted` module contains direct, recursive, and mixed sorting strategies, but is itself private. Neither these encoders nor the named strategy options should be counted as directly configurable public Oxide adapters without upstream API work. Public linear encoders can use them internally. `SwcEncoder::with_propagation` and `TotalizerEncoder::with_propagation` also take the private-module `Consistency` enum: its Bounds/Domain variants cannot be named by an external adapter. Do not advertise these strengths as usable explicit options until that API is exposed; the default remains usable. [Integer source](https://docs.rs/crate/pindakaas/0.5.1/source/src/integer.rs), [sorting source](https://docs.rs/crate/pindakaas/0.5.1/source/src/sorted.rs)

`Encoder::encode_implied` provides a guarded constraint by guarding emitted clauses. This is implication, not full reification; equivalence requires encoding the reverse direction as well. [Encoder trait](https://docs.rs/crate/pindakaas/0.5.1/source/src/lib.rs)

## Recommendation: retain RustSAT and add Pindakaas as an encoding provider

Use RustSAT for the existing solver integration, variable allocation, clause storage and DIMACS output. Add Pindakaas selectively for sorting networks, BDD and SWC first; defer its totalizer and adder implementations because these algorithm families are already supplied by RustSAT. Their CNF and performance can differ, so provider comparisons remain possible future benchmarking work. Do not switch solver libraries just to gain an encoder.

The workspace now uses `pindakaas = { version = "=0.5.1", default-features = false }`. Its default enables its own CaDiCaL binding, which is unnecessary alongside the existing RustSAT binding. Its sorting-network cardinality encoder is available through `--sat-encoding-cardinality=pindakaas-sorting-network`; `rustsat-totalizer` selects the RustSAT provider.

The private Pindakaas `ClauseDatabase` bridge is backed by the RustSAT instance and its authoritative allocator. Every semantic, representation and auxiliary variable shares that allocator. Pindakaas literals use signed nonzero, one-based variable numbers; RustSAT's variable indices are zero-based. The bridge translates signed identifiers, handles zero-length ranges, and preserves an empty clause when Pindakaas reports `Unsatisfiable`. There is no second independently allocating CNF buffer.

Cardinality decisions cover asserted upper, lower and exact bounds. Repeated or complementary operand occurrences retain their multiplicity; the Pindakaas provider allocates equivalent proxy literals before normalisation so weighted aggregation cannot silently select a different encoder. Exhaustive projected-solution tests cover both providers, constants and boundary bounds. All-mode selects one algorithm per encoding class across the model; explicit pins override heuristic selection.

Weighted decisions now expose `--sat-encoding-pb` with `rustsat-generalized-totalizer`, `rustsat-binary-adder`, `pindakaas-bdd`, `rustsat-dynamic-poly-watchdog` and `pindakaas-swc`. Upper/lower/exact bounds share signed coefficient, constant and complement normalisation. RustSAT uses selected native bound interfaces directly, including reusable GTE/adder upper and inverted-lower counters; Pindakaas BDD handles linear and specialised cardinality variants explicitly, avoiding a silent provider switch. Compact uses GTE for coefficient sums up to 4096 and the adder above that as an initial policy; all-mode enumerates all five. RustSAT DPW uses a native upper-bound encoder and an inverted copy for lower bounds, with full precision (divisor one). Exact constraints use both. Pindakaas SWC uses the same explicit aggregation and specialisation path as BDD, retaining the selected algorithm.

Integer linear views preserve actual values: Direct uses each value in the domain span as its coefficient, Order uses its minimum plus the non-anchor threshold bits, and BinaryValue uses powers of two with a negative sign-bit coefficient. Structural domain constraints continue to exclude sparse-domain gaps. BinaryOffset (`IntOffset`, `int_offset`) adds its minimum to unsigned weighted bits. BinaryRank (`IntRank`, `int_rank`) stores a sorted-domain index; sparse rank is mapped to actual values before numeric encoding. The mapping uses canonical intervals rather than expanding domain gaps. Structural constraints exclude unused unsigned codes, including singleton code one. Both representations support solution encoding/decoding and uniform family selection.

All six RustSAT AMO algorithm families are available. Commander and bimander currently partition inputs into groups of up to four literals (the last group may be smaller) and use their default pairwise sub-encoder; two-product also uses pairwise. Alternative grouping parameters/sub-encoders and Pindakaas's public pairwise, ladder and bitwise providers remain unwired.

Both libraries support clause sinks: RustSAT `CollectClauses` with `ManageVars`, and Pindakaas `ClauseDatabase` with allocation in `new_var_range`. This is the bridge boundary; their literal types never escape into the AST. [RustSAT collector](https://docs.rs/rustsat/0.7.5/rustsat/encodings/trait.CollectClauses.html), [Pindakaas database](https://docs.rs/crate/pindakaas/0.5.1/source/src/lib.rs), [Pindakaas features](https://docs.rs/crate/pindakaas/0.5.1/source/Cargo.toml)

The inventory establishes API availability, not comparative speed or propagation guarantees. Benchmark clause count, auxiliary count, translation time and solving time separately on the uniform portfolio. Check bounded exhaustive semantic projection before activating an adapter. RustSAT's incremental interfaces are useful for objective tightening; Pindakaas's generic one-shot encoder interface does not promise reusable incremental bound state.

## Implementation order

1. Keep modelling decisions in the AST: decomposition choice, representation requests and explicit channelling policy. Keep encoding decisions there too: semantic operands, algorithm/provider/options, assertion or reification mode and provenance. Library compilation must not choose these silently.
2. Migrate Boolean selection to decision nodes; reuse RustSAT atomic helpers immediately. This helper reuse and explicit Boolean adaptor input are implemented.
3. Add cardinality/AMO and PB decision nodes, explicit normalization and library capability validation. Reuse RustSAT AMO, totalizer, GTE and adder rather than rebuilding them as expression trees.
4. Add the Pindakaas sink bridge and differential tests, then sorting-network, BDD and SWC provider choices.
5. Lower integer representations to literals plus weighted semantic views; keep BinaryValue, BinaryOffset and BinaryRank separate. Add checked coefficient/constant arithmetic and sparse membership restrictions before library dispatch.
6. Migrate all remaining integer, channel, objective and dominance paths. Remove `Model`/`SerdeModel` CNF fields, `RuleEffect::new_clauses`, `CnfClause`, clause extraction/rewrite logic, and the legacy converter together. Encoding decisions replace clause-producing rules; renamed clause nodes do not meet this boundary.
7. Split test artifacts into stable decision-AST snapshots and adaptor DIMACS/statistics. Continue validating semantic solutions across every uniform representation. No generated CNF should be serialized as a model AST golden.

## Structured linear inputs

Production PB decisions retain contiguous Direct choice groups, Order implication chains and weighted BinaryValue/Offset bounds. Ordinary Boolean occurrences remain free terms; sparse Rank is still mapped to actual values before arithmetic. RustSAT receives the same flattened mathematical sum.

The Pindakaas bridge aggregates repeated/complementary terms with widened arithmetic before assigning each variable to at most one group. Choice and chain groups retain signed coefficients and canonical positive input polarities. Bounded binary groups are used only when their occurrence weights remain intact. Signed bits and negative multipliers are transformed to unsigned groups with equivalent complemented-bit proxies; bounds and constants are adjusted, and lower-bound constraints are inverted before library aggregation. Structured equality is encoded as upper and lower inequalities through the selected provider: the 0.5.1 direct equality path rejects some valid choice assignments. Incompatible groups fall back to free terms. A range guard also accounts for the extra coefficient sum introduced by negative-weight choice normalisation.

Pindakaas 0.5.1 does not fully exploit binary bounds: its default internal conversion expands domain groups into individual bits, with the binary-to-order coupling disabled. Passing bounds preserves the available interface; it does not establish a performance improvement. Choice and chain groups are used directly by the encoders.

## allDifferent compositions

Oxide now exposes `--sat-encoding-alldifferent pairwise|value-amo`. Pairwise composes actual-value integer disequalities using the selected PB provider, with Direct choice specialisations using library Boolean gates. Value-AMO groups Direct/Boolean value indicators and uses the selected RustSAT AMO encoder for assertions. Nested and negated uses compose equivalent PB count bounds with library Boolean gates. These are Oxide semantic compositions of public library encoders, not native allDifferent algorithms advertised by either library. Compound-valued operands and allDifferentExcept remain follow-ups.

## Table compositions

The pinned RustSAT 0.7.5 and Pindakaas 0.5.1 public APIs contain no dedicated table/MDD encoder. `--sat-encoding-table tuple|mdd` therefore selects semantic compositions over the existing library-backed integer equalities and RustSAT Boolean gates. Tuple shares column/value comparisons and combines matching rows. MDD shares identical suffix relations at each layer. Both retain actual numeric values and support positive, negative and reified relations. PB component selection applies across all integer representations; compatible Direct cell comparisons use the existing library-gate specialisation. Binary-support tables and short tables remain separate follow-ups.

## Element compositions

The pinned public APIs provide no dedicated element encoder. Oxide exposes `--sat-encoding-element implication|support` over library numeric equalities and RustSAT Boolean gates. Implication asserts each valid index selector implies the corresponding value equality. Support combines selector/equality matches, allowing an out-of-domain index to leave the value unconstrained. Existing bubbling rules determine unsafe lookup definedness, including nested and masked expressions. Matrix labels retain their actual values, including sparse, offset and Boolean domains; variable and constant scalar entries share the selected PB provider. BinaryRank uses actual-value mapping. Constant indices continue through existing simplification rules. Compact initially prefers support above four entries; this is a policy to evaluate, not a performance claim. Identity-default `ElementId` shares these compositions via guarded indexing and an index-valued fallback. Boolean entries become numeric before the lookup; empty matrices return the index. This is scalar selection, distinct from the internal function-domain inverse `IndexOf` operation. Constant scalar `IndexOf` matrices now invert their value/label mapping and reuse `ElementId`, including sparse labels and numeric Boolean values. Compound-valued and non-constant inverse lookups remain follow-ups.

Scalar lexicographic comparisons expand sequence order into actual-value equalities/strict comparisons and Boolean gates, reusing the connected numeric relation providers. Neither pinned library advertises a dedicated lexicographic encoder.

Floor modulo now shares Oxide's existing restoring division circuit. The remainder's floor semantics are expressed with the same RustSAT gate decisions; no new clause encoder or selectable library family is introduced. All five integer representations convert through actual-value binary operands, while surrounding numeric relations retain the selected PB provider. Zero-divisor definedness remains in the model's bubble guards.

Asserted AMO/cardinality/PB selectors also consume ready leaves inside literal root conjunctions, without flattening the worklist. These use the same library algorithms and CLI decisions. Boolean comparisons beneath negation, disjunction, implication or reification are not asserted constraints. Ready counts in those contexts, including `toInt`, use output-bearing `CountRelation` decisions. Both implication directions compose the selected cardinality provider; upper thresholds of one use the selected AMO provider. Guarded clause collection reuses all six RustSAT AMO algorithms, RustSAT totalizer and Pindakaas sorting network. Constants, repeated and opposite literals retain their count semantics. Weighted and numeric comparisons continue to use the selected PB provider. PB objective tightening and cardinality bounds now reuse native RustSAT state; repeated cardinality inputs also share counters across dominance updates.


## Reusable objective bounds

Ordinary optimisation now produces a semantic actual-value `Objective` decision with the shared PB provider. RustSAT GTE, binary adder and DPW retain native state across strictly improving bounds, using `encode_ub_change` and replacing `enforce_ub` assumptions for each solve. Generated structural clauses persist; obsolete enforcement literals do not. In particular, DPW bound-control bits cannot be made permanent across tightening steps. Pindakaas BDD/SWC use their one-shot interfaces, retaining compatible structured views. Maximisation normalises to a minimised signed cost. Cardinality bounds now retain state across dominance updates. Weighted dominance counters now share GTE/adder native state and bound-specific predicates; see the weighted sharing section below.


## Shared cardinality bounds

The SAT adaptor now retains a solver-local input cache across initial compilation and dominance updates. RustSAT totalizers extend native upper/lower bound encodings lazily; structural clauses persist independently of enforcement guards. Pindakaas's public sorting-network API remains one-shot, so fully equivalent threshold predicates are cached for repeated bounds and complementary lower bounds. A different threshold still creates a separate native network. Equality combines upper/lower enforcement; repeated/opposite occurrences use shared equivalent aliases. Upper thresholds of one continue to use the selected AMO provider. Weighted dominance sharing is described below; objective tightening retains its separate solve-time state.


## Shared weighted dominance bounds

The solver-local encoding cache spans initial compilation and dominance updates. GTE and binary adder retain native upper and inverted-lower counters for canonical signed sums. Structural clauses are unconditional; bound enforcement is guarded. Different bounds can coexist, and repeated reified thresholds reuse their complete equivalence predicates.

The provisional reuse policy limits reified GTE native sharing to 64 weighted inputs. Larger predicates retain library atomic implication transformations and share repeated predicates. Binary adder reification retains native sharing without that limit. Unconditional assertions preserve heavy-term pruning first; GTE/adder assertions above 64 remaining inputs retain their original library BoundBoth construction. Provider selection is unchanged. Cost-aware tuning remains open.

DPW control literals cannot safely enforce different bounds simultaneously on one counter. Its dominance predicates own bound-specific native encodings; repeated thresholds share the predicate. Ordinary optimisation instead replaces obsolete DPW assumptions between solves. Pindakaas BDD/SWC reuse equivalent threshold predicates through their public one-shot interfaces, retaining choice, chain and bounded-binary inputs. Different thresholds still create separate native encodings for these providers.

Structured cache keys preserve group invariants and canonical coefficients, ignoring group order and constant spelling. Complementary signed sums share opposite predicate polarities. Choice/binary member order is immaterial; chain implication order remains significant. Encoder state and CNF stay outside the model.

The sparse weighted regression returns five independently enumerated assignments across all integer/PB choices. Direct, Order, BinaryValue and BinaryOffset record reuse for every provider. Sparse BinaryRank is semantically correct but creates fresh actual-value arithmetic auxiliaries during dominance rewrites, preventing reuse. Sharing those numeric projections remains an Oxide arithmetic-view/Boolean-arena connection, not an upstream encoder bug.

Unrestricted native sharing caused large Order/GTE searches to exceed 706 seconds. The final policy restores the three complete wide-domain fixtures to 63-65 seconds, close to their previous 65-72-second baselines. Six focused integration checks pass, including large Direct addition and the dominance RFC example. This is a performance observation for the construction, not a library correctness bug.
