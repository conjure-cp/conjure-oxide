[//]: # (Author: Calum Cooke)
[//]: # (Last Updated: 12/12/2025)

# Code Coverage

## Introduction
Code coverage is a test which shows how much of the codebase gets automatically tested. There are two main metrics - **Function** and **Line** - both given as percentage points. We use Rust's testing function which comes with cargo, and **GRCov** to generate the reports.

### Line Coverage
This is the easier of the two to understand. Line coverage gives the percentage of lines that were ran during testing of all the lines total.

### Function Coverage
There are many more functions than there may appear to the eye. (TODO: explain macro expansions, and the difference between a function and the various "implementations" of functions, and check that everything is actually valid. Also give an example of how a function expands to "multiple" functions.)


## How to Test
There are couple ways of interrogating code coverage:
1. Push your commits to the remote, and look at the PR
2. Run `tools/coverage.sh`, and open `target/debug/coverage/index.html` in a browser

The second option (the coverage script) gives a much more verbose breakdown for _which_ areas of the repo are starved of testing, breaking down to each and every line.

## How to Improve
There are numerous way of improving our current code coverage. Below are some some considerations.

### Test-as-you-go
TL;DR - leave the repo in at least as good a condition as you found it.
Make sure whenever you commit something, that you add sufficient tests for it such that the coverage isn't worse than before. This is very easy to forget; even though I started out with a code coverage project, when I moved on to adding a feature of my own I immediately forgot to add tests! 

### Search and Destroy
Use the [coverage script](#how-to-test) to find which files are badly covered, and add tests for them.

## Quirks and Implications
### Cloning (instead of Forking) the Repository
For the most up to date testing advice, look at the [contributing guide](https://github.com/conjure-cp/conjure-oxide/blob/main/CONTRIBUTING.md). As of Decemember 2025, we primarily work by cloning the main repository instead of working in our own forks. To prevent remote code execution, GitHub prevents a GH Action from running code on a fork. This means that our Action for reporting test coverage whenever you push your commits only works if your upstream branch is in the main repository (and not your own fork of it).

### Macros
The coverage script builds conjure-oxide, runs `cargo test`, then runs `grcov` to generate the report. Whenever conjure-oxide gets built, a lot of the macros get expanded (e.g. the derive macro may create multiple implementations of a function). These are treated as seperate functions in the report.
