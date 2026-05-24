[//]: # (Author: Shikhar Srivastava)
[//]: # (Last Updated: 25/05/2026)

# Overview

This section contains a technical description of the testing infrastructure used by the Conjure Oxide Project to run all of the tests which have been described so far. It is useful for developers who intend to rework this system, add to it, or even replace it entirely. 

The Testing Infrastructure currently used by conjure oxide is built using the Rust Testing Harness[todo: add ref to docs]. The harness provides a lot of very useful functionality - for instance, it natively supports integration tests, compiles and formats tests nicely and prints outputs in a readable fashion.

However, the harness also introduces certain limitation that one must work within when adding new tests to the rust-based testing harness. As such, it is also important to understand how to use, work with (and in certain cases, chafe against) the harness before attempting to add new testing functionality or rework old testing functionality.  

## Rust Testing Harness

The `cargo` CLI tool contains several rust utilities[^1], which are used to interact with the Rust Testing Harness. The harness on its own is a fully usable testing system that provides support for unit tests and integration tests. It also has the benefit of being able to implement custom functionality using a build script. This means that features can be added to the harness[^2], without having to provide a custom entry point[^3].

Integration tests in Rust are run using `cargo test`. When `cargo test` is run, the tests in each package are run, unless a specific package is specified with the `-p | --package` flag. This then leads to a number of tests that can be run either all together or one-at-a-time. For large projects like Conjure Oxide, this allows for continuous integration.

## The Conjure Oxide Testing Setup

Conjure Oxide uses a middle-ground testing setup, which is necessary because the project contains multiple packages, which are used as internal APIs in conjure oxide. One of the packages that is maintained internally is the `tests-integration` package, used for testing the libraries that are used by the `conjure-oxide` tool. The package uses the `conjure-oxide` tool as a dependency, and uses it the complicated tests that we have seen so far in the book. 

This is a neat system which allows us to use the rust testing harness to run our own code without having to rewrite the harness and deal with all the complications that come with defining custom entry points. 

This works because of the build system in cargo, which works by looking for a 'build script'[^4] and running it just before compiling the crate. The 'tests-integration' crate's structure looks like this: 

```
         .
        ├──  build.rs
        ├──  Cargo.toml
        ├── 󰂺 README.md
        ├── 󰣞 src
        │   ├──  lib.rs
        │   └──  test_config.rs
        └──  tests
            ├──  custom
            ├── 󰡯 custom_test_template
            ├──  custom_tests.rs
            ├──  integration
            ├── 󰡯 integration_test_template
            ├──  integration_tests.rs
            ├──  parser_tests
            ├──  roundtrip
            ├── 󰡯 roundtrip_test_template
            └──  roundtrip_tests.rs
```

one limitation that should be kept in mind is that the build script is compiled and run before the compilation of any code in the crate. This means that when the build script is run, it cannot link in any of the libraries or functions within the `tests-integration` crate. It therefore cannot use any of the code in the other integration testing files, as it cannot rely on the crate which it is being used to compile. 

To circumvent this, the testing system uses a few different tricks. As mentioned previously, it is actually built as an entirely separate crate which the `cargo test` utility can use to run tests. The build script for this crate uses a collection of macros to fetch tests and generate new code - one file for each of the tests specified in the custom-, roundtrip- and integration-tests directories. When cargo moves on to building and running the code within the crate, it discovers these newly generated files, compiles them and runs them. It will treat each individual file as a single test, and provide information for these tests' runtimes and exit status (pass/fail). 