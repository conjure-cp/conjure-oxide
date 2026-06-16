# Integration tests

Each subdirectory with exactly one `.essence` file and a `config.toml` is an integration test case. The harness discovers them under `test-suite/tests/integration/`.

The `config.toml` file is used for selecting parser(s), rewriter(s), comprehension expander(s), and solver(s). There are additional flags, like `skip-conjure-validation` (omit or set to an empty string to validate against Conjure during accept; a non-empty string skips validation and records why), see `test-suite/src/test_config.rs` for a full list.

## Top-level layout

| Directory | Purpose |
|-----------|---------|
| `basic/` | Small, focused tests grouped by language feature. One folder per feature with numbered or descriptive cases inside. |
| `bugs/` | Regression tests for specific bugs (`experiment/` holds exploratory cases) |
| `cnf/` | CNF / SAT-oriented models |
| `dominance/` | Dominance-relation models (oxide-specific) |
| `eprime-minion/` | Essence' models targeting Minion via the Conjure pipeline |
| `hakank-eprime/` | Models from Hakank's Essence' collection |
| `mildly-interesting/` | Larger puzzles and examples |
| `minion-constraints/` | Individual Minion constraint tests |
| `optimisations/` | Rewriter / optimisation behaviour |
| `savilerow/` | Savile Row benchmark models (see `savilerow/README`) |
| `sets/` | Set operators and constant evaluation |
| `smt/` | SMT backend tests (`bool/`, `int/`, `matrix/`) |
