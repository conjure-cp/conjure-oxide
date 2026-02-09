[//]: # (Author: Owain Thorp)
[//]: # (Last Updated: 07/05/2025)
# Why benchmark?
Benchmarking is an essential part of any coding project, especially when it is performance-oriented. While it can be a little daunting when first getting started, this guide aims to show that benchmarking can be integrated into conjure-oxide and its workflows with a little work. 

# ``criterion``
By far, the most popular benchmarking tool currently available for ``Rust`` is the ``criterion`` [crate](https://bheisler.github.io/criterion.rs/book/). Based off the ``Haskell`` library of the same name, it is a statistics-based tool which aims to measure _wall-clock time_ for individual functions. To get started on ``criterion`` benching in a rust project ``my_project``, you first need to make a directory called ``benches``, which ``Rust`` will recognise as holding all benchmarking files. Let's now make a benchmark called ``my_bench.rs`` inside of ``my_project/benches``

We now add the following changes to the crate's ``cargo.toml`` file 
```rust 
[dev-dependencies]
criterion = "0.3"

[[bench]]
name = "my_bench"
harness = false
```

Suppose that we now want to create function to benchmark the addition of two numbers (which, as expected should be very fast!). We add the following to ``my_bench``. 
```rust
use criterion::{Criterion, black_box, criterion_group, criterion_main};

pub fn add(x: u64, y: u64) -> u64 {
    x + y
}

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("add 20 + 20", |b| {
        b.iter(|| add(black_box(20), black_box(20)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
```
The ``.bench_function`` method creates an instance of a benchmark. Then the ``.iter`` method tells Criterion to repeatedly execute the provided closure. Finally, black_box is used to prevent the compiler from optimising away the code being benchmarked. To run the benchmark simply run ``cargo bench``. Among other things, your terminal should show something alone the lines of:
<img width="1322" alt="image" src="https://github.com/user-attachments/assets/53565407-11fc-43b0-afe1-2e3b582d41c5" />
Which shows the average wall-clock time, as well as providing some information on outliers and performance against previous benchmarks. For the full details, see ``\target\criterion``. The ``.html`` reports are especially good. 

``criterion`` is usually the right tool for most benchmarks, althought there are issues. Due to the statistics-driven ethos of ``criterion``, there is currently no one-shot support, with 10 samples being the minimum number of samples for a benchmark. Wall-clock time also gives little insight into where things are slowing in your code, and will not catch things like poor memory locality. Even more crucially, however, is how ``criterion`` performs in CI pipelines. The developer’s themselves say the following: 

_”You probably shouldn’t (or, if you do, don’t rely on the results). The virtualization used by_
_Cloud-CI providers like Travis-CI and Github Actions introduces a great deal of noise into the_
_benchmarking process, and Criterion.rs’ statistical analysis can only do so much to mitigate_
_that. This can result in the appearance of large changes in the measured performance even_
_if the actual performance of the code is not changing."_

As such, we need some other metric apart from wall-clock time to use in order to still run benchmarks in a CI pipeline. This is where the ``iai-callgrind`` crate comes in. 

# ``iai-callgrind``
Iai-Callgrind is a benchmarking framework which uses [Valgrind's Callgrind](https://valgrind.org/docs/manual/cl-manual.html) and other to provide extremely accurate and consistent measurements of Rust code. It does **not** provide information on wall-clock time, instead focussing on metrics like instruction count and memory hit rates. It is important to note that this will only run on ``linux``, due to the valgrind dependancy. Let us create a benchmark called ``iai-bench`` in the ``benches`` folder. We add the following to ``cargo.toml``
```rust
[profile.bench]
debug = true

[dev-dependencies]
iai-callgrind = "0.14.0"
criterion = "0.3"

[[bench]]
name = "iai-bench"
harness = false
```
To get the benchmarking runner, we can quickly compile from source with ``cargo install --version 0.14.0 iai-callgrind-runner``. To benchmark ``add`` using ``iai-callgrind`` we add the following to ``benches/iai-bench.rs``. 

```rust
use iai_callgrind::{main, library_benchmark_group, library_benchmark};
use std::hint::black_box;

fn add(x: u64, y:u64) -> u64 {
   x+y
}

#[library_benchmark]
#[bench::name(20,20)]
fn bench_add(x: u64,y:u64) -> u64 {
    black_box(add(x,y))
}

library_benchmark_group!(
    name = bench_fibonacci_group;
    benchmarks = bench_add
);

main!(library_benchmark_groups = bench_fibonacci_group);
```
And again run using ``cargo bench``. To specify only running this benchmark we can instead do ``cargo bench --bench iai-bench``. Upon running, you should see something like the following. 

![image](https://github.com/user-attachments/assets/8db29f54-0558-435e-a8ae-844bbfd3f312)
As you can see, ``iai`` is lightweight, fast and can provide some really accurate statistics on instruction count and memory hits. This makes ``iai`` perfect for benching in CI workflows!
# Workflows
Once benchmarking is established, workflows are not too difficult to add too. As discussed before, for CI workflows ``iai`` should be used, and not ``criterion``. Take the following example from the ``tree-morph`` crate. I will put the code below and then briefly explain each portion. It should not be too difficult to adapt to other benchmarks. 

```yaml
name: "iai tree-morph Benchmarks"

on:
  push:
    branches:
      - main 
      - auto-bench 
    paths:
      - conjure_oxide/**
      - solvers/**
      - crates/**
      - Cargo.*
      - conjure_oxide/tests/**
      - .github/workflows/iai-tree-morph-benches.yml
  pull_request:
    paths:
      - conjure_oxide/**
      - solvers/**
      - crates/**
      - Cargo.*
      - conjure_oxide/tests/**
      - .github/workflows/iai-tree-morph-benches.yml
  workflow_dispatch:



jobs:
  benches:
    name: "Run iai tree-morph benchmarks"
    runs-on: ubuntu-latest
    timeout-minutes: 10

    strategy:
      # run all combinations of the matrix even if one combination fails.
      fail-fast: false
      matrix:
        rust_release:
          - stable
          - nightly
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: ${{ matrix.rust_release }}

      - name: "Cache Rust dependencies"
        uses: actions/cache@v4
        with:
            path: |
              ~/.cargo/registry
              ~/.cargo/git
              target
            key: ${{ runner.os }}-cargo-${{ matrix.rust_release }}-${{ hashFiles('**/Cargo.lock') }}
            restore-keys: |
              ${{ runner.os }}-cargo-${{ matrix.rust_release }}-
      - name: Install Valgrind
        run: sudo apt-get update && sudo apt-get install -y valgrind

      - name: Install iai-callgrind-runner
        run: cargo install --version 0.14.0 iai-callgrind-runner

      - name: Run tree-morph benchmarks with iai-callgrind
        run: cargo bench --manifest-path crates/tree_morph/Cargo.toml --bench iai-factorial --bench iai-identity --bench iai-modify_leafs > iai_callgrind_output.txt

      - name: Upload artefact
        uses: actions/upload-artifact@v4
        with:
          name: iai-callgrind-results-${{ matrix.rust_release }}
          path: iai_callgrind_output.txt
```
Some comments:
- ``name`` just tells GitHub what to call the workflows
- ``on`` tells GitHub __when__ to run the workflow
- ``jobs`` is the core of the workflow:
  - ``strategy`` specifies that we want to run both nightly and stable rust
  - In ``steps``, we first check out the repository code and set up a specific stable Rust toolchain based on a matrix variable, and then cache Rust dependencies. Next we install the necessary things for valgrind to run, before running benchmarks. We tell the virtual machine to an ``.txt`` file and upload it as an artefact. 