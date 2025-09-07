## `perf-benchmarks` 

This folder provides a standard set of models that can be used to benchmark
Conjure Oxide. The time measured is the wall time to run `conjure-oxide solve
--no-run-solver <model>`. 

This is not a representative suite covering all use-cases of Conjure Oxide; it
is primarily intended to quantify general performance improvements that affect
many models, such as optimisations to the AST or domain code. In particular,
benchmarks are only run with default settings, targeting Minion.


## Installation 

Install `hyperfine`:

```
cargo install hyperfine
```

## Usage

Use the script `./run_all` to run the benchmarks.

See the comments inside `./run_all` for details on environment variables.
