## `conjure-cp`

`conjure-cp` provides Essence parsing, modelling and solving as a Rust library.
The API is under development.

### Usage

Add the library and its rule provider to your `Cargo.toml`. This configuration uses
Minion without the optional SAT or Z3 backends:

```toml
[dependencies]
conjure-cp = { git = "https://github.com/conjure-cp/conjure-oxide", default-features = false }
conjure-cp-rules = { git = "https://github.com/conjure-cp/conjure-oxide", default-features = false }
```

For a local checkout, use paths to the two crates instead:

```toml
[dependencies]
conjure-cp = { path = "../conjure-oxide/crates/conjure-cp", default-features = false }
conjure-cp-rules = { path = "../conjure-oxide/crates/conjure-cp-rules", default-features = false }
```

Link the rule provider with `use conjure_cp_rules as _;`. Its registered rules are
resolved when constructing a `Pipeline`; missing or unknown rule sets return an
error. Removing `default-features = false` enables the default optional backends.

```rust
use conjure_cp::pipeline::{Config, Pipeline};
use conjure_cp::solutions::SolutionOptions;
use conjure_cp_rules as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pipeline = Pipeline::new(Config::default())?;
    let model = pipeline.compile(
        "given n : int\nfind x : int(1..n)\nsuch that x > 1",
        Some("letting n be 3"),
    )?;
    let solutions = pipeline.solve(model, &SolutionOptions {
        number_of_solutions: 0, // Enumerate all solutions; the default is one.
        ..Default::default()
    })?;
    assert_eq!(solutions.len(), 2);
    Ok(())
}
```

`compile` parses source strings, instantiates optional Essence parameters, and
rewrites the model. For individual stages, use `pipeline.parse(source)`,
`pipeline.parse_file(path)` and `pipeline.rewrite(model)`. `solve` reconstructs
high-level Essence values, including structured values and optimisation results.
For generated ASTs, start with `Model::new(pipeline.context())` so rules and
statistics share the pipeline's context.
The default parser works in memory and needs no external Conjure executable.

Configure only the options you need with Rust defaults:

```rust
use conjure_cp::pipeline::{Config, Pipeline};
use conjure_cp::settings::Heuristic;
use conjure_cp_rules as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pipeline = Pipeline::new(Config {
        heuristic: Heuristic::Random,
        seed: 42,
        ..Default::default()
    })?;
    Ok(())
}
```

Each pipeline operation installs its settings on the current thread and restores
the caller's settings afterwards, including during panic unwinding. Random seeds,
interactive responses and encoding overrides are reset for each outer operation.
Group compilation and solving inside `pipeline.with_settings(|| { ... })` to
continue the same modelling choices and interactive answers through both stages. No
`GlobalArgs` or manual settings initialisation is required. A pipeline can be
reused across fuzzer inputs; create a new one for each input when statistics must
be separate. `pipeline.context()` exposes accumulated statistics and source
metadata.

For lower-level solver access, keep model loading and solving inside a synchronous
`pipeline.with_settings(|| { ... })` closure. This also configures solver-time
rewrites; the settings belong to the current thread and do not follow asynchronous
work onto another thread. `pipeline.solve` and `pipeline.solve_with_timings` manage
this scope automatically.

### Documentation

- User documentation: <https://conjure-cp.github.io/conjure-oxide-reports/>
- API Docs: <https://conjure-cp.github.io/conjure-oxide/docs/conjure_cp/>

### Licence

This project is licenced under the [Mozilla Public Licence
2.0](https://www.mozilla.org/en-US/MPL/2.0/).
