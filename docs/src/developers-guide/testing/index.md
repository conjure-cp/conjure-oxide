[//]: # (Author: Nicholas Davidson)
[//]: # (Last Updated: 10/02/2026)

# Testing
## Types of Tests
Conjure-Oxide currently has four forms of tests:
- Integration tests
- [Custom tests](writing-custom-tests.md)
- [Roundtrip tests](roundtrip-testing.md)
- A small number of unit tests
## Test generation
The test generation occurs in the file `./test-suite/build.rs`. This generates the integration, custom and roundtrip tests all in a similar manner.

For each test framework we define a base directory for all these tests:
- `./test-suite/tests/integration`
- `./test-suite/tests/custom`
- `./test-suite/tests/roundtrip`

Any sub-directory of these defined directories containing the necessary files to make a test, will be considered a test.
- For integration and roundtrip tests this is containing a singular file with a `.essence` or `.eprime` extension.
- For custom tests this is containing a `run.sh` script file.

Once these tests are identified a Rust `#[test]` is created using the defined templates (`<test type>_test_template`). This test is within the pre-generated file `gen_tests_<test type>.rs` within the specified output directory (`OUT_DIR`).

This template contains the code to call a new function which carries out the test. The values passed to the template determines its test name, through sanitation of the path, and determine the arguments to this test function.

Finally, the test function itself includes the line `include!(concat!(env!("OUT_DIR"), "/gen_tests_<test type>.rs"));`, such that it is accessible during the test through insertion using the include macro.

This format improves scalability and allows new tests to be created by just creating the input file.

## Scheduling longer tests first

`make test`, `make test-accept`, `make test-accept-with-max-times`, and the coverage
script use `tools/nextest.py` to run tests with longer `expected-time` values first.
This requires Python 3.11 or newer. Integration test timings come from
`stats.toml`; custom test timings come from `config.toml`. Tests without timings
(including roundtrip and unit tests) retain nextest's default priority.

For a direct nextest run with the same scheduling, use:

```bash
python3 tools/nextest.py --release --workspace
# Optional Cargo toolchain selector:
python3 tools/nextest.py +nightly --workspace
```

The wrapper reads current timings on every invocation and passes a temporary
configuration to nextest. Repository and command-line settings take precedence.
Nextest 0.9.91 or newer is required. Tests with equal expected times share a
priority; if there are more than 100 distinct times, adjacent duration buckets
share priorities. This changes scheduling only; test filtering, concurrency,
and timeouts still work as usual.
