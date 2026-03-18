User guide:
1. Copy essence files from seeds to corpus using `cp fuzz/seeds/* fuzz/corpus/{testname}/`
2. Run `cargo fuzz run {testname}`, for example: `cargo fuzz run error_detector`
3. Minimise found error test cases with `cargo fuzz tmin {testname} fuzz/artifacts/{testname}/crash-{crashID}`