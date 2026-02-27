[//]: # (Author: Hayden Brown)
[//]: # (Last Updated: 25/02/2026)

# Installation

To install Conjure Oxide, either [build it from source](#building-from-source).

## Prerequisites
Conjure Oxide currently requires [Conjure](https://github.com/conjure-cp/conjure) (including solvers).

## Building From Source
1. Ensure you have all the required dependencies to build Conjure Oxide:
    - [Conjure](https://conjure.readthedocs.io/en/latest/installation.html) (including solvers).
        - Ensure that Conjure is placed early in your PATH to avoid conflicts with ImageMagick's conjure command!
    - [Clang](https://clang.llvm.org/).
    - Libclang.
    - [CMake](https://cmake.org/download/).
    - [Rust](https://rust-lang.org/tools/install/) (installed using rustup).
2. Clone the repository:
    ```
    git clone https://github.com/conjure-cp/conjure-oxide.git
    cd conjure-oxide
    ```
3. Run the install command to install `conjure-oxide`:
    ```
    cargo install --path crates/conjure-cp-cli
    ```
