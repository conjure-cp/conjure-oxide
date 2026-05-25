# Changelog

All notable changes to this extension will be documented in this file.

## 0.1.4

- Added semantic token contributions to the published manifest:
  - `semanticTokenTypes` for `givenVar`, `findVar`, and related custom tokens
  - `semanticTokenScopes` mappings so themes can style custom token types consistently
  - default semantic highlighting configuration parity with dev manifest

## 0.1.3

- Fixed release-mode startup failures caused by spawning the server in a non-writable working directory.
- Added `conjureOxide.serverPath` setting to pin the exact `conjure-oxide` executable.
- Improved server binary resolution order:
  - configured `conjureOxide.serverPath`
  - workspace `target/release/conjure-oxide`
  - extension-bundled binary
  - `conjure-oxide` on `PATH`
- Added startup logging of resolved server command and working directory to aid diagnostics.

## 0.1.0

- Initial Marketplace-ready manifest setup.
- Syntactic error detection with diagnostics and underlining
- Semantic error detection with diagnostics and underlining
- Semantic highlighting
- Hover tooltips
- Fallback TextMate syntax highlighting
- Language server startup supports both:
  - local repo binary at `target/release/conjure-oxide` (when present), and
  - fallback to `conjure-oxide` from `PATH` (`conjure-oxide server-lsp`)
