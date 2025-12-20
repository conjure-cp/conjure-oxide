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

[Rust integration tests](https://doc.rust-lang.org/rust-by-example/testing/integration_testing.html) are written ...

<!-- ### Writing Integration Tests -->


### Running Integration Tests

They are run with the following commands: 
```bash
cargo test
``` 
or 
```bash
cargo test ACCRPT=true
```

## Essence Tests

Essence tests focus on the modelling language. It verifies that a high level problem specification correctly produces the expected result after being translated to Essence' and solved.

<!-- ### Writing Essence Tests -->

### Running Essence Tests

They are run through the 'conjure solve' command followed by a filename, like the following:
```bash
conjure solve input_file.essence
```

