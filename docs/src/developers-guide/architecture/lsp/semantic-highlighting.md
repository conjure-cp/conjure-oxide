[//]: # (Author: Yi Xin Chong)
[//]: # (Last Updated: 13/5/2026)

# Semantic Highlighting for the LSP Server

## Overview

Semantic highlighting is an LSP feature that provides context-aware token styling. Unlike syntax highlighting, which relies solely on TextMate grammar patterns, semantic highlighting understands the meaning and usage of tokens within the program.

This implementation extends the existing TextMate-based syntax highlighting and falls back to syntax highlighting whenever a semantic style is not defined by the active theme. Semantic tokens are generated based on prior declarations, allowing variables to be styled differently depending on how they were declared and used throughout the source file.

## Key Functions

### Token Encoding

During parsing, the semantic meaning of each token is stored in the `HoverInfo` of the `SourceMap` as a `SymbolKind` (see list below). Specifically when parsing variables, they are assigned a `DeclarationKind` during the initial declaration of the variable, which is stored in the symbol table. When used in expressions, the `DeclarationKind` is then mapped to either a `FindVar`, `LettingVar`, or `GivenVar` and stored in the `SourceMap`, which allows different styling of the variable based on its declaration type.

List of available `SymbolKinds` in `diagnostics_api.rs`:
- Integer
- Decimal
- Function
- Letting
- Find
- Variable
- Constant
- Domain
- FindVar 
- LettingVar
- Given
- GivenVar

Semantic tokens are encoded using the `encode_semantic_tokens()` function in `semantic_tokens.rs`. The function processes each token in the `SourceMap` and converts it into a semantic token by calling `token_encoding()`. This helper maps each `SymbolKind` to a corresponding `TokenEncoding`.

Most editor themes do not provide dedicated styles for Essence variables. To visually distinguish variables by declaration type, Essence declarations are mapped onto standard semantic token classifications commonly used in other languages but otherwise unused in Essence.

The current mappings are shown below:

| Declaration kind(s)                                    | Symbol kind | Semantic token classifier | Fallback TextMate grammar          |
| ------------------------------------------------------ | ----------- | ------------------------- | ---------------------------------- |
| Find                                                   | FindVar     | property                  | variable.other.property.findVar    |
| ValueLetting<br>TemporaryValueLetting<br>DomainLetting | LettingVar  | variable                  | variable.other.constant.lettingVar |
| Given                                                  | GivenVar    | string                    | string.other.givenVar              |

Additionally, `find` variables are visually emphasised because they represent the variables the solver is attempting to solve for. An underline style is therefore hard-coded in `package.json` for tokens classified as `FindVar`.

### LSP Semantic Highlighting Handler

The semantic highlighting handler retrieves the `SourceMap` from cache and passes it to `encode_semantic_tokens()` to generate semantic tokens. These encoded tokens are then returned to the LSP server, which forwards them to the client. The client uses the semantic token classifications together with the active theme to determine how each token should be rendered.

## Expanding Semantic Highlighting
To expand on the current semantic highlighting available, the files that will need to be changed are: `crates/conjure-cp-essence-parser/src/diagnostics/diagnostics_api.rs`, `crates/conjure-cp-essence-parser/src/diagnostics/semantic-tokens.rs`, `crates/conjure-cp-lsp/src/server.rs`, and `package.json`

1. In `diagnostics_api.rs`, add the new symbol kind to the `SymbolKind` enum.
2. In `semantic_token.rs`, add a new constant variable for the new symbol kind and create a mapping from the symbol kind to a token in the `token_encoding()` function.
3. In `server.rs`, add the new symbol kind to the server's `semantic_toke_provider`.
4. In `package.json`, add the new symbol kind to the semantic token scopes to specify its styling in TextMate, and if it is a custom token, add a new field for it in the semantic token types

## Semantic Highlighting Customisation (VSCode)
To customise semantic highlighting styles, open up `settings.json` in VSCode, and add the following code snippet if it does not exist in the file:

```bash
"editor.semanticTokenColorCustomizations": {
  "rules": {
    "{token_name}": "{styles}",
  }
}
```