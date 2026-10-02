# SAT IR implementation status

Branch: `sat-ir`. Conventional commits; never push.

## Decisions

- Integer kinds: Direct, Order, BinaryValue (actual numeric value), BinaryOffset (value minus minimum), BinaryRank (sorted-domain index).
- `channelling=uniform` selects one representation kind per outer type family throughout a model, with no channelling. Integer encoding and matrix layout are independent.
- Test available uniform combinations with the integration tester's `heuristic=x`.
- Modelling/encoding decisions belong to the model AST; generated clauses belong only to the SAT adaptor/library. The production model no longer contains CNF.
- Existing `no`/`yes` settings and test defaults remain compatible; enable uniform explicitly in the expanding SAT corpus.

## Implemented

- Uniform CLI/config setting, family choice reuse, initialisation conflict guard, rewrite-scoped choices, and speculative choice rollback. Detached representation probes cannot commit choices.
- Integer encoding capability metadata distinguishes whole-matrix SMT integer theory from matrix layout.
- Unit tests for unequal domains, differing set element types, incompatible domains, speculative effects, and scope isolation.
- Sparse/negative/unequal integer-domain integration fixture: all three current SAT integer representations produce the same five solutions.
- SAT preserves residual false constraints and empty clauses; the out-of-domain `int_order` regression has no solutions for every representation.
- Validated, serialisable `ast::encoding_plan` decision nodes owned by `Model::sat_encoding`, independent of literal allocation. Existing declaration identities are interned as semantic variables; fresh reference IDs distinguish individual uses. Domains are canonicalised without enumerating intervals. Representation requests and encoding provenance are explicit.
- The SAT adaptor loads explicit terminal Boolean decision ASTs and rejects mixed terminal payloads, residual constraints, missing/stale variables, objectives and dominance. Non-SAT adaptors reject SAT decisions. Model serialization, cloning, hashing and stable-ID collection retain the decision nodes.
- A first generator realises explicitly selected Boolean Tseitin plans directly as RustSAT clauses, reusing RustSAT atomic gate helpers. Literal types stay private to the adapter. Exhaustive assignment tests check semantic projection, repeated uses, constants, empty conjunctions/disjunctions, and deterministic clause output.
- Uniform SAT portfolios enabled in 41 existing CNF fixture directories (97 SAT portfolios), with reference solution checks and normal golden verification.

- Production Boolean and integer gate rules now emit semantic `SatEncodingDecision` assertions/definitions through `RuleEffect::sat`. Model CNF fields, `CnfClause` and the legacy clause converter have been removed. RustSAT generates all clauses at load time; dominance injection uses the same allocation frontier as encoder auxiliaries.
- Asserted AMO decisions carry a selectable algorithm and explicit/heuristic provenance. `--amo-encoding` pins pairwise, ladder, bitwise, commander, bimander or two-product. Otherwise the existing first/random/compact/interactive/all heuristics choose one AMO algorithm per model. Compact uses pairwise through five inputs and ladder above that; this is an initial policy, not a performance claim.
- Direct integer domain constraints and direct division quotient constraints retain AMO semantics until library generation. All-mode integration portfolios enumerate encoder choices alongside uniform representation choices.
- `EncodingSelection<T>` records a resolved algorithm and its provenance without tying the choice container to AMO. Pindakaas providers and the other encoding classes remain subsequent work.

## Coverage investigation

- Production SAT integer representations remain IntDirect, IntOrder, and IntLog (two's-complement actual value). BinaryOffset and BinaryRank currently exist only as IR kinds.
- An initial short CLI survey of 45 CNF inputs found six timeouts and the dropped residual false regression. The premature probe-choice bug meant this survey did not exercise all integer choices; integration portfolio enumeration supersedes its apparent coverage.
- Longer acceptance and normal regression runs enabled comparison/sparse_direct, comparison/sparse_log, int_direct/06-add, and integer/05-product. The integer/10-div and neg-div fixtures still exceeded a 180-second per-test limit and retain their previous solver configuration. These timeouts do not establish unsupported semantics.
- cnf/cnf2 contains two Essence files, so the current integration test discovery skips that directory; split these inputs before enabling them.

## Outstanding

- Extend the decision mechanism from the AMO proof of concept to cardinality, pseudo-Boolean and other encoding classes, with explicit user overrides and heuristic selection.
- Extend plans beyond Boolean Tseitin and implement shared representation materialisation, decoding, and domain constraints.
- Add BinaryOffset and BinaryRank; expose the existing actual-value encoding as BinaryValue in the new production pipeline.
- Add lazy multi-representation materialisation, channelling, and exhaustive semantic tests before enabling mixed representations.
- The versioned [RustSAT/Pindakaas inventory](sat_encoder_inventory.md) covers public algorithms, compositions, integer views and internal-only machinery. Implement the Pindakaas clause-sink bridge and controlled provider choices; no new dependency has been added yet.
- Expand verified portfolios beyond the CNF corpus and investigate the remaining slow fixtures.

## Validation

- Core: 218 unit tests passed, including decision-AST serialization and direct solving without model clauses. Rules: 128 passed. Tester: 7 passed.
- Final CNF group verification passed all 43 discovered tests with `TEST_CASE_TIMEOUT=180` (about 90 seconds after moving decisions into the AST). Every enabled SAT portfolio was accepted against reference solutions and passed normal golden verification. The new unequal-domain fixture also passed its final regression run.
- Production Clippy (`--lib --bins -- -D warnings`) and formatting checks pass. Strict all-target Clippy is blocked by existing test lints in unchanged code (redundant clones, useless conversions, items after test modules, unnecessary mutable arguments, and a useless vector).
- The sparse/negative/unequal-domain fixture adds three SAT portfolios beyond the 97 in the existing CNF corpus.
