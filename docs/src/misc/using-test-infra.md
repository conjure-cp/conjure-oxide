# Testing in Conjure Oxide

## Rust Testing Harness

The `cargo` CLI tool contains several rust utilities[^1], which are used to interact with the Rust Testing Harness. While the harness on its own is a fully usable testing script that provides support for unit tests and integration tests. It also has the benefit of being able to implement customised functionality using a build script. This means that functionality can be added to the harnesss[^2], without having to provide a custom entry point[^3].

Integration tests in Rust are run using `cargo test`. When `cargo test` is run, each of the files in the integration testing interface get compiled as a separate package. This then leads to a number of tests that can be run either all together or one-at-a-time. For large projects like Conjure Oxide, this allows for continuous integration.

## The Conjure Oxide Testing Setup

Conjure Oxide uses a middle-ground testing setup, which is necessary because the project contains multiple packages, which are used as internal APIs in conjure oxide. One of the packages that is maintained internally is the 'tests-integration' package, used for testing the conjure-oxide tool. The package uses the conjure-oxide tool as a dependecy, and uses it to run testing input files. This allows the use of the rust testing harness to run custom code without having to rewrite the harness and deal with all the complications that come with defining custom entry points. 

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

This means, however, that the build script cannot use any of the code in the other integration testing files, as it cannot rely on the crate it is used to build. To circumvent this, the integration tester uses a few different tricks 

TODO: ASK FELIX AND GEORGII FOR HELP

## The Production Testing Setup

So far, production testing of the compiled conjure oxide tool has been done using the GNU Parallel command line tool, and in certain cases using python scripts. GNU Parallel is a useful command line tool which, true to its name, can be used to run commands in parallel. In addition to this, the command can also be used to actually put together the commands which are built. For a full explanation of the tool's many useful features, check out the (incredibly comprehensive) documentation for the tool on GNU's webpage. 

The Conjure Oxide production testing setup uses GNU parallel with a layer of abstraction built in python which records the time comparisons in CSV. 

---

[^1]: Check out the Rust Book's documentation.
[^2]: Primarily, this added functionality uses integration testing, which uses the external interface of the crate. Check out the page in Rust Book here. 
[^3]: This refers to Rust's provided advanced testing, which is not used by conjure oxide as of now
[^4]: which is a file name 'build.rs' in the root directory of the crate.
