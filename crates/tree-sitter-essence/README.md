## `tree-sitter-essence`

`tree-sitter-essence` provides a [tree-sitter](https://tree-sitter.github.io/tree-sitter/) grammar for the Essence constraint-modelling language used by `conjure-oxide`.

The grammar defines how Essence source is parsed into a concrete syntax tree, including the core language structure: declarations (for example `find`, `given`, and `letting`), domains, expressions, constraints, comprehensions, and literals. The grammar is based on the [Essence Docs](https://conjure.readthedocs.io/en/latest/essence.html#) and is not complete. This parse tree is then used by higher-level parser code (in the `conjure-cp-essence-parser` crate) to build semantic model representations.

## Usage
After changing `grammar.js`, run `tree-sitter generate --abi 15` using CLI version 0.26.7 and commit the generated files. The exact CLI version is pinned in `package.json` and CI to keep generation reproducible. Run `tree-sitter test` to check the grammar corpus.

### Licence

This project is licenced under the [Mozilla Public Licence
2.0](https://www.mozilla.org/en-US/MPL/2.0/).
