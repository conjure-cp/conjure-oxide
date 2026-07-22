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
`heuristic = ["f", "c", "r", "x"]`. The strategies mean first, compact (minimum resulting AST
depth), seeded random, and all respectively. The default is `x`. Set `seed = 123` for random runs. The `x` strategy
replays every representation and equally-applicable-rule choice from a fresh parsed model and keeps
separate `model-000`, `model-001`, … golden artifacts for every parser/rewriter/expander/solver
configuration. Channelling is configured with `channelling = "no"`; `yes` is reserved but currently
unsupported.

### Creating a new test

1. Create a new directory under `test-suite/tests/integration/<folder>`.
2. Add your `input.essence` file and (optionally) a `config.toml` to configure the solver.
   Integration run metadata is tracked separately in `stats.toml`.
3. Run the following to generate the expected solution and JSON files:

```sh
ACCEPT=true cargo nextest run -p test-suite
```

### Updating tests with `ACCEPT=true`

If you expect the rewritten AST to change (e.g. after a refactor), you can overwrite the stored output files by running:

```sh
ACCEPT=true cargo nextest run -p test-suite
```

Instead of comparing against the existing JSON files, the test harness will:

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
the slowest observed budget and avoids recording a fast fluke. To discard `config.toml` / `stats.toml` files whose only local edits are noisy `*-time`
field updates, run `./tools/discard-config-time-changes.sh`. Files with any other edits
are left alone.

After an accept run, you can write a Git-diff-based timing comparison CSV with
`python3 ./tools/accept-times-diff-report.py` (default output:
`target/accept-times-diff.csv`).

`stats.toml` also records the last accepted status, Conjure and oxide timing stats, and
aggregate rule trace application counts derived from the expected rule trace snapshots.

For timing-only runs where rule trace generation overhead is unwanted, set
`CONJURE_OXIDE_TEST_DISABLE_TRACING=1`. This skips integration-test rule trace file
generation and rule trace snapshot validation; solution checks and timing recording still run.
