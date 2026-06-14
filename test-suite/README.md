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

1. Create a new directory under `testing/tests/integration/<folder>`.
2. Add your `input.essence` file and (optionally) a `config.toml` to configure the solver.
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

### Licence

This project is licenced under the [Mozilla Public Licence
2.0](https://www.mozilla.org/en-US/MPL/2.0/).
