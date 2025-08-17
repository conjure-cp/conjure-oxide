# Conjure Oxide

This repository contains the in-progress Conjure Oxide constraints modelling
tool and its dependencies. 

This repository hosts the following projects:

* [Conjure Oxide](https://github.com/conjure-cp/conjure-oxide/tree/main/conjure_oxide)
* [`minion-sys` - Rust bindings to Minion](https://github.com/conjure-cp/conjure-oxide/tree/main/crates/minion-sys)

This project is being produced by staff and students of University of St
Andrews, and is licenced under the [MPL 2.0](./LICENCE).

## Installation

### Building from Source

The following dependencies are required:

* Conjure (including solvers)
* Clang 
* Libclang
* Rust, installed using rustup

**Ensure that Conjure is placed early in your PATH to avoid conflicts with
ImageMagick's conjure command!**

Run `cargo install --path conjure_oxide` to install `conjure_oxide`.



## Documentation

API documentation can be found [here](https://conjure-cp.github.io/conjure-oxide/docs/).

## Contributing

See the [project wiki](https://github.com/conjure-cp/conjure-oxide/wiki)
<!-- vim: cc=80
-->
