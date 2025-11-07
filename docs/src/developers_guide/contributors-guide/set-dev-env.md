<!-- maturity: draft
authors: Georgii Skorokhod, Niklas Dewally
created: 31-03-25
---- -->
# Setting up your development environment

Conjure Oxide supports Linux and Mac.

Windows users should install [WSL](https://learn.microsoft.com/en-us/windows/wsl/setup/environment#set-up-your-linux-username-and-password) and follow the Linux (Ubuntu) instructions below:


<details><summary><b>Linux (Debian/Ubuntu)</b></summary>

**The following software is required:**
- The latest version of stable Rust, installed using [Rustup](https://www.rust-lang.org/tools/install).
- A C/C++ compilation toolchain and libraries:
  - Debian, Ubuntu and derivatives: `sudo apt install build-essential libclang-dev`
  - Fedora: `sudo dnf group install c-development` and `sudo dnf install clang-devel`
* [Conjure](https://github.com/conjure-cp/conjure).
  - **Ensure that Conjure is placed early in your PATH to avoid conflicts with ImageMagick's `conjure` command!**

</details>

<details><summary><b>MacOS</b></summary>

**The following software is required:**
* the latest version of stable Rust, installed using [Rustup](https://www.rust-lang.org/tools/install).
* an XCode Command Line Tools installation (installable using `xcode-select --install`)
* CMake: `brew install cmake` (for SAT solving)
* [Conjure](https://github.com/conjure-cp/conjure).

</details>

<details><summary><b>St Andrews CS Linux Systems</b></summary>

1. Download and install the *pre-built binaries* for [Conjure](https://github.com/conjure-cp/conjure). Place these in `/cs/home/<username>/usr/bin` or elsewhere in your `$PATH`.

2. Install `rustup` and the latest version of Rust through `rustup`. 
   *The school provided Rust version does not work*.
   - By default, `rustup` installs to your local home directory; therefore, you may need to re-install `rustup` and Rust after restarting a machine or when using a new lab PC. 

</details>

---

### Improving Compilation Speed

Installing [sccache](https://github.com/mozilla/sccache) improves compilation speeds of this project by caching crates and C/C++ dependencies system-wide. 

* Install [sccache](https://github.com/mozilla/sccache) and follow the setup instructions for Rust. Minion detects and uses sccache out of the box, so no C++ specific installation steps are required.

---

*This section had been taken from the 'Setting up your development environment' page of the [conjure-oxide wiki](https://github.com/conjure-cp/conjure-oxide/wiki/Setting-up-your-development-environment)*