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

This means, however, that the build script cannot use any of the code in the other integration testing files, as it cannot rely on the crate it is used to build. To circumvent this, the integration tester uses a few different tricks. It is actually built as an entirely separate crate which the `cargo test` utility uses to run tests. It uses a colletion of macros to build the crate in an order which is different from the default, so that the testing utility tool runs all of the testing code. The testing code calls different members of the internal API in order to run tests in a step-by-step manner.

With the current setup, when the build script is run, it then executes every other testing utility. This includes the following scripts: 

### Integration Test

The Integration Test setup takes one essence file and checks that every step of solving works correctly by comparing the output of each step with files that contain previously prepared 'outputs', which have been verified for correctness. It checks the parsed output, the application of rules in the rule engine and the solutions, thus verifying each of the steps of the solution process.

New Integration Tests should be added whenever additions are made to the AST, more solver support, new solving features, new representations etc. 

### Custom Tests

Custom tests work by running a shell script (generally, this uses a release-compiled binary of conjure oxide), and then comparing the standard out and standard err streams to a statically stored expected output, which has also been generated and checked for correctness. The custom tests are generally used to check things other than the solving process. For example, it is used to check pretty printing, error presentation (for unsupported models), logging behaviour, intended failure testing and so on. 

New Custom Tests should be added whenever new features are added to the tool which do not involve changes being made to the solution generation. This does not include changes to the parser, rule engine, rulesets or solver. However, it does include changes like new flags, new logging features, changes to the file interaction, additions to the API etc. 

### Roundtrip Tests

TODO ASK NICK AND CALLUM

TODO: ASK FELIX AND GEORGII FOR HELP

## The Production Testing Setup

So far, production testing of the compiled conjure oxide tool has been done using the GNU Parallel command line tool, and in certain cases using python scripts. GNU Parallel is a useful command line tool which, true to its name, can be used to run commands in parallel. In addition to this, the command can also be used to actually put together the commands which are built. For a full explanation of the tool's many useful features, check out the (incredibly comprehensive) documentation for the tool on GNU's webpage. 

The Conjure Oxide production testing setup uses GNU parallel with a layer of abstraction built in python which records the time comparisons in CSV. 

## Using Each Tester

Adding 

---

[^1]: Check out the Rust Book's documentation.
[^2]: Primarily, this added functionality uses integration testing, which uses the external interface of the crate. Check out the page in Rust Book here. 
[^3]: This refers to Rust's provided advanced testing, which is not used by conjure oxide as of now
[^4]: which is a file name 'build.rs' in the root directory of the crate.
