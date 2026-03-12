[//]: # (Author: Hayden Brown)
[//]: # (Last Updated: 25/02/2026)

# Installation
Currently, Conjure Oxide is installed either by [building from source](#building-from-source) or by downloading a [nightly release](#downloading-a-nightly-release).

## Building From Source
### Dependencies
The following dependencies are required to build and use Conjure Oxide:
- [Conjure](https://conjure.readthedocs.io/en/latest/installation.html) including solvers.
    - Conjure is currently required for some components of Conjure Oxide. This is something that will change in the future as Conjure Oxide becomes more independent.
    - Ensure that Conjure is placed early in your PATH to avoid conflicts with ImageMagick's `conjure` command!
- [Clang](https://clang.llvm.org/) and libclang.
- [CMake](https://cmake.org/download/).
- [Rust](https://rust-lang.org/tools/install/) installed using rustup.

### Building
1. Clone the repository:
    ```
    git clone https://github.com/conjure-cp/conjure-oxide.git
    cd conjure-oxide
    ```
2. Run the install command to install `conjure-oxide` (this may take some time):
    ```
    cargo install --release --path crates/conjure-cp-cli
    ```
3. Verify `conjure-oxide` is installed and working by running a command:
    ```
    conjure-oxide --help
    ```

## Downloading a Nightly Release
1. Download the appropriate archive from the [latest nightly release](https://github.com/conjure-cp/conjure-oxide/releases/tag/nightly).
    - Make sure you choose the correct archive for your system. Conjure Oxide currently supports ARM-based macOS (`aarch64-darwin`) and x86-based Linux (`x86_64-linux-gnu`).
    - Make sure you choose the correct archive containing the dependencies you need:
        - If you do not have Conjure or solvers on your system, download the archive for your system ending with `with-solvers`.
        - If you have the solvers on your system but not Conjure, download the archive for your system ending with `with-conjure`.
        - If you have both Conjure and the solvers on your system, download the archive for your system ending with `standalone`.
2. Extract the archive using your preferred method.
3. Open a terminal in the extracted directory and run a test command:
    ```
    ./conjure-oxide --help
    ```
- If you are on macOS, you may run into a problem with binaries being blocked from running. If this is the case, run the following command in the extracted directory:
    ```
    xattr -dr com.apple.quarantine .
    ```
- If you would like these commands to be available everywhere on your system, copy the binaries into a directory which is in your PATH.

## Troubleshooting
### `Unknown command: conjure-oxide`
Check the path at which `cargo install` places binaries (see [cargo-install(1)](https://doc.rust-lang.org/cargo/commands/cargo-install.html)) and ensure it's in your PATH environment variable. You may need to restart your shell for it to pick up these changes.

### ImageMagick Conflicts
If the `conjure` command that is part of ImageMagick conflicts with your Conjure installation, ensure that the directory containing Conjure binaries is earlier in your PATH than the directory containing ImageMagick binaries.

### macOS Quarantine
If you are running into problems on macOS, make sure you remove the quarantine attribute from all pre-built binaries using the following command in their containing directory:
```
xattr -dr com.apple.quarantine .
```
