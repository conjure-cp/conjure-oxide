# Testing in Conjure Oxide

## Rust Testing Harness

The `cargo` CLI tool contains several rust utilities[^1], which are used to interact with the Rust Testing Harness. While the harness on its own is a fully usable testing script that provides support for unit tests and integration tests. It also has the benefit of being able to implement customised functionality using a build script. This means that functionality can be added to the harness[^2], without having to provide a custom entry point[^3].

Integration tests in Rust are run using `cargo test`. When `cargo test` is run, each of the files in the integration testing interface get compiled as a separate package. This then leads to a number of tests that can be run either all together or one-at-a-time. For large projects like Conjure Oxide, this allows for continuous integration.

## The Conjure Oxide Testing Setup

Conjure Oxide uses a middle-ground testing setup, which is necessary because the project contains multiple packages, which are used as internal APIs in conjure oxide. One of the packages that is maintained internally is the 'tests-integration' package, used for testing the conjure-oxide tool. The package uses the conjure-oxide tool as a dependency, and uses it to run testing input files. This allows the use of the rust testing harness to run custom code without having to rewrite the harness and deal with all the complications that come with defining custom entry points. 

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

This means, however, that the build script cannot use any of the code in the other integration testing files, as it cannot rely on the crate it is used to build. To circumvent this, the integration tester uses a few different tricks. It is actually built as an entirely separate crate which the `cargo test` utility uses to run tests. It uses a collection of macros to build the crate in an order which is different from the default, so that the testing utility tool runs all of the testing code. The testing code calls different members of the internal API in order to run tests in a step-by-step manner.

With the current setup, when the build script is run, it then executes every other testing utility. This includes the following scripts: 

### Integration Test

The Integration Test setup takes one essence file and checks that every step of solving works correctly by comparing the output of each step with files that contain previously prepared 'outputs', which have been verified for correctness. It checks the parsed output, the application of rules in the rule engine and the solutions, thus verifying each of the steps of the solution process.

New Integration Tests should be added whenever additions are made to the AST, more solver support, new solving features, new representations etc. 

### Custom Tests

Custom tests work by running a shell script (generally, this uses a release-compiled binary of conjure oxide), and then comparing the standard out and standard err streams to a statically stored expected output, which has also been generated and checked for correctness. The custom tests are generally used to check things other than the solving process. For example, it is used to check pretty printing, error presentation (for unsupported models), logging behaviour, intended failure testing and so on. 

New Custom Tests should be added whenever new features are added to the tool which do not involve changes being made to the solution generation. This does not include changes to the parser, rule engine, rulesets or solver. However, it does include changes like new flags, new logging features, changes to the file interaction, additions to the API etc. 

### Roundtrip Tests

Roundtrip Tests are used to ensure that valid files passed to conjure oxide will actually complete a full 'roundtrip'; that is, to check that they go through each step in the solving 'pipeline' and that each step behaves as expected. 

Each of the above testing frameworks are documented much more rigorously in their own sections. They are also mentioned and referenced extensively elsewhere in this book. 

## The Black-Box Testing Setup

So far, testing of the compiled conjure oxide tool (also sometimes called Black-Box Testing[^5]) has been done using the GNU Parallel command line tool, and in certain cases using python or bash scripts. For a full explanation of the tool's many useful features, take a look at the (incredibly comprehensive) documentation for the tool on GNU's webpage. 

For the purposes of this chapter, it is important to know that GNU Parallel is a useful command line tool which, true to its name, is used to run commands in parallel. In addition to this, the tool can put together the commands which are to be run, using what is essentially a cartesian product of lists which are passed to it as arguments. It can also manage per-process runtime, memory allocation, process usage and so on, using command line flags. 

The Conjure Oxide testing setup (in the conjure-cp/conjure-oxide-tester repository) uses GNU parallel with a layer of abstraction built in python which records the time comparisons into a database. The Repository also includes tools that allow some rudimentary examination and visualisation of the runtime data that has been collected and placed in the output database. 

## Using the Tester and Associated Tools

### Prerequisites

- Python 3.14+ (project pyproject.toml requires >=3.14).
- GNU Parallel (command line: `parallel`) for `runner/run_tests.sh`
- SQLite3 (for the results Database)
- runsolver: used to manage an environment for the solver tool. This tool is the standard in SAT solver competitions, and other such applications
- Optional, but highly recommended: `uv` for a project manager and `ty` for a type checker (used by some workflows)
- In case `uv` is not used, look at pyproject.toml for an up-to-date list of the python dependencies. Install each of these using pip _inside_ the virtual environment (for various reasons, your linux distribution may not allow system-wide installation).

Create and activate a virtual environment, then install these dependencies using the following commands:

```bash
python3 -m venv .venv
source .venv/bin/activate
python -m pip install --upgrade pip
python -m pip install uv pandas textual ty
```

### Configuration

- `settings.json` contains runner command mappings and the output DB path. Example keys:
	- `runner_commands`: internal JSON object whose keys are the names of the 'runners' and values are the command with which those 'runner names' are associated. You can use this to effectively define a short alias for a long command. The command can be pretty much whatever you need it to be, including flags and so on. 
	- `outfile`: relative path to the SQLite Database, where all of the data that is collected by `timer.py`.
    - `runsolver_cfg`: internal JSON object which takes in the resource configuration to allocate internally to the `runsolver` tool.
        - `memory`: The used memory allocation allowed per solver. 
        - `walltime`: The used maximum runtime per solver, the process will terminate after this amount of time.
        - `cpus`: The CPUs allocated per solver 

Edit `settings.json` to add or modify runners and change the configuration with which to run `runsolver`.

### Running Tools

- **setup.py**: Re-initialise the database at the path from the configuration. To (re)create the Database used by the runner, run `python3 src/setup.py`. This script will prompt for confirmation and will wipe the existing database if confirmed. If no database exists, it will create one.
- **show.py**: This will print out the database in the form of a database. By itself, this is not very useful but it is good to know the status of your data collection run.
- **timer.py**: This tool will take a runner name and an essence file as CLI arguments. It will then use the runner passed in to generate a solution for the file passed in a record a bunch of information, including the runtime and the number of clauses and so on. 
- **view.py**: This is a TUI tool to look through the result stored in the database at the path in the `settings.json` file. It uses the `textual` package.


### Running the Convenience Script

The main runner script is `runner/run_tests.sh`.

Usage examples:

Run with a single runner defined in settings.json
```bash
./runner/run_tests.sh oxide_main_minion
```
Run with a runner and filter models that contain 'max'
```bash
./runner/run_tests.sh oxide_main_sat max
```

Disable clauses collection
```bash
./runner/run_tests.sh --no-closures oxide_main_sat
```

`run_tests.sh` finds all `.essence` files under `models/`, filters them by an optional operand string, and runs them in parallel using the runner commands from `settings.json`. Results and failures are written to the configured SQLite DB. If a run exits non-zero, its runtime is recorded as `-1.0` and the error is logged to the `failures` table.

---

[^1]: Check out the Rust Book's documentation.
[^2]: Primarily, this added functionality uses integration testing, which uses the external interface of the crate. Check out the page in Rust Book here. 
[^3]: This refers to Rust's provided advanced testing, which is not used by conjure oxide as of now
[^4]: which is a file name 'build.rs' in the root directory of the crate.
[^5]: 'Black-Box Testing' is a model of testing in Software Engineering, which refers to tests which are made in a way that presumes no knowledge of the system being tested. 