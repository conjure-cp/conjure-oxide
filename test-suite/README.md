## `test-suite`

Integration tests for `conjure-oxide`

### Usage

Run the integration tests from the repository root with:

```sh
cargo nextest run -p test-suite
```

### How tests work

Each test runs `conjure-oxide` on an `input.essence` file and checks that the rewritten AST and solver solutions match the expected output files stored in the test directory.

`config.toml` may select one or several modelling heuristics with `heuristic = "f"` or
`heuristic = ["f", "c", "r", "x"]`. The strategies mean first, compact, seeded random, and all
respectively. Compact minimises representation-domain size for representation choices and resulting
AST depth for equally-applicable rules. The default is `x`. Set `seed = 123` for random runs.
Backend solver randomness is configured independently with `solver-seed = 123` and defaults to
`0`. Both seed fields remain explicit in recorded stats. The `x` strategy
replays every representation and equally-applicable-rule choice from a fresh parsed model and keeps
separate `model-000`, `model-001`, … rule traces for every parser/rewriter/expander/solver
configuration. Channelling is configured with `channelling = "no"` or `"uniform"`. Uniform uses one
representation for every value of the same type throughout the model. `yes` is reserved
but currently unsupported.

Backend-specific modelling choices override the global settings. For example, this keeps
the other solvers on their global choices while exploring every uniform SAT portfolio:

```toml
solver = ["minion", "sat"]
heuristic = "f"
channelling = "no"

[solver-options.sat]
heuristic = "x"
channelling = "uniform"
trace-level = "aggregate"
```

SAT coverage uses `x` with uniform channelling when the fixture's existing backend profiles
enumerate all choices. Where those profiles do not use `x`, SAT uses `c`: compact chooses one
representation and the normal compact option for each SAT encoding family.

`trace-level = "full"` is the default: write and compare human-readable rule trace snapshots
(up to the 1,000,000-character snapshot limit).
`trace-level = "aggregate"` counts rule applications in memory and records totals by rule in
`stats.toml`, without formatting or writing full traces. `trace-level = "none"` disables both.
The field can appear at the top level or in `[solver-options.sat]` (and other solver overrides).
Aggregate counts are recorded in accept mode but are not compared with the saved counts during
normal test runs. Switching from `full` to `aggregate` therefore removes rule trace regression
checks; it retains recording of counts for inspection.
Solution comparisons remain enabled at every trace level. SAT fixtures with many expanded
configurations use aggregate tracing to limit snapshot size and recording overhead.

Integration tests use the `auto` comprehension expander by default, matching the CLI default.
Configure it explicitly with `comprehension-expander = "auto"`. Accepted oxide timings, statuses,
and rule counts are recorded in a `[[runs]]` entry in `stats.toml` for each compiled
model; the Conjure reference run is recorded separately. Each accepted run replaces the previous
stats snapshot rather than merging with it.

Tests with `number-of-solutions = "all"` compare every model/configuration's complete solution
set (ignoring duplicate assignments) with the first run, then check one shared
`<model>.expected.solutions` file (`input.expected.solutions` for `input.essence`). Solution order
and boolean spelling (`false`/`true` versus `0`/`1`) are canonicalised. Matrix annotations use
the index domain consistently across solvers; index ranges remain part of the snapshot.
Per-run generated solution files remain available for debugging. Acceptance writes the shared
file only after all configurations, rule traces, and enabled Conjure reference checks succeed.
If configurations disagree, acceptance fails and preserves the previous shared solution file.
Tests with a numeric solution limit retain per-run expected files and their existing count
checks, because solver ordering can produce different subsets. Tests with solving skipped do
not write a solution snapshot. The out-of-domain set equality regression checks the shared empty solution set across solvers.

### Creating a new test

1. Create a new directory under `test-suite/tests/integration/<folder>`.
2. Add your `input.essence` file and (optionally) a `config.toml` to configure the solver.
   Integration run metadata is tracked separately in `stats.toml`.
3. Run the following to generate the expected `.solutions` and rule-trace files:

```sh
ACCEPT=true cargo nextest run -p test-suite
```

### Updating tests with `ACCEPT=true`

If you expect the rewritten AST to change (e.g. after a refactor), you can overwrite the stored output files by running:

```sh
ACCEPT=true cargo nextest run -p test-suite
```

Instead of comparing against the existing `.solutions` files, the test harness will:

1. Run old Conjure on the same input.
2. Run the new `conjure-oxide` implementation.
3. Compare the solutions. If they match, it'll overwrite the stored AST and solution files with the new output.

`ACCEPT=true` (or `make test-accept`) updates expected outputs and overwrites
`expected-time` with the current observed runtime, while still guarding correctness by
checking against old Conjure.

When a test fails, the harness writes debugging artifacts under `diagnostics/` in that
test's directory (gitignored): `failure.json`, Conjure/Savile Row `conjure/*.eprime-minion`,
and oxide generated traces / Minion snapshots when available. Diagnostics are captured as
each stage runs; on timeout the partial snapshot is kept and `stats.toml` is set to
`timeout(N)`.

To only raise `expected-time` (max of the current budget and the observed time), use
`make test-accept-with-max-times` (`ACCEPT=with-max-times`). Running this a few times keeps
the slowest observed budget and avoids recording a fast fluke. To discard small timing-only
changes in `config.toml` / `stats.toml`, run `./tools/discard-config-time-changes.sh`.
A file is restored from HEAD only when every changed `*-time` value either has both
old and new times <= 1 second or has an absolute change <= 10% of the HEAD value.
Override these defaults with `--max-time SECONDS` and `--max-change-percent PERCENT`,
for example `./tools/discard-config-time-changes.sh --max-time 2 --max-change-percent 5`.
Use `--dry-run` to preview restorations. Files with larger timing changes, any other
edits, added/removed timing fields, or non-numeric timing changes are left alone.
When the HEAD value is zero, only the small-time threshold can permit a change.

After an accept run, you can write a Git-diff-based timing comparison CSV with
`python3 ./tools/accept-times-diff-report.py` (default output:
`target/accept-times-diff.csv`).

`stats.toml` also records the last accepted status, the shared Conjure timing stats, and per-model
oxide timing stats and rule trace application counts. Full tracing derives application
counts from the generated traces; aggregate tracing counts them in memory.

`stats.toml` is an observational record, not a correctness baseline: normal tests do not compare
current statuses, timings, rule-attempt counts, or per-rule application counts against its saved
values. Its `expected-time` field is used to select tests via `MAX_EXPECTED_TIME`. Correctness
checks use solution and rule trace snapshots separately; recording aggregate counts does not
assert that those counts remain unchanged.

For timing-only runs where rule trace generation overhead is unwanted, set
`CONJURE_OXIDE_TEST_DISABLE_TRACING=1`. This skips integration-test rule trace file
generation and rule trace snapshot validation; solution checks and timing recording still run.

Accepted integration runs store a single flat `[[runs]]` list, with one entry per
compiled model. Each row identifies its configuration and model index, with the
full realised choice path and selected labels for `heuristic = "x"`. Compact and other heuristics produce one row
per model. `representations` lists the representation rules used; the
`sat-encoding-*` arrays list the resolved algorithms present in SAT decisions.
A compact model may use several algorithms in one family: the timing measures
that combination, rather than assigning an invented time to each algorithm.

`translation-time` covers parsing, instantiation and rewriting; `solve-time`
covers backend loading, solving, reconstruction and writing the solution file.
Configuration totals are obtained by summing their model rows; no separate
aggregate rows are stored. Each row also records its own rule counts. The additional
`backend-time`, `search-time` and `solution-processing-time` fields separate
backend loading, the solver call (including callbacks and solver-time rewrites),
and reconstruction plus solution output. These are wall-clock seconds, not
internal solver CPU measurements. Validation, snapshots and reference-tool time
are excluded. `solution-limit = 0` means all solutions; absent solver timings
mean translation only or a failed run. A failed model has `status = "fail"`;
completed translation and solution-collection phases are retained when a later
validation or snapshot check fails.
Accepting a configuration replaces its model rows so smaller portfolios do not
leave stale entries behind.

The unified format replaces the former configuration-level `[[runs]]` and
`[[model-runs]]` sections. Regenerate older integration stats by deleting their
`stats.toml` files and running `make test-accept`.
