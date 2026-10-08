# Conjure Oxide in a browser

Build the standard `conjure-oxide` executable as WebAssembly, pass its usual
arguments and input files from JavaScript, and get its exit code, stdout and
stderr back. This is a small starting point for student integrations.

The build includes the normal Essence parser, rewriting rules, Minion, and
SAT with BatSat. Minion is the default. Each call runs in a fresh Web Worker,
so the page stays responsive and concurrent calls have separate filesystems.

## Build

To use an already-built `wasm-package`, skip this section and go to
[Browser example and checks](#minimal-page-and-browser-checks). These tools are needed
only to build the engine from source.

### Install the Rust target

Install Rust through [rustup](https://rustup.rs/) if needed. From the repository
root, run:

```sh
rustup target add wasm32-unknown-emscripten
```

This installs Rust's standard library for the Emscripten target, using the Rust
version selected by this repository's `rust-toolchain.toml`. Emscripten itself
is installed separately below.

### Install Emscripten

Choose one of these two installation methods.

**macOS with Homebrew already available:**

```sh
brew install emscripten
```

This is the [Homebrew package](https://formulae.brew.sh/formula/emscripten) used
on our development machine. If you cannot install packages through Homebrew,
use the SDK method below.

**macOS or Linux without administrator access:**

With Git and Python 3 available, install the official
[Emscripten SDK (`emsdk`)](https://emscripten.org/docs/getting_started/downloads.html)
in a directory you own:

```sh
git clone https://github.com/emscripten-core/emsdk.git "$HOME/emsdk"
cd "$HOME/emsdk"
./emsdk install 6.0.10
./emsdk activate 6.0.10
source ./emsdk_env.sh
```

No `sudo` is needed for this SDK installation. The commands select Emscripten
6.0.10, a version tested with this engine. Each time you open a new terminal,
make its tools available to that terminal with:

```sh
source "$HOME/emsdk/emsdk_env.sh"
```

Use Bash or Zsh for these commands. On Windows, use them inside an existing
WSL Linux environment; native Windows builds with this script have not been tested.
Building from source also needs native C/C++ build tools, libclang, and CMake
for the Rust dependencies. On a managed machine, use the tools provided by your
course or ask course staff if they are missing.

### Check the tools and build

Return to the repository root and run:

```sh
python3 --version
command -v emcc
emcc --version
emcc --check
rustup target list --installed
```

Python should report version 3, `command -v emcc` should print the compiler's
path, and `emcc --version` should identify Emscripten. `emcc --check` checks its
supporting tools; resolve any reported problems before building. The Rust target
list should include `wasm32-unknown-emscripten`. If `emcc` is not found after an
SDK installation, run the `source` command above in this terminal.

Then build the package:

```sh
bash tools/build-wasm.sh
```

The build writes a self-contained
package to `target/wasm-package`, using the configured Cargo target directory for
compiler outputs. An optional first argument chooses another package directory:

```sh
bash tools/build-wasm.sh /tmp/conjure-wasm
```

Builds have been tested with Rust 1.98.1, Emscripten 6.0.10, and
released `minion-sys` 0.1.2. The workspace pins Tree-sitter's upstream Emscripten
fix; no local vendor
patch is needed. On macOS the build selects Homebrew's Python 3.14 when available;
set `EMSDK_PYTHON` to override that selection. Rust's Emscripten linker currently
produces a harmless `WASM_BIGINT` deprecation warning with this SDK.

## JavaScript API

```js
import { run } from "./conjure/index.mjs";

const result = await run(
  ["solve", "/model.essence", "--solver", "sat", "-n", "1000"],
  { "/model.essence": "find x, y : int(1..5)\nsuch that x < y, x + y = 6\n" },
);
console.log(result.exitCode); // 0 on success
console.log(result.stdout);   // Conjure's ordinary output (Essence solutions here)
console.log(result.stderr);   // Conjure's diagnostics
```

`run(args, files?, options?)` returns a Promise of
`{ exitCode, stdout, stderr, elapsedMs, files }`. Do not include the executable name in
`args`. `files` maps paths to input text; paths such as `/model.essence` are in the
engine's private in-memory filesystem. Parent directories must already exist,
so root-level paths are the simplest choice. These are not files on the user's disk.
The package includes TypeScript declarations in `index.d.mts`.

Use ordinary CLI arguments to select solvers, limits, and output formats. For
example, `--output-format json` requests the CLI's JSON solution format, which
your page can parse with `JSON.parse(result.stdout)`. `await run(["--help"])`
returns the CLI help. To collect output files separately from stdout and stderr,
request their paths with `captureFiles`:

```js
const result = await run(
  ["solve", "/model.essence", "--output-format", "json", "-o", "/solutions.json"],
  { "/model.essence": "find x : int(1..3)\nsuch that x = 3\n" },
  { captureFiles: ["/solutions.json"] },
);
if (result.exitCode !== 0) throw new Error(result.stderr);
const solutions = JSON.parse(result.files["/solutions.json"]);
console.log(solutions); // [{ x: 3 }]
```

`result.files` maps the requested paths to UTF-8 text. Unrequested files are
discarded when the worker ends. Every requested file must exist, including after
a nonzero CLI exit code; a failed read rejects with an Error naming the path and
CLI exit code, together with any captured stderr. With no requested files,
`result.files` is an empty object.

Pass a separate parameter file by adding its path to the arguments:

```js
const result = await run(["solve", "/model.essence", "/model.param"], {
  "/model.essence": "given n : int(1..5)\nfind x : int(1..n)\nsuch that x = n\n",
  "/model.param": "letting n be 3\n",
});
```

A nonzero CLI exit code still resolves the Promise if requested files can be
read: inspect `exitCode` and `stderr` to report model errors or invalid arguments.
JavaScript does not
interpret model text or invent another solver result format. Worker startup,
Wasm loading, and filesystem failures reject with an Error identifying the step
that failed. Underlying engine diagnostics are included when available.

| Option | Default | Meaning |
| --- | --- | --- |
| `timeoutMs` | `30000` | Integer wall-clock deadline in milliseconds, including startup (1 to 2147483647) |
| `signal` | absent | An `AbortSignal` to stop the worker |
| `onStage` | absent | Callback receiving `"loading"` and `"running"` |
| `captureFiles` | `[]` | Paths of files to return as UTF-8 text after the CLI finishes |

Cancellation rejects with an `AbortError`; timeout rejects with a `TimeoutError`.
Both terminate the worker and discard partial output. Completed calls also
terminate their worker. For example:

```js
const controller = new AbortController();
const pending = run(["--help"], {}, { signal: controller.signal });
controller.abort();
try { await pending; } catch (error) { console.log(error.name); } // AbortError
```

## Minimal page and browser checks

[`minimal.html`](minimal.html) has three text boxes (Essence, optional parameters,
and output) and one Submit button. It needs no CSS, framework, npm installation,
or website build. Its script assembles arguments and files, calls `run`, and
shows the exit code and captured output. It writes solutions to `/solutions.json`,
captures that file, parses it with `JSON.parse`, and displays the JSON at the end.
Edit the argument array to try other CLI options.

After building, serve the complete package:

```sh
python3 -m http.server 8767 --bind 127.0.0.1 --directory target/wasm-package
```

Open `http://127.0.0.1:8767/minimal.html`. To integrate elsewhere, copy the complete
package directory and edit this HTML file. An already-built package needs no
Rust tooling to serve it. Keep `index.mjs`, `worker.mjs`, `conjure.mjs`, and
`conjure_oxide.wasm` together: their URLs resolve relative to the package,
including when served under a subpath. Serve over HTTP instead of opening a
`file:` URL. No cross-origin isolation headers or solver service are required.

For the browser regression suite, serve the repository root instead:

```sh
python3 -m http.server 8767 --bind 127.0.0.1
```

Open `http://127.0.0.1:8767/tools/wasm/tests/browser.html`. It covers raw CLI output,
empty and unsatisfiable models, parameters, both solvers, 1000 solutions, CLI
errors, cancellation, timeout, concurrent files, and worker startup failure.
To test another built package, add `?package=/path/to/package/index.mjs`.

The full React playground lives in the separate `conjure-oxide-web` repository.
It imports this complete package, including its declarations and worker assets:

```sh
# In the website repository, after building the package here:
npm ci
npm run import-engine -- /path/to/conjure-oxide/target/wasm-package
npm run dev
```

The website records the imported version and asset hashes in `engine.lock.json`.
It has no Rust dependencies or source imports from this checkout.

## Following a request

| File | Responsibility |
| --- | --- |
| `minimal.html` | Read the text boxes, assemble arguments and files, call `run`, display output. |
| `index.mjs` | Create a worker, handle messages, terminate it on completion, cancellation, or timeout. |
| `worker.mjs` | Load Emscripten's engine, write the inputs, call the standard CLI's `main`, capture stdout and stderr. |
| `../../crates/conjure-cp-cli/src/main.rs` | The existing native CLI entry point, also compiled for Wasm. |

The engine uses Emscripten's `FS` and `callMain`. The comments in `worker.mjs`
explain the Emscripten options; `createConjure` is its generated module factory.
There is no separate Rust executable or duplicated solving pipeline to maintain.

## Build features and limitations

The script compiles `conjure-cp-cli` with
`--no-default-features --features wasm-target`. This selects BatSat and omits
CaDiCaL, Z3, the language server, mimalloc, and extra rule checks. Requesting Z3
or `server-lsp` gives an ordinary error explaining that the feature was not
compiled in. Commands that launch external programs, such as
`--parser via-conjure`, compile but fail normally in a browser.

Model support and output are those of the standard CLI. Search completion
metadata, richer errors, streaming, and reusable workers can be student projects.
For command-line use outside the browser, use the native `conjure-oxide` executable.
See the [developer guide](../../docs/src/developers-guide/wasm.md) for backend,
Tree-sitter, and linking details.
