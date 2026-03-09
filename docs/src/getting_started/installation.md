[//]: # (Author: Hayden Brown)
[//]: # (Last Updated: 25/02/2026)

# Installation
Currently, Conjure Oxide is installed by building from source.

## Dependencies
The following dependencies are required to build and use Conjure Oxide:
- [Conjure](https://conjure.readthedocs.io/en/latest/installation.html) including solvers.
    - Conjure is currently required for some components of Conjure Oxide. This is something that will change in the future as Conjure Oxide becomes more independent.
    - Ensure that Conjure is placed early in your PATH to avoid conflicts with ImageMagick's `conjure` command!
- [Clang](https://clang.llvm.org/) and libclang.
- [CMake](https://cmake.org/download/).
- [Rust](https://rust-lang.org/tools/install/) installed using rustup.

## Building From Source
1. Clone the repository:
    ```
    git clone https://github.com/conjure-cp/conjure-oxide.git
    cd conjure-oxide
    ```
2. Run the install command to install `conjure-oxide` (this may take some time):
    ```
    cargo install --path crates/conjure-cp-cli
    ```
3. Verify `conjure-oxide` is installed and working by running a command:
    ```
    conjure-oxide --help
    ```

## Troubleshooting
### `Unknown command: conjure-oxide`
Check the path at which `cargo install` places binaries (see [cargo-install(1)](https://doc.rust-lang.org/cargo/commands/cargo-install.html)) and ensure it's in your PATH environment variable. You may need to restart your shell for it to pick up these changes.
