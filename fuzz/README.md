## `fuzz`

A fuzz tester from the [cargo fuzz library](https://rust-fuzz.github.io/book/introduction.html) that invokes [libFuzzer](https://llvm.org/docs/LibFuzzer.html), for testing robustness of target functions in `conjure-oxide`.

### Usage
Copy Essence files from seeds to corpus before running fuzz tester ```cp fuzz/seeds/* fuzz/corpus/{testname}/```

Run `cargo fuzz run {testname}`, for example: `cargo fuzz run error_detector`

Minimise found error test cases with `cargo fuzz tmin {testname} fuzz/artifacts/{testname}/crash-{crashID}`

Specify test limit using `-runs={number of runs}` or `-max_total_time={time limit}` flags

### Licence

This project is licenced under the [Mozilla Public Licence
2.0](https://www.mozilla.org/en-US/MPL/2.0/).