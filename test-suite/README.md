## `test-suite`

Integration tests for `conjure-oxide`

### Usage

Run the integration tests from the repository root with:

```sh
cargo test
```

### How tests work

Each test runs `conjure-oxide` on an `input.essence` file and checks that the rewritten AST and solver solutions match the expected output files stored in the test directory.

### Creating a new test

1. Create a new directory under `test-suite/tests/integration/<folder>`.
2. Add your `input.essence` file and (optionally) a `config.toml` to configure the solver.
   Integration run metadata is tracked separately in `stats.toml`.
3. Run the following to generate the expected solution and JSON files:

```sh
ACCEPT=true cargo test
```

### Updating tests with `ACCEPT=true`

If you expect the rewritten AST to change (e.g. after a refactor), you can overwrite the stored output files by running:

```sh
ACCEPT=true cargo test
```

Instead of comparing against the existing JSON files, the test harness will:

1. Run old Conjure on the same input.
2. Run the new `conjure-oxide` implementation.
3. Compare the solutions. If they match, it'll overwrite the stored AST and solution files with the new output.

`ACCEPT=true` lets you update expected outputs, while still guarding correctness by checking against old Conjure.

When a test fails, the harness writes debugging artifacts under `diagnostics/` in that
test's directory (gitignored): `failure.json`, Conjure/Savile Row `conjure/*.eprime-minion`,
and oxide generated traces / Minion snapshots when available. Diagnostics are captured as
each stage runs; on timeout the partial snapshot is kept and `stats.toml` is set to
`timeout(N)`.

To update `stats.toml` `expected-time` entries, use `make test-accept-with-slower-times`. This only
writes a new time when the rounded runtime is slower than the current value, so speedups
remain visible as diffs. Use `make test-accept-with-exact-times` to overwrite times with
the current observed runtime. The exact-times target also writes a Git-diff-based timing
comparison CSV to `target/accept-times-diff.csv`.

`stats.toml` also records the last accepted status, Conjure and oxide timing stats, and
aggregate rule trace application counts derived from the expected rule trace snapshots.

For timing-only runs where rule trace generation overhead is unwanted, set
`CONJURE_OXIDE_TEST_DISABLE_TRACING=1`. This skips integration-test rule trace file
generation and rule trace snapshot validation; solution checks and timing recording still run.

### Licence

This project is licenced under the [Mozilla Public Licence
2.0](https://www.mozilla.org/en-US/MPL/2.0/).
