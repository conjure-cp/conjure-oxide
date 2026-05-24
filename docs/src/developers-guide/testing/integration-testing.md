[//]: # (Author: Shikhar Srivastava)
[//]: # (Last Updated: 25/05/2026)

# Integration Testing

## Overview

The Integration Test setup takes one essence file and checks that every step of solving works correctly by comparing the output of each step with files that contain previously prepared 'outputs', which have been verified for correctness. It checks the parsed output, the application of rules in the rule engine and the solutions, thus verifying each of the steps of the solution process.

New Integration Tests should be added when:
- Additions are made to the AST 
- More solver support is added
- New solving features are added in the form of transformation rules
- New representations are added

And in any other cases where outcome-oriented changes are introduced. Ideally, all behaviour should be tested via integration tests, in order to ensure that breaking changes introduced later do not accidentally remove or break features[^1].

## Adding Integration Tests

Integration test live at the following path within the conjure-oxide repository: `conjure-oxide/tests-integration/integration`. Any essence file in this test directory, or any of it's subdirectories is treated as the input for a separate integration test. While it is not strictly necessary, by convention we never put two essence files in the same directory. This allows us to then (also by convention) use `input.essence` as the name of all input files. The name of an Integration test is a trunctation of its path, relative to the `integration` directory.

To add an integration test, create a new subdirectory and give it an appropriate name. Then write a valid essence file (ideally, granular enough to only test one new feature) in that same directory. Then, create a file named `config.toml` in the same directory as the input file. The name of the config file is _not_ convention alone, and your test may not work if it has some other name. In `config.toml`, select all of the options which should be tested for the essence file which you are writing this test for. Configuring is covered in greater detail in its own section below. 

To automatically generate all of the necessary testing metadata, run the following command:

```bash
ACCEPT=true cargo test <your-directory-name> 
```

The environment variable `ACCEPT` is particularly important: when `ACCEPT` is set, the integration system will first attempt to solve the model using each of the configurations. It will compare the generated solution against conjure, and if the solutions are the same, it will generate and save test metadata for each configuration. 

## Configuring Integration Tests

By convention, the `config.toml` file for a test looks something like this:

```toml
parser = [
    "tree-sitter",
    "via-conjure",
]

rewriter = [
    "naive",
    "morph",
]

comprehension-expander = [
    # "native",
    # "via-solver",
    "via-solver-ac",
]

solver = [
    "minion",
    "sat-log",
    "sat-direct",
    "sat-order",
    # "smt-bv-arrays-nodiscrete",
    # "smt-bv-arrays",
    # "smt-bv-atomic-nodiscrete",
    # "smt-bv-atomic",
    # "smt-lia-arrays-nodiscrete",
    # "smt-lia-arrays",
    # "smt-lia-atomic-nodiscrete",
    # "smt-lia-atomic",
]
expected-time = 10
```

The config file has the following fields, each of which contains a list of the 'active' or 'enabled' configuration options. All possible options supported by conjure-oxide are written in the file:

- `parser`: defines which parsers
- `rewriter`: defines which rewriters to use 
- `comprehension-expander`: defines how to expand comprehensions
- `solver`: defines which solvers to use
- `expected-time`: defines in seconds how long a solution is expected to take.  

The integration tester tries all possible combinations of the options enabled in the `config.toml` file.

Also by convention, we comment out configurations which are supported by conjure-oxide but do not work for the current solver. For instance, SAT solver does not support Matrices of Integers, so any tests that have matrices of integers will have all sat-solver options as comments. 

As support is added, we go through tests and change the comments to code incrementally. We then regenerate the test metadata for the new tests. To generate a large number of tests at the same time, use the `test-accept` MakeFile target.

---

[^1]: Early on in the development of the SAT compilation system, the SAT team briefly turned off all testing of SAT Solver features, which led to changes being merged which had not been tested on existing SAT functionality. This created a dangerous situation, where certain features were merged without being tested on a large part of the codebase. Now, we avoid this by ensuring that [code coverage](code-coverage.md) is not dropping unexpectedly in new Pull Requests using a CI/CD workflow. 