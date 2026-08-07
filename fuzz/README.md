## `fuzz`

A fuzz tester from the [cargo fuzz library](https://rust-fuzz.github.io/book/introduction.html) that invokes [libFuzzer](https://llvm.org/docs/LibFuzzer.html), for testing robustness of target functions in `conjure-oxide`.

### Usage
Create directory for test corpus file ``` mkdir fuzz/corpus/{test_name}```

Copy Essence files from codebase to corpus before running fuzz tester ```find . -name "*.essence" -type f -exec cp {} fuzz/corpus/{test_name}/ \;```

Run `cargo fuzz run {testname}`, for example: `cargo fuzz run detect_errors -- -max_len=4096 -max_total_time=3600`

Minimise found error test cases with `cargo fuzz tmin {testname} fuzz/artifacts/{testname}/crash-{crashID}`

To make the fuzzer stop manually, use the following limits:
- Total iterations: `-runs=N`
- Time (seconds): `-max_total_time=N`
- Maximum input length: `-max_len=N`

### Licence

This project is licenced under the [Mozilla Public Licence
2.0](https://www.mozilla.org/en-US/MPL/2.0/).
