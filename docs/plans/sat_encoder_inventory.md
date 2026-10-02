# SAT encoder inventory and reuse decisions

Verified on 2026-10-02 against RustSAT **0.7.5** (the workspace dependency) and the published Pindakaas **0.5.1** source. The crates.io index lists 0.5.1 as its latest published, non-yanked release. This inventory separates public encoders, adapters, representation views, and private implementation machinery. Availability here does not mean an Oxide adapter is implemented.

Sources: [RustSAT 0.7.5 source](https://docs.rs/crate/rustsat/0.7.5/source/src/encodings/), [Pindakaas 0.5.1 source](https://docs.rs/crate/pindakaas/0.5.1/source/src/), [Pindakaas version index](https://index.crates.io/pi/nd/pindakaas). The release source was inspected directly, rather than relying on README claims or development-branch documentation.

## Architectural boundary

The model AST records mathematical operands, occurrence identities, representation choices, selected algorithms, options, and provenance. A terminal decision AST contains no SAT literals, clauses, or library objects. Compilation allocates and shares representations, then dispatches the already selected algorithms into a library-owned clause collector or solver. Generated CNF is an adaptor artifact and can be exported as DIMACS; it never becomes a model field or expression.

The new `ast::encoding_plan` nodes are the first implementation of this boundary. `Model::sat_encoding` owns the decision arena. The SAT adaptor already accepts explicit Boolean decision ASTs and rejects mixed legacy inputs. Automatic production selection and migration of integer rules remain necessary before removing legacy `CnfClause` completely.

## RustSAT: concrete public algorithms

| Constraint | Algorithm / public type | Capabilities and likely Oxide uses |
| --- | --- | --- |
| At most one | `am1::Pairwise` | Direct representation uniqueness; small AMO constraints. Current default AMO. |
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
- `BoolLinExp::add_choice`, `add_chain`, and `add_bounded_log_encoding` attach AMO, chain, and domain information to weighted terms. These can help exploit representation structure, but Oxide must retain its own domain/channel invariants and decoding.

Source: [public integer views](https://docs.rs/crate/pindakaas/0.5.1/source/src/lib.rs), [linear conversion and structured terms](https://docs.rs/crate/pindakaas/0.5.1/source/src/bool_linear.rs).

`BoolLinAggregator` normalizes and specializes linear constraints into `BoolLinVariant::{Linear,Cardinality,CardinalityOne,Trivial}` and can emit clauses during aggregation. `StaticLinEncoder` selects component encoders; its published defaults use the adder for both general linear and cardinality, and bitwise for AMO. `LinearEncoder` combines aggregation and encoding. This is a pipeline, not another algorithm. If used, its specialization policy must be recorded; an explicit BDD choice must not silently become an unrecorded adder choice. [Linear dispatch source](https://docs.rs/crate/pindakaas/0.5.1/source/src/bool_linear.rs)

`integer::IntVarEnc`, `TernLeEncoder`, and `ImplicationChainEncoder` are private implementation machinery in 0.5.1. The `sorted` module contains direct, recursive, and mixed sorting strategies, but is itself private. Neither these encoders nor the named strategy options should be counted as directly configurable public Oxide adapters without upstream API work. Public linear encoders can use them internally. `SwcEncoder::with_propagation` and `TotalizerEncoder::with_propagation` also take the private-module `Consistency` enum: its Bounds/Domain variants cannot be named by an external adapter. Do not advertise these strengths as usable explicit options until that API is exposed; the default remains usable. [Integer source](https://docs.rs/crate/pindakaas/0.5.1/source/src/integer.rs), [sorting source](https://docs.rs/crate/pindakaas/0.5.1/source/src/sorted.rs)

`Encoder::encode_implied` provides a guarded constraint by guarding emitted clauses. This is implication, not full reification; equivalence requires encoding the reverse direction as well. [Encoder trait](https://docs.rs/crate/pindakaas/0.5.1/source/src/lib.rs)

## Recommendation: retain RustSAT and add Pindakaas as an encoding provider

Use RustSAT for the existing solver integration, variable allocation, clause storage and DIMACS output. Add Pindakaas selectively for sorting networks, BDD and SWC first; compare its totalizer and adder implementations after the common semantic interface is verified. Overlapping algorithms remain separate provider/algorithm choices for benchmarking. Do not switch solver libraries just to gain an encoder.

Use `pindakaas = { version = "=0.5.1", default-features = false }` when an adapter is ready. Its default enables its own CaDiCaL binding, which is unnecessary alongside the existing RustSAT binding. No Pindakaas dependency is added by this investigation.

Implement a private Pindakaas `ClauseDatabase` backed by the RustSAT collector and the same authoritative allocator. It must reserve every semantic, representation, channel and auxiliary variable from that allocator. Pindakaas literals use signed nonzero, one-based variable numbers; RustSAT's variable indices are zero-based. Translate signs and indices explicitly, enforce both libraries' identifier limits, handle zero-length ranges, and preserve an empty clause when Pindakaas reports `Unsatisfiable`. Avoid a second independently allocating CNF buffer.

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
