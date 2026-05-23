[//]: # (Author: Hanaa Khan, Georgii Skorokhod)
[//]: # (Last Updated: 20-05-25)

# Integration Tests

Integration tests are located under `tests-integration/tests/integration`.

Each test contains: 
- An Essence model.
- A JSON file containing all the expected solutions to the model.
- JSON dumps of the original model AST and model AST after rewriting.
- The expected and actual rule traces.

The tests run Conjure-oxide with that Essence file as input. Conjure-oxide first rewrites the model into Essence', then runs it through a solver (Minion by default; some solvers use SAT or SMT instead) and collects the solution.

Each test checks that:
- The produced solutions match the expected ones.
- The rewritten model matches the expected AST.
- The rule trace matches (optional). 

Test configuration (e.g. which solver to use) is set in a `config.toml` file in each test directory. If it is not present, the default Minion solver will be used.

The source code of the testing harness is (mostly) in `tests-integration/tests/integration_tests.rs`.

## Running Integration Tests

Run the full test suite with: 
```bash
cargo test
``` 

## Updating Expected Outputs with `ACCEPT=true`

If you expect the rewritten AST to change (e.g. after a refactor), or are creating a new test, run:

```bash
ACCEPT=true cargo test
```

Instead of comparing against the existing output files, the harness will:
1. Run old Conjure on the same input.
2. Run the new `conjure-oxide` implementation.
3. Compare the solutions. If they match, overwrite the stored AST and solution files with the new output.

This allows expected outputs to be updated while still guarding correctness by validating against old Conjure.

## Running Specific Tests

They are run through the 'conjure-oxide solve' command followed by a filename, like the following:
```bash
conjure-oxide solve input_file.essence
```

There are example Essence models and their expected solutions under `tests-integration/tests/integration`.

### Essence Files

Essence is the modelling language. The tests verify that a high level problem specification correctly produces the expected result after being translated to Essence' and solved.
