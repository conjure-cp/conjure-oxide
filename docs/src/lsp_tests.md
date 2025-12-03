<
status: draft
author: Yi Xin Chong
last updated: 3-12-2025 1:39am
>

# LSP Diagnostics API syntactic and semantic testing
## Introduction
The Language Server Protocol for Essence calls the Diagnostics API to diagnose the Essence file and return any errors found in the file. To be able to accurately diagnose the Essense files for errors, tests cases have been written for situations that Diagnostics API might encounter during diagnosing.

## Testing
There are two types of error when trying to diagnose a given Essence file, which are syntactic and semantic errors. Syntactic errors are errors that stem from the tokens in the Essence file not being in the correct syntax when given into the parser. Semantic errors are errors that may pass the syntactic error checking (ie. the file lines have the correct syntax), but ultimately is unable to be parsed due to errors relating to the entire context of the file.

### Syntactic error types
#### Missing tokens
#### Invalid tokens
#### Unexpected tokens

### Semantic error types
#### Keyword used as identifier
#### Undeclared variables
#### Incorrect variable type for expression
#### Unsafe division expression
#### Redeclaration of variables