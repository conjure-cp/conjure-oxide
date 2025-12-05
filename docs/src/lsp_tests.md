<!--
status: draft
author: Yi Xin Chong
last updated: 3-12-2025 1:39am
-->

# Frontend syntactic and semantic testing
## Introduction
The frontend of Essence, the language of Conjure Oxide, include a server that uses a Language Server Protocol (LSP) to check Essence files for errors before the file is parsed. There are two types of error when trying to diagnose a given Essence file, which are syntactic and semantic errors. Syntactic errors are errors that stem from the tokens in the Essence file not being in the correct syntax when given into the parser. Semantic errors are errors that may pass the syntactic error checking (i.e. the file lines have the correct syntax), but ultimately is unable to be parsed due to errors relating to the entire context of the file. To be able to accurately diagnose the Essense files for errors, tests cases have been written for situations that Diagnostics API and Native parser might encounter during diagnosing and parsing.

## Testing
Test cases that test the coverage of the Native parser are seperated into the syntax error and semantic error directories in roundtrip, and uses Roundtrip testing to call only the Native parser (and not the legacy parser) when testing. For testing the Diagnostics API, test cases are written in the tests directory of the `conjure-cp-essence-parser` directory which uses normal Rust unit testing. The sections below describe the test cases that the Native parser and Diagnostics API should be able to pass in order to be a successful frontend system.

### Syntactic error types
#### Missing tokens
#### Invalid tokens
#### Unexpected tokens

### Semantic error types
#### Keyword used as identifier
#### Undeclared variables
#### Incorrect variable type for expression
#### Unsafe division expression
#### Duplicate declaration of a variable