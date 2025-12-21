[//]: # (Author: Leia McAlister-Young)
[//]: # (Last Updated: 08/05/2025)

# Overview
Custom tests were created as a solution to the problem that integration tests do not allow for testing of error messages, which automatically return from the integration test method and therefore cannot be analyzed. The custom tests, however, work for both erroneous and non-erroneous code. Each test contains an input, a `run.sh` file to execute the test, and an expected standard output/error (or both) to compare against. Conjure Oxide is set up to automatically create and execute a test for each test folder in the `tests/custom `directory when `cargo test` is ran so to add a test case, all one must do is add a folder in the directory which contains the necessary components.

# How to add a test case
Adding a test case is simple. First add a folder with the test case name to the `tests/custom` directory. Tests can be organized within folders in the directory--only folders with a `run.sh` file will be treated as a test case. Within the test folder, add these components:

1. `run.sh` file: This will typically be in the format `conjure_oxide <options> <command>` with the command being `solve model.eprime` (assuming the input file is named `model.eprime`). The `conjure_oxide` executable path is added to the PATH environment variable, so the script can invoke `conjure_oxide` as a command (i.e., just by name, without the full path).
   
      ex) `conjure_oxide --solver Minion --enable-native-parser solve model.eprime`

2. input file: This file will be the Essence code that is inputted into Conjure Oxide. It can be named anything but be sure to reference it correctly in the `run.sh` file.
   
      ex) `find x : bool`

3. `stdout.expected`/`stderr.expected`: These files will be what the output is compared against. They will also be what is overwritten if the test is run with `ACCEPT=true`. Avoid creating empty files: if there is no expected error from the input, do not create a `stderr.expected` file and if there is no output, do not create a `stdout.expected` file.

The custom tests are run automatically when `cargo test` is run. You can run just the custom tests with `cargo test custom` and a specific test with `cargo test custom_<test_path>`. To overwrite the expected output/error with the actual output/error, run the tests with `ACCEPT=true`.

# Code structure
The code to run the custom tests is integrated into the `build.rs` file. Much like for the integration tests, the custom tests directory is traversed and a test is dynamically written at compile time for any folder containing a `run.sh` file. Tests are based on a custom test template (shown below) which calls the `custom_test` function (passing in the test folder path). They are written into a generated file, which is included at the bottom of the `custom_tests.rs` file (which contains the `custom_test` function). 

```
#[test]
fn {test_name}() -> Result<(), Box<dyn Error>> {{
    custom_test("{test_dir}")
}}
```

The `custom_test` function takes the test directory as a parameter. It adds the `conjure_oxide` executable to the PATH environment variable and runs the commands from the `run.sh` file, saving the produced output. It then overwrites the expected output and error if accept was set to true and compares the actual and expected outputs/errors if not. If either does not match, the expected and actual output are printed and the test fails. Otherwise, the test passes.