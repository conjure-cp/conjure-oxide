[//]: # (Author: Hanaa Khan, Georgii Skorokhod)
[//]: # (Last Updated: 21-12-25)

# Testing

## Introduction

<!-- TODO- fix -->
Integration testing and Essence Testing are different ways of describing and testing the same problem, but at different levels. The Rust file can be used by developers to test the interface directly, whereas the Essence file can be used by high-level users to specify complex problems abstractly.

## Integration Tests

Integration tests are located under `tests-integration/tests/integration`.

Each test contains: 
- An Essence model
- A JSON file containing all the expected solutions to the model
- JSON dumps of the original model AST and model AST after rewriting
- The expected and actual rule traces

The tests run conjure-oxide with that Essence file as input. Conjure-oxide first rewrites the model into Essence', then runs it through a solver (Minion by default; some solvers use SAT or SMT instead) and gets the solution.

The tests check that:
- The produced solutions match the ones we expected to get
- The rewritten model is the same as expected
- (Optionally) that the rule trace is as expected

Test configuration (e.g. which solver to use) is done in config.toml files in each test directory. If it is not present, the default Minion solver will be used.

The source code of the testing harness is (mostly) in `tests-integration/tests/integration_tests.rs`.

### Writing Integration Tests

Information about writing integration tests can be found in `../coding_resources/integration_tests_implementation.md`.

<!-- TODO: remove and document in a separate page. 

[Rust integration tests](https://doc.rust-lang.org/rust-by-example/testing/integration_testing.html) follow a similar format, where some tests can use all, some, or none of the following common syntax. 

They often contain imports, a static `SOLS_COUNTER` to store the number of solutions found, and a callback function that increments at every solution and returns true to make the solver continue searching for solutions. 

```rust
static SOLS_COUNTER: Mutex<i32> = Mutex::new(0);
```

The test function should be annotated with `#[test]`, and can include `#[allow(clippy::panic_in_result_fn)]` to suppress a warning for ease of testing when the panic is expected. 

When testing a model, the new model is created with `Model::new()`, and its symbol table is populated with variables.

```rust
let mut model = Model::new();

model
    .named_variables
    .add_var(String::from("a"), VarDomain::Bool);
model
    .named_variables
    .add_var(String::from("b"), VarDomain::Bool);
```

The solver must then be run using `minion_sys::run_minion` which takes the model and callback function. 

```rust
minion_sys::run_minion(model, callback)?;
```

Lastly, the results are verified to ensure the number of solutions found match the expected solutions.  -->

### Running Integration Tests

They are run with the standard command that executes the testing suite: 
```bash
cargo test
``` 

Or with the following command that is used for new tests or changing behaviour. It overwrites the old expected file with new actual output instead of just failing when output doesnt match: 
```bash
cargo test ACCEPT=true
```

## Essence Tests

Essence tests focus on the modelling language. It verifies that a high level problem specification correctly produces the expected result after being translated to Essence' and solved.

<!-- ### Writing Essence Tests -->

### Running Essence Tests

They are run through the 'conjure solve' command followed by a filename, like the following:
```bash
conjure solve input_file.essence
```
