[//]: # (Author: Nicholas Davidson)
[//]: # (Last Updated: 10/02/2026)

# Testing
## Types of Tests
Conjure-Oxide currently has four forms of tests:
- Integration tests
- [Custom tests](./Custom-Tests.md)
- [Roundtrip tests](./roundtrip/roundtrip_testing.md)
- A small number of unit tests
## Test generation
The test generation occurs in the file `./tests_integration/build.rs`. This generates the integration, custom and roundtrip tests all in a similar manner.

For each test framework we define a base directory for all these tests:
- `./tests_integration/tests/integration`
- `./tests_integration/tests/custom`
- `./tests_integration/tests/roundtrip`

Any sub-directory of these defined directories containing the necessary files to make a test, will be considered a test.
- For integration and roundtrip tests this is containing a singular file with a `.essence` or `.eprime` extension.
- For custom tests this is containing a `run.sh` script file.

Once these tests are identified a Rust `#[test]` is created using the defined templates (`<test type>_test_template`). This test is within the pre-generated file `gen_tests_<test type>.rs` within the specified output directory (`OUT_DIR`).

This template contains the code to call a new function which carries out the test. The values passed to the template determines its test name, through sanitation of the path, and determine the arguments to this test function.

Finally, the test function itself includes the line `include!(concat!(env!("OUT_DIR"), "/gen_tests_<test type>.rs"));`, such that it is accessible during the test through insertion using the include macro.

This format improves scalability and allows new tests to be created by just creating the input file.