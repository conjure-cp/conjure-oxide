# WebAssembly interface and Emscripten builds

The WebAssembly build compiles the standard `conjure-oxide` executable for
`wasm32-unknown-emscripten`. A small browser wrapper passes CLI arguments and
input files, then returns the exit code, stdout, and stderr. The normal Essence
parser, released `minion-sys` 0.1.2, and BatSat are included.

## Build and try the minimal page

First follow [Install the build tools](#install-the-build-tools), then run from
the repository root:

```sh
bash tools/build-wasm.sh
python3 -m http.server 8767 --bind 127.0.0.1 --directory target/wasm-package
```

Open `http://127.0.0.1:8767/minimal.html`. It has three text boxes and a Submit
button, with no CSS or npm setup. The build writes a self-contained
`target/wasm-package` directory. An optional first argument to the build script
chooses another package directory. Copy the whole directory into your application.

## Use the JavaScript interface

```js
import { run } from "./conjure/index.mjs";
const result = await run(
  ["solve", "/model.essence", "--solver", "sat", "-n", "20"],
  { "/model.essence": "find x, y : bool\nsuch that x != y\n" },
);
console.log(result.exitCode, result.stdout, result.stderr);
```

`run(args, files?, options?)` accepts ordinary CLI arguments without the executable
name and a map of input paths to text. Files live in each worker's private
in-memory filesystem. For a parameter file, supply its text in `files` and add
its path after the model path, just as with the native CLI. Use
`--output-format json` if your page wants to parse JSON instead of displaying
the usual Essence output. `await run(["--help"])` returns CLI help.

Results contain `exitCode`, `stdout`, `stderr`, `elapsedMs`, and `files`. Pass
`captureFiles: ["/solutions.json"]` in the options to return that file's UTF-8
text in `result.files["/solutions.json"]`, for example after writing JSON solutions
with `--output-format json -o /solutions.json`. Requested files are read before
the worker terminates; missing files reject with an error identifying the path
and CLI exit code. With no requested files, `files` is an empty object.
A nonzero exit code resolves normally with the CLI's diagnostics if requested
files can be read. Worker, Wasm loading, and
filesystem failures reject the Promise. Optional `signal` supports cancellation;
`timeoutMs` defaults to 30000 and includes startup. Cancellation and timeout
terminate the worker, discard partial output, and reject with `AbortError` and
`TimeoutError` respectively. `onStage` receives `"loading"` and `"running"`.
Each invocation has a fresh worker and engine; concurrent calls are independent
and leave the page responsive.

See [`tools/wasm/README.md`](../../../tools/wasm/README.md) for the full API and
student integration instructions. To run the browser regression suite, serve
the repository root over HTTP and open `/tools/wasm/tests/browser.html`.
The full React playground is maintained in the separate `conjure-oxide-web`
repository, which imports the built package without Rust dependencies.

## CLI build features

The script compiles `conjure-cp-cli` with
`--no-default-features --features wasm-target`. This selects BatSat; Minion
remains available. Z3, the language server, mimalloc, and extra rule checks are
omitted. Requesting Z3 or `server-lsp` reports that support was not compiled in,
with the feature needed to enable it. These feature choices also work natively.

The generated `conjure.mjs` exposes Emscripten's `FS` and `callMain` to
`worker.mjs`, which writes the input files, runs the standard `main`, and captures
stdout and stderr. There is no dedicated Wasm Rust executable. CLI parsing,
parameters, solution limits, output formats, and help use the same code as native
builds. Commands that launch external programs, such as `--parser via-conjure`,
compile but return an operating-system error in a browser.

## Library backend features

Minion remains available unconditionally. SAT is also always available in a
successful library build: select exactly one backend at compile time, `sat-cadical` or `sat-batsat`.
Native defaults select CaDiCaL and enable Z3. To select BatSat, disable defaults
and enable `sat-batsat` on `conjure-cp` / `conjure-cp-rules`. Re-enable `z3`
explicitly if needed on native builds. Selecting neither or both SAT features is
a compile error; consequently `--all-features` is not a supported configuration.

Wasm requires `sat-batsat`. CaDiCaL is a native-only dependency. Cargo features
are additive, so dependencies must agree on the target backend. Parser features
forward the same choice. Procedural macros run on the host and use the parser's
native default backend independently of the Wasm target's selection.

The common `Sat` adaptor API is unchanged. BatSat supports timeout termination
through its callback API; nonzero solver seeds return an explicit unsupported
error rather than being ignored. Default seed zero uses BatSat's defaults.
These features select the library backend. CLI builds expose the same backend
choices, together with optional `z3`, `lsp`, and `mimalloc` features. Defaults
retain CaDiCaL, Z3, the language server, and mimalloc on native builds. The
language server itself still uses its native dependencies.

## Install the build tools

If you have an already-built package, you can serve it and use the JavaScript
interface without installing Rust or Emscripten. Building from source needs
Python 3 and native C/C++ build tools, libclang, and CMake for Rust dependencies.

With Rust installed through [rustup](https://rustup.rs/), run this from the
repository root to install the target for the repository's selected Rust version:

```sh
rustup target add wasm32-unknown-emscripten
```

This installs Rust's target standard library. Install Emscripten separately.
On macOS with Homebrew available, use the
[Homebrew package](https://formulae.brew.sh/formula/emscripten):

```sh
brew install emscripten
```

For macOS or Linux without administrator access, use the official
[Emscripten SDK installation instructions](https://emscripten.org/docs/getting_started/downloads.html).
With Git and Python 3 available, these commands install the SDK in your home
directory without `sudo`:

```sh
git clone https://github.com/emscripten-core/emsdk.git "$HOME/emsdk"
cd "$HOME/emsdk"
./emsdk install 6.0.10
./emsdk activate 6.0.10
source ./emsdk_env.sh
```

The commands select a tested Emscripten version. In each new Bash or Zsh terminal,
run `source "$HOME/emsdk/emsdk_env.sh"` again before building. Windows users can
use an existing WSL Linux environment; native Windows builds have not been tested.
On a managed machine, ask course staff about any missing native build tools.

Return to the repository root and check the installation:

```sh
python3 --version
command -v emcc
emcc --version
emcc --check
rustup target list --installed
```

These should identify Python 3, the Emscripten compiler and version, and
`wasm32-unknown-emscripten` in the installed target list. `emcc --check` verifies
Emscripten's supporting tools. If `emcc` is not found, source the SDK environment
in this terminal. Resolve any check errors before running `bash tools/build-wasm.sh`.

## Tree-sitter dependency fix

Tree-sitter 0.27.0 incorrectly enables its bare-Wasm libc replacements for
Emscripten. The workspace and lockfile pin upstream revision
`b2f00cb0b5942ff10c45dc77d3a9c7e9c7515da8`, which limits those replacements to
`wasm32-unknown-unknown`. The runtime declares version 0.28.0. Its language crate
is pinned to the same source to keep grammar and runtime types consistent.
No manifest edits, local vendor patch, or command-line Cargo override are needed.
Downstream Rust workspaces must carry the same patches until an upstream release
contains this fix; Cargo does not inherit patches from dependencies.

## Engine linking

If `CARGO_TARGET_DIR` is overridden, use that output directory instead of `target`.
Use the Python configured by the Emscripten SDK; set `EMSDK_PYTHON` explicitly if
its launcher picks an incompatible Python installation.

The linker wrapper uses `--whole-archive` for the built-in rules archive, retaining
objects referenced only through inventory constructors. Additional downstream
rule crates would need equivalent retention. Emscripten runs the constructors at
startup; no explicit registry or initialisation call is needed in Rust.

The C++ link setting supplies Minion's runtime. The larger stack supports parser,
rewriter and solver recursion. Memory growth allows Minion to allocate its search
storage rather than abort at the default fixed heap limit. `std::time::Instant`
works through Emscripten, so this path does not need a Wasm timing replacement.
