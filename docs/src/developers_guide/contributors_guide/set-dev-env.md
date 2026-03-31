<!-- maturity: draft
authors: Georgii Skorokhod, Niklas Dewally
created: 31-03-25
---- -->
# Setting up your development environment

Conjure Oxide supports Linux and Mac.

Windows users should install [WSL](https://learn.microsoft.com/en-us/windows/wsl/setup/environment#set-up-your-linux-username-and-password) and follow the Linux (Ubuntu) instructions below:


<details><summary><b>Linux (Debian/Ubuntu)</b></summary>

**The following software is required:**
- The latest version of stable Rust, installed using [rustup](https://www.rust-lang.org/tools/install).
- A C/C++ compilation toolchain and libraries:
  - Debian, Ubuntu and derivatives: `sudo apt install build-essential libclang-dev`
  - Fedora: `sudo dnf group install c-development` and `sudo dnf install clang-devel`
* [Conjure](https://github.com/conjure-cp/conjure).
  - **Ensure that Conjure is placed early in your PATH to avoid conflicts with ImageMagick's `conjure` command!**

**Z3**
One of the solver backends, Z3, requires a separate install. Using some command-line package installer, install Z3. (e.g. `apt`). 
</details>

<details><summary><b>MacOS</b></summary>

**The following software is required:**
* the latest version of stable Rust, installed using [rustup](https://www.rust-lang.org/tools/install).
* an XCode Command Line Tools installation (installable using `xcode-select --install`)
* CMake: `brew install cmake` (for SAT solving)
* [Conjure](https://github.com/conjure-cp/conjure).

**Z3**
One of the solver backends, Z3, requires a separate install. Using some command-line package installer, install Z3. (e.g. for `Homebrew` run `brew install z3`). Depending on your device, you may need to do additional configuration:
* For **Intel** Macs, you may need to update your `~/.cargo/config.toml`:
```
[env]
Z3_LIBRARY_PATH_OVERRIDE = "/usr/local/lib"
Z3_SYS_Z3_HEADER = "/usr/local/include/z3.h"
```
Note that the Z3 shared object file (`libz3.dylib`) and header (`z3.h`) may be elsewhere. E.g. on a M3 Mac using Homebrew, the paths (as of writing) were:
```
Z3_LIBRARY_PATH_OVERRIDE = "/opt/homebrew/opt/z3/lib"
Z3_SYS_Z3_HEADER = "/opt/homebrew/opt/z3/include/z3.h"
```
</details>

<details><summary><b>St Andrews CS Linux Systems</b></summary>

1. Download and install the *pre-built binaries* for [Conjure](https://github.com/conjure-cp/conjure). Place these in `/cs/home/<username>/usr/bin` or elsewhere in your `$PATH`.

2. Install `rustup` and the latest version of Rust through `rustup`. 
   *The school provided Rust version does not work*.
   - By default, `rustup` installs to your local home directory; therefore, you may need to re-install `rustup` and Rust after restarting a machine or when using a new lab PC. 

3. Install `z3`, one of the solver backends. You may have to build it from [source](https://github.com/Z3Prover/z3), and then add the binary to your path. 

> To add a binary to your PATH in a way that persists every time you log out, run
> mkdir -p /cs/home/$USER/.paths.d
> echo ~/Documents/... > /cs/home/$USER/.paths.d/z3
> where the "..." if the path to your compiled z3 binary

</details>

---

### Improving Compilation Speed

Installing [sccache](https://github.com/mozilla/sccache) improves compilation speeds of this project by caching crates and C/C++ dependencies system-wide. 

* Install [sccache](https://github.com/mozilla/sccache) and follow the setup instructions for Rust. Minion detects and uses sccache out of the box, so no C++ specific installation steps are required.

---

*This section had been taken from the 'Setting up your development environment' page of the conjure-oxide wiki*