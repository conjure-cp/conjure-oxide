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
- The initial uniform SAT expansion enabled 41 existing CNF fixture directories plus the unequal-domain fixture. That stage contained 245 uniform SAT portfolios across these 42 fixtures, including all six AMO choices where applicable. Reference solutions are checked before accepting goldens.

- Production Boolean and integer gate rules now emit semantic `SatEncodingDecision` assertions/definitions through `RuleEffect::sat`. Model CNF fields, `CnfClause` and the legacy clause converter have been removed. RustSAT generates all clauses at load time; dominance injection uses the same allocation frontier as encoder auxiliaries.
- Asserted AMO decisions carry a selectable algorithm and explicit/heuristic provenance. `--amo-encoding` pins pairwise, ladder, bitwise, commander, bimander or two-product. Otherwise the existing first/random/compact/interactive/all heuristics choose one AMO algorithm per model. Compact uses pairwise through five inputs and ladder above that; this is an initial policy, not a performance claim.
- Direct integer domain constraints and direct division quotient constraints retain AMO semantics until library generation. All-mode integration portfolios enumerate encoder choices alongside uniform representation choices.
- `EncodingSelection<T>` records a resolved algorithm and its provenance without tying the choice container to AMO. Cardinality and weighted PB reuse that selection container.

- Asserted cardinality decisions retain upper/lower/exact bounds and occurrence multiplicity. `--cardinality-encoding` selects RustSAT totalizer or Pindakaas sorting network. Heuristics resolve one cardinality choice per model; compact initially selects totalizer.
- Pindakaas 0.5.1 is pinned with its default solver features disabled. Its clause sink shares the RustSAT allocator, including every encoder auxiliary. Constants and impossible/trivial bounds are normalised before dispatch.

- Asserted pseudo-Boolean decisions retain signed weights, upper/lower/exact bounds and selected provider provenance. `--pb-encoding` pins RustSAT generalised totalizer, RustSAT binary adder, Pindakaas BDD, RustSAT dynamic polynomial watchdog or Pindakaas SWC. Explicit overrides take precedence; all-mode shares one choice per model. Compact initially uses GTE through a coefficient sum of 4096 and the adder above that.
- Linear extraction uses Direct, Order and BinaryValue views, retaining numeric values for sparse domains. Checked coefficient arithmetic precedes library dispatch; constants, repeated/complementary literals, negative weights and trivial bounds are normalised in the adaptor. Library-range overflow is reported rather than wrapped.
- Signed BinaryValue multiplication includes sign extension. Division bounds and circuits follow Essence floor semantics, including negative operands and minimum-value magnitudes. The three previous solution mismatches now pass uniform portfolios.

## Original decision family catalogue

The copied architecture plan named Boolean/Tseitin, AMO, cardinality, pseudo-Boolean, linear integer, allDifferent, table, element, integer equality and integer comparison plans. The remaining dedicated choices are linear-integer strategies (including order-tree and mixed forms), allDifferent decompositions, table encodings (tuple/MDD/binary support), element, equality/disequality and comparisons. Reification/guards, domain membership, channelling, objective tightening and proof-producing variants are supporting capabilities across these families.

## Coverage investigation

