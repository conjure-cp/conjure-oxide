# Essence LSP

An Essence Language Server Protocol (LSP) extension for VS Code, built on the Conjure-Oxide parser.

## Features

- Essence language registration (`.essence`, `.eprime`)
- Syntactic error detection with diagnostics and underlining
- Semantic error detection with diagnostics and underlining
- Semantic highlighting
- Hover tooltips
- TextMate syntax highlighting

## Requirements

This extension launches the `conjure-oxide` language server process.

- if a local `conjure-oxide` binary exists at `target/release/conjure-oxide`, it is used;
- otherwise it falls back to `conjure-oxide` from your `PATH`.

To use the `PATH` fallback, install `conjure-oxide` and ensure it is available on your `PATH`.

From the roor of the Conjure-Oxide this repository, one option is:

```bash
cargo install --path crates/conjure-cp-cli
```

You can verify installation with:

```bash
conjure-oxide --help
```

## Development

From `tools/vscode-ext`:

```bash
npm install
npx @vscode/vsce package
```

To run in an Extension Development Host, use VS Code `Run Extension` from this folder/workspace.

## License

MPL-2.0. See `LICENSE`.
