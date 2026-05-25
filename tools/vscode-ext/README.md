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

Current implementation detail:
- the server binary is expected at `target/release/conjure-oxide` relative to this repository layout,
- and is started with `conjure-oxide server-lsp`.

So before using it, build Conjure Oxide in release mode:

```bash
cargo build --release
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
