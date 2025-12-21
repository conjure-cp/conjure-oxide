<!-- maturity: draft
author: Hanaa Khan
created: 20-12-25
---- -->

# Testing

## Introduction

Integration testing and Essence Testing are different ways of describing and testing the ssame problem, but at different levels. The Rust file can be used by developers to test the interface directly, whereas the Essence file can be used by high-level users to specify complex problems abstractly.

## Integration Tests

Integration testing is done to test how the different components, like the rust interface and the underlying solver (eg C++ Minion solver), work together. 

The tests manually recreate models using rust to ensure constraints are passed to the solver correctly, and that the results are as expected. 

### Writing Integration Tests

[Rust integration tests](https://doc.rust-lang.org/rust-by-example/testing/integration_testing.html) follow a similar format. 

They contain imports, a static `SOLS_COUNTER` to store the number of solutions found, and a callback function that increments at every solution and returns true to make the solver continue searching for solutions. 

The test function should be annotated with `#[test]` and include `#[allow(clippy::panic_in_result_fn)]` to suppress a warning for ease of testing. 

The new model is created with `Model::new()`, and its symbol table is populated with variables.

The solver must then be run using `minion_sys::run_minion` which takes the model and callback function. 

Lastly, the results are verified to ensure the number of solutions found match the expected solutions.

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
