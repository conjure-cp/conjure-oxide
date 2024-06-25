# Conjure-Oxide

This repository contains the in-progress Conjure Oxide constraints modelling
tool and its dependencies. 

This repository hosts the following projects:

* [Conjure Oxide](https://github.com/conjure-cp/conjure-oxide/tree/main/conjure_oxide)
* [`minion_rs` - Rust bindings to Minion](https://github.com/conjure-cp/conjure-oxide/tree/main/solvers/minion)
* [`chuffed_rs` - Rust bindings to Chuffed](https://github.com/conjure-cp/conjure-oxide/tree/main/solvers/chuffed)
* [`uniplate` - An implementation of the Haskell Uniplate in Rust](https://github.com/conjure-cp/conjure-oxide/tree/main/crates/uniplate)

This project is being produced by staff and students of University of St
Andrews, and is licenced under the [MPL 2.0](./LICENCE).

## Rust Nightly Support

The following compiler flags are required for Conjure-Oxide to work with
Nightly Rust:

```sh
export RUSTFLAGS="-Zlinker-features=-lld" 
export RUSTDOCFLAGS="-Zlinker-features=-lld" 
cargo build <...>
```

This is because of current incompatibilities with linkme and the new default
linker ([link](https://github.com/dtolnay/linkme/issues/94)).


## Documentation

API documentation can be found [here](https://conjure-cp.github.io/conjure-oxide/docs/).

## Contributing

See the [Contributors Guide](https://github.com/conjure-cp/conjure-oxide/wiki/Contributing).

<!-- vim: cc=80
-->
