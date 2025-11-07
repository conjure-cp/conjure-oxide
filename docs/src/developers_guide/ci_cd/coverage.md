<!-- TODO: Edit this -->

## User Guide

1. Run coverage with `./tools/coverage.sh`.
2. Open this in a web-browser by opening `target/debug/coverage/index.html`.
3. An `lcov` file is generated at `target/debug/lcov.info` for in-editor coverage. 

    * If you use VSCode, configuration for this should be provided when you clone the repo. Click the "watch" button in the status bar to view coverage for a file.

## Implementation

See the code for full details: [tools/coverage.sh](https://github.com/conjure-cp/conjure-oxide/blob/main/tools/coverage.sh).

A high level overview is:

1. The project is built and tested using instrumentation based coverage.
2. Grcov is used to aggregate these reports into `lcov.info` and `html` formats.
3. The `lcov.info` file can be used with the `lcov` command to generate summaries and get coverage information. This is used to make the summaries in our PR coverage comments.

**Reading:**

1. [grcov readme - how to generate coverage reports](https://github.com/mozilla/grcov).
2. [rustc book - details on instrumentation based coverage](https://doc.rust-lang.org/rustc/instrument-coverage.html).
### Doc Coverage
**Text:** This prints a doc coverage table for all crates in the repo:
```sh
RUSTDOCFLAGS='-Z unstable-options --show-coverage' cargo +nightly doc --workspace --no-deps 
```


**JSON:**
Although we don't use it yet, we can get doc coverage information as JSON. This will be useful for prettier and more useful output:

```sh
RUSTDOCFLAGS='-Z unstable-options --show-coverage --output-format json' cargo +nightly doc --workspace --no-deps 
```

**Reading**

See the unstable options section of the rustdoc book. [Link](https://doc.rust-lang.org/rustdoc/unstable-features.html).

---

*This section had been taken from the 'Coverage' page of the conjure-oxide wiki*