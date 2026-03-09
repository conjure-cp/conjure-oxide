[//]: # (Author: Hayden Brown)
[//]: # (Last Updated: 25/02/2026)

# Installation
Currently, Conjure Oxide is installed by building from source.

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
3. Run the install command to install `conjure-oxide` (this may take some time):
    ```
    cargo install --path crates/conjure-cp-cli
    ```
    - Ensure `bin` within the [install directory](https://doc.rust-lang.org/cargo/commands/cargo-install.html) is in your PATH.
4. Verify `conjure-oxide` is installed and working by running a command:
    ```
    conjure-oxide --help
    ```
