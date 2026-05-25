# Changelog

All notable changes to this extension will be documented in this file.

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