- Production SAT integer representations are IntDirect, IntOrder, IntLog (two's-complement actual value), IntOffset (unsigned displacement from minimum) and IntRank (unsigned sorted-domain index).
- An initial short CLI survey of 45 CNF inputs found six timeouts and the dropped residual false regression. The premature probe-choice bug meant this survey did not exercise all integer choices; integration portfolio enumeration supersedes its apparent coverage.
- Longer acceptance and normal regression runs enabled comparison/sparse_direct, comparison/sparse_log, int_direct/06-add, and integer/05-product. At that stage, integer/10-div and neg-div exceeded a 180-second per-test limit. The later cardinality coverage expansion enables integer/10-div; the later signed arithmetic fixes enable neg-div as well.
- cnf/cnf2 contains two Essence files, so the current integration test discovery skips that directory; split these inputs before enabling them.

- The [cardinality coverage survey](sat_coverage_survey.md) initially tried all 489 previously SAT-disabled runnable fixtures. That stage enabled 191 existing fixtures plus a new cardinality fixture: 322 SAT-enabled fixtures out of 620 runnable, with 1,931 SAT portfolios (1,836 uniform portfolios across 234 fixtures). All enabled SAT run records are successful.
- A BinaryValue division-bound fix removes panics for zero-containing divisor intervals and checks quotient extrema without overflowing. Nine additional division fixtures now pass all uniform portfolios.

## Outstanding

- Extend the decision mechanism to the remaining encoding classes, with explicit user overrides and heuristic selection.
- Extend decisions to the other encoding classes and implement shared representation materialisation, decoding, and domain constraints. Unify the explicit Boolean arena and production gate/AMO payloads as those plans are migrated.
- Expose the existing actual-value `IntLog` naming as BinaryValue in the new production pipeline; BinaryOffset and BinaryRank are implemented.
- Add lazy multi-representation materialisation, channelling, and exhaustive semantic tests before enabling mixed representations.
- The versioned [RustSAT/Pindakaas inventory](sat_encoder_inventory.md) covers public algorithms, compositions, integer views and internal-only machinery. The Pindakaas clause-sink bridge and cardinality provider choices are implemented; weighted PB now has GTE, adder, BDD, DPW and SWC providers. Pindakaas adder and totaliser are deferred as overlapping algorithm families; alternate provider implementations and tuning remain future work.
- Expand verified portfolios beyond the CNF corpus and investigate the remaining slow fixtures.

## Validation history

- The requested `NEXTEST_TEST_THREADS=4 make test-accept` passed all 1,491 workspace tests, with 14 skipped. Workspace doctests also passed.
- After the final AMO lowering guard, focused acceptance and normal golden verification each passed 574 tests: core/rule tests and all 130 fixture directories with existing SAT goldens. Core library: 220 unit tests; rules: 129 unit tests.
- Exhaustive AMO checks cover all six algorithms, sizes zero through nine (including group boundaries), every input assignment, constants and repeated inputs. Tests also cover global user overrides, one all-mode choice per model, unresolved-decision rejection and postponing extraction until Boolean operands are lowered.
- Production Clippy and formatting checks pass. Strict workspace Clippy additionally encounters an existing needless-borrow lint in `fuzz/fuzz_targets/fuzz_detect_errors.rs`; that unrelated file is unchanged.
- Acceptance artefacts are committed separately from code. The contribution guide's cleanup script discarded 493 timing-only files; existing timing fields were preserved in the 130 files with semantic statistics changes. Unrelated non-SAT golden deletions are excluded.

- Cardinality-stage verification: full workspace acceptance passed 1,497 tests with 14 skipped, plus all workspace doctests. Core library: 222 unit tests; rules: 131. Exhaustive cardinality checks cover both providers, upper/lower/exact bounds, constants, repeated/complementary operands and the shared allocation frontier. Choice tests cover group sharing, explicit overrides and retained node pins.
- Final normal golden verification passed 770 tests, covering all 322 SAT-enabled fixture directories and core/rule tests. Timing cleanup restored 402 timing-only files and preserved existing measurements in 218 semantic-statistics files; seven expanded fixtures received deliberate expected-time budget increases.

## Weighted-stage validation

- Weighted-stage verification: complete `NEXTEST_TEST_THREADS=4 make test-accept` passed 1,505 tests with 14 skipped, plus all workspace doctests. Normal golden verification passed 781 tests across every enabled SAT fixture and the core/rule packages. Production Clippy and formatting passed.
- Weighted-stage coverage was 327 of 622 runnable fixtures, with 2,663 SAT portfolios and 2,568 uniform portfolios across 239 fixtures. The three former mismatches are enabled; new fixtures cover all 48 signed-arithmetic operand pairs and five solutions of sparse weighted constraints. The signed fixture uses Conjure/Minion references because existing Z3 Euclidean division differs for negative divisors.
- Weighted-stage timing cleanup restored 300 timing-only files and retained prior measurements in 325 semantic-statistics files. The newly expanded signed-division fixture deliberately raises its expected-time budget from one to ten seconds. Generated artefacts are committed separately from implementation.

## Remaining PB algorithms

- Added `rustsat-dynamic-poly-watchdog` and `pindakaas-swc`; all-mode enumerates five PB choices. DPW lower bounds use inversion and exact bounds combine both directions, at full precision. Pindakaas specialisation retains SWC when selected. Overlapping Pindakaas adder and totaliser implementations are deferred.
- Full `NEXTEST_TEST_THREADS=4 make test-accept` passed 1,505 tests with 14 skipped, plus workspace doctests. Production Clippy and formatting passed. Exhaustive PB projection now also covers weights 1, 64 and 257 and their bound transitions.
- All 327 SAT-enabled fixtures remain successful, with 3,355 SAT portfolios (3,260 uniform across 239 fixtures), an increase of 692 portfolios. Timing cleanup restored 578 timing-only files and retained previous timing fields in the 44 files with semantic statistics changes.
- Normal golden verification passed 781 tests, covering every enabled SAT fixture and the core/rule packages.

## BinaryOffset and BinaryRank

- Added `int_offset` and `int_rank` production representations, with finite-domain initialisation, canonical sparse intervals, uniform family selection, structural code bounds and solution encoding/decoding. Singletons use one constrained zero bit. Full signed 32-bit endpoints are handled with widened unsigned-code arithmetic.
- Offset linear views use the domain minimum plus positive powers of two, so all PB algorithms consume the actual numeric expression directly. Rank maps interval ranks to actual values before arithmetic; sparse codes are never substituted for values. Fallback arithmetic uses temporary actual-value binary circuit operands rather than materialising another semantic representation.
- AMO/cardinality/PB algorithms remain Boolean encoders. Binary representations do not need Direct's one-hot domain AMO, but explicit AMO and cardinality constraints remain available with every integer representation.
- The new unsigned-integers fixture checks all 300 combinations of five integer representations, six AMO choices, two cardinality choices and five PB choices. Each portfolio agrees with six reference solutions. Unit tests cover sparse-domain round trips, singleton and unused codes, and signed 32-bit endpoint mapping.
- Replaced eager free-Boolean completion enumeration with a lazy iterator, so solution limits avoid allocating all 2^50 completions. Dominance constraints now cause re-solving before further completions; regression tests cover both behaviours.
- Full `NEXTEST_TEST_THREADS=4 make test-accept` passed 1,511 tests with 14 skipped, plus workspace doctests. Production Clippy and formatting passed. Coverage is 328 of 623 runnable fixtures, with 4,333 SAT portfolios and 4,236 uniform portfolios across 240 fixtures. Timing cleanup restored 299 timing-only files and preserved prior measurements in 327 semantic-statistics files.
- Normal golden verification passed 787 tests, covering every SAT-enabled fixture and the core/rule packages. Generated traces retain their native formatting. Implementation and generated artefacts are committed separately.
