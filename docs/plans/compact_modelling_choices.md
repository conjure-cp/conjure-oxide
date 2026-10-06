# Compact modelling choices

Audit of `sat-ir`, 6 October 2026. Compact is a deterministic baseline, not a measured
best-encoding policy. No performance-policy changes were made in this audit.

## Current policy

| Choice | Compact policy | Main limitation |
| --- | --- | --- |
| Representation | Lowest representation compactness score; usually name breaks ties | Scores omit constraint costs, propagation and decoding |
| Uniform representations | First selected representation fixes the type family for the rewrite | Chooses from the first encountered domain, rather than a whole-model cost |
| Rewrite rules | Lowest resulting AST depth among equally applicable candidates | AST depth does not estimate CNF size or propagation |
| AMO | Pairwise when the largest relevant input has at most five entries; otherwise ladder | One large AMO determines the algorithm for every AMO |
| Cardinality | RustSAT totalizer | Never selects the available Pindakaas sorting network |
| PB / integer relations / objective | RustSAT GTE if all inspected absolute coefficient sums are at most 4096; otherwise binary adder | Ignores bound tightness, coefficient structure, repeated bounds and structured inputs |
| Element | Implication if all inputs have at most four entries; otherwise support | Ignores index/value domains and sharing |
| Table | Tuple if all tables have at most four rows; otherwise MDD | Ignores tuple width, prefix sharing and binary-support opportunities |
| allDifferent | Value-AMO when every unresolved instance has usable value indicators and no compound comparisons; otherwise pairwise | One ineligible instance prevents value-AMO for all unresolved instances |
| Minion integer domain threshold | Span at most 10 uses DISCRETE | Uses maximum - minimum + 1, including holes; historical default without a measured model-specific cost |

SAT family choices are model-wide. AMO input sizes are estimates: an allDifferent contributes
its number of operands, not the number of indicators surviving for each value. PB estimates
precede the provider's coefficient/complement normalisation. These cutoffs are initial policies.

Integer scores expose the bias particularly clearly: Direct and Order score approximately
`2^number_of_Boolean_variables`; Log, Offset and Rank also score from their bit widths. This
favours short binary representations without comparing the circuits needed by the constraints.
Direct can still be valuable for equality, tables and value-AMO; Order for bounds. Rank can
save bits for sparse domains while making arithmetic value recovery more expensive.

## Evidence and what it establishes

The recent acceptance run showed that compact avoids the explosion from enumerating modelling
portfolios: five total-function fixtures failed or had their all-mode runs stopped, then passed
compact retries in one to two seconds. This establishes the value of selecting one portfolio;
it does not establish that its encodings are better than other single portfolios.

Conversely, `function_partial_smoke` has only 201 semantic assignments, but compact selected a
packed integer representation for its function/relation/set chain. Bit membership then became
general division/modulo circuits, reaching approximately 484,000 auxiliaries and the 4 GiB
limit before a solution. This is a representation-to-constraint cost mismatch. The lee-distance
sample also showed direct integer representation construction and symbol-table cloning during
rewriting. Such construction costs must be separated from solver search in future comparisons.

See `sat_compact_acceptance.md` and its CSV for the measured fixture outcomes. The shared
constant-matrix domain fix landed afterwards; the six resource failures have not been remeasured
after that fix. Do not treat the older timings as controlled comparisons of encoding algorithms.

## User pins and CLI grouping

All six `--sat-encoding-*` flags already accept optional pins. Without a flag, first, random,
compact, interactive and all-mode select from applicable algorithms. Pins retain explicit
configuration provenance and consume no heuristic decision; irrelevant families consume none.
The resolver runs after rewriting, before SAT loading, and records the selected algorithms in
the SAT decision IR.

`--minion-discrete-threshold` now follows the same optional-pin convention. A Minion model with
optional integer domains selects once from 10, 0 (prefer BOUND), or `usize::MAX` (prefer DISCRETE).
First and compact retain 10; random, interactive and all-mode can select the other policies.
Required DISCRETE domains override every policy. Selection occurs at Minion loading, so it does
not add SAT choices. Comparing thresholds uses an unsigned wide type, including unlimited pins.

The CLI groups the SAT flags, Minion threshold, channelling, comprehension expander and heuristic
controls under **Modelling choices**. Solver selection, solver seeds and Minion search orders
remain configuration controls. The integration harness retains its existing configured/default
threshold of 10 as an explicit pin: this change does not multiply the recorded Minion portfolios.
An optional threshold in fixture configuration is a remaining harness connection.

Validation: `make check` passed; 553 selected core, CLI, rule, representation and constant-matrix
integration tests passed. The added tests cover omitted and explicit CLI pins, the help group,
all three Minion policies shared across differently sized domains, unlimited threshold pins,
and constraints forcing DISCRETE despite a zero threshold. No fixture recordings changed.

## Other choices to expose

- Type-family representation pins, including Direct, Order, BinaryValue/Log, BinaryOffset and
  BinaryRank. Essence representation annotations already pin domains; there is no equivalent
  general CLI type-family pin. Keep integer encoding separate from compound layout. SMT's
  integer theory (LIA/BV) is another representation choice covered by the same mechanism.
- RustSAT commander/bimander group sizes and sub-encoders, and two-product sub-encoders.
- RustSAT DPW precision, with refinement to exact bounds before accepting solutions.
- Public Pindakaas BDD/SWC consistency and cutoff settings.
- Optional Minion threshold pins in the integration configuration, with bounded portfolio
  recording when enabled.
- Mixed representations and non-uniform SAT channelling, already deferred.

The comprehension expander has its own `auto` policy; omission is not currently a general
heuristic choice. Channelling is a portfolio policy, rather than an encoder selected from a
constraint. Search ordering and solver seeds merit tuning separately from modelling choices.

## Next analysis

After the remaining feature gaps, compare individual portfolios on a fixed benchmark set and
seed, recording representation/provider/parameters, rewrite and CNF construction time, variables,
clauses, peak memory, search time and solution checks separately. Include the current compact
portfolio as the baseline. Start with type-family costs across all domains and per-constraint
family choices, preserving explicit family pins. Test bound tightness and structure before
changing the PB or table cutoffs. Optimise known construction hotspots independently, so changes
in frontend cost are not attributed to a SAT solver or encoding algorithm.
