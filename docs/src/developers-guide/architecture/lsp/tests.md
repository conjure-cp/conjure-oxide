<!--
author: Yi Xin Chong, Soph Morgulchik
last updated: 24-05-2026
-->

# LSP Error Testing
## Introduction
Conjure Oxide includes a server that uses a Language Server Protocol (LSP) to check Essence files for errors before the file is parsed. The LSP server communicates with the Diagnostics API to check for errors, and it will return the error message along with the range of where the error occurred in the given Essence file. The server will then use the diagnosis to perform error underlining, and syntax and semantic highlighting (more details in [LSP Documentation](server-client-model.md) and [Diagnostics API documentation](diagnostics-api.md)). 

There are two types of error when trying to diagnose a given Essence file, which are **syntactic** and **semantic errors**:

- **Syntactic errors** are errors that stem from the tokens in the Essence file not being in the correct syntax when given into the parser. 
- **Semantic errors** are errors that may pass the syntactic error checking (i.e. the file lines have the correct syntax), but ultimately is unable to be parsed due to errors relating to the entire context of the file. 

To be able to accurately diagnose the Essence files for errors, tests cases have been written for situations that [Diagnostics API](diagnostics-api.md) and [Tree-Sitter parser](../essence-parser/index.md) might encounter during diagnosing and parsing, referencing the [Essence Error Detection](../essence-parser/error-detection/error-detection.md).

## Testing
### Parser
Test cases that test the coverage of the tree-sitter parser use [Roundtrip testing](../../testing/roundtrip-testing.md). The `syntax-error` and `semantic-error` test directories exist in the `test-integration` crate as child directories of `roundtrip\invalid`. `config.toml` sets which parsers to run the test with: "via-conjure" or "tree-sitter"

These test cases will run alongside all the integration tests in the `tests-integration` crate, which can be done by using:
```bash
cargo test
```

Alternatively, to run only the Roundtrip tests, use the command:
```bash
cargo test tests-integration --test roundtrip_tests 

```

### Diagnostics API
For testing the Diagnostics API, test cases are written in the `tests` directory of the `conjure-cp-essence-parser` crate, which uses normal [Rust unit testing](https://doc.rust-lang.org/book/ch11-01-writing-tests.html). To run these tests, use the command:
```bash
cargo test -p conjure-cp-essence-parser
```

Or, to run the test files individually, use the command:
```bash
cargo test -p conjure-cp-essence-parser --test {file_name}
```