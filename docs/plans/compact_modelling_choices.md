# Compact modelling choices

Audit and initial policy improvements on `sat-ir`, 6 October 2026. Compact is a deterministic
baseline, not a measured best-encoding policy.

## Current policy

| Choice | Compact policy | Main limitation |
| --- | --- | --- |
| Representation | Lowest compactness score; SAT packed sets/relations count the full stored mask domain; usually name breaks ties | Scores still omit most constraint costs, propagation and decoding |
| Uniform representations | First selected representation fixes the type family for the rewrite | Chooses from the first encountered domain, rather than a whole-model cost |
| Rewrite rules | Lowest resulting AST depth among equally applicable candidates | AST depth does not estimate CNF size or propagation |
| AMO | Per constraint: pairwise up to five entries; otherwise ladder | allDifferent uses operand count as an upper estimate for its value groups |
| Cardinality | RustSAT totalizer | Never selects the available Pindakaas sorting network |
| PB / integer relations / objective | RustSAT GTE if all inspected absolute coefficient sums are at most 4096; otherwise binary adder | Ignores bound tightness, coefficient structure, repeated bounds and structured inputs |
| Element | Implication if all inputs have at most four entries; otherwise support | Ignores index/value domains and sharing |
| Table | Per constraint: tuple up to four distinct constant rows or actual variable rows; otherwise MDD | Ignores tuple width and detailed prefix/suffix sharing |
| allDifferent | Value-AMO when every unresolved instance has usable value indicators and no compound comparisons; otherwise pairwise | One ineligible instance prevents value-AMO for all unresolved instances |
| Minion integer domain threshold | Span at most 10 uses DISCRETE | Uses maximum - minimum + 1, including holes; historical default without a measured model-specific cost |

SAT family choices remain model-wide except for compact AMO and table choices. Explicit pins
still cover the whole family. AMO input sizes are estimates: an allDifferent contributes
its number of operands, not the number of indicators surviving for each value. PB estimates
precede the provider's coefficient/complement normalisation. These cutoffs are initial policies.
The AMO cutoff has a concrete clause-count basis: RustSAT pairwise uses `n(n-1)/2`
clauses and ladder uses `3n-4` for `n >= 2`. Pairwise is smaller through five inputs;
ladder is smaller from six. This compares those two providers, not every available AMO algorithm.

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

The packed-set/relation score previously counted only cardinality-valid values, while occurrence
scores included invalid storage assignments. SAT now counts the full mask domain too. This
removes the artificial advantage from cardinality restrictions, and permits occurrence or
explicit storage to win. Minion/Z3 scores and explicit representation annotations retain their
existing behaviour. This corrects a storage estimate rather than providing a complete cost model.

A controlled CNF measurement used a binary relation over `0..7` allowing the 48 pairs whose sum
is not divisible by four. All three encoders passed all 128 assignments to the six value bits
and reified output, existentially quantifying their auxiliaries. With the same GTE component
provider, tuple used 164 variables/435 clauses, MDD 128/303, and binary support 142/383. Density
alone therefore does not justify choosing binary support: MDD benefits from shared suffixes.
Binary support stays available through pins and portfolio heuristics pending propagation/search
measurements. These counts include the common numeric-equality machinery.

The corrected storage score resolves `function_partial_smoke`: compact chooses explicit slots
and BinaryOffset integers, producing all 201 assignments and matching Conjure. SAT translation
took 0.063 seconds and enumeration 0.044 seconds in the bounded four-worker recording run. The
trace ends at auxiliary 478, rather than approximately 484,000 in the earlier failed packed
attempt. The whole fixture also runs Minion portfolios and took about 34 seconds; its sampled
2.2 GiB process-tree peak must not be attributed to the much shorter SAT run.

Policy validation: all 556 core/CLI/rule/representation tests passed, together with `make check`.
All 123 previously passing compact fixtures and the partial-function reproducer passed a fresh
four-worker recording run with the existing 600-second/4 GiB limits. Ten focused normal golden
checks and the four full table integration portfolios also passed. Existing timing baselines
were restored; incidental changes to truncated solution subsets were discarded. The
[measurement CSV](compact_modelling_measurements.csv) preserves fresh timings. Its fixture times
and sampled memory peaks include reference and other backend work; a zero peak means the short
run was not sampled, not zero memory use. The five other known resource failures were not retried.

## Recorded before/after timings

The [comparison CSV](compact_modelling_before_after.csv) uses the raw saved measurements from
the earlier full acceptance and the compact refresh, matching heuristic, channelling and seeds.
Times are SAT translation plus solving, excluding reference and other backend work. Across the
123 previously passing compact fixtures, summed translation fell from 612.860 to 473.700 seconds,
summed solving from 623.547 to 565.448 seconds, and their total from 1236.407 to 1039.148 seconds
(16.0% less recorded time). Five of these fixtures are rewrite-only.

| Fixture | Before SAT seconds | After SAT seconds |
| --- | ---: | ---: |
| function_partial_smoke | Failed, over 4 GiB | 0.107570 |
| function_total_int_set_01 | 0.060497 | 0.004408 |
| binrel04 | 17.181014 | 0.007649 |
| set08 | 9.264405 | 0.002234 |
| element-compound-sets | 0.849040 | 0.127200 |
| sparse-partial-small | 1.996082 | 0.009425 |
| blackhole | 445.665928 | 380.208208 |
| plotting | 178.685064 | 144.955670 |
| test-branchingon2 | 149.312085 | 110.213260 |
| opd | 11.640579 | 44.832179 |
| bibd-implied | 15.683916 | 18.280900 |

The storage changes explain a substantial reduction in the first six fixtures' rewritten models.
The overall timing difference is not an isolated measurement of this policy change: the earlier
run predates the shared constant-matrix fix and scheduled a different mix of tests. Model traces
are unchanged for the other examples above, including `opd`'s slower first-solution search.
Controlled repeated runs are needed before attributing those differences to compact. No elapsed
before time is available for the memory-limited partial-function attempt; its new full fixture
time was 34.16 seconds including the Minion portfolios and Conjure reference.

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

CLI/threshold validation: `make check` passed; 553 selected core, CLI, rule, representation and constant-matrix
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
family choices, extending the AMO/table approach to other families while preserving explicit
family pins. Test bound tightness and structure before
changing the PB or table cutoffs. Optimise known construction hotspots independently, so changes
in frontend cost are not attributed to a SAT solver or encoding algorithm.
