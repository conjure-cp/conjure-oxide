# Contributing to Conjure Oxide
We love your input! We want to make contributing to this project as easy and transparent as possible, whether it's:

- Reporting a bug
- Discussing the current state of the code
- Submitting a fix
- Proposing new features
- Becoming a maintainer

## Where are we
- We use GitHub to host code, to track issues and feature requests, as well as accept pull requests.
- There is also [Conjure Zulip](https://conjure.zulipchat.com/join/gtrnmpmlnzbfwgjgps26vbzc).
- At St Andrews, we are running a [Vertically Integrated Project on Conjure](https://ozgurakgun.github.io/vip). If you are a student on this project, you will also be added to an MS Teams team.

## GitHub Flow

We Use [GitHub Flow](https://docs.github.com/en/get-started/using-github/github-flow), so all code changes happen through pull requests

Pull requests are the best way to propose changes to the codebase (we use [GitHub Flow](https://docs.github.com/en/get-started/using-github/github-flow)). We actively welcome your pull requests:

1. Fork the repo, create a branch, do not develop on `main`.
2. Create a pull request as soon as you want others to be able to see your progress, comment, and/or help. Err on the side of creating the pull request too early instead of too late. Having an active PR makes your work visible, allows others to help you and give feedback. Request reviews from people who have worked on similar parts of the project.
3. Keep the PR in draft status until you think it's ready to be merged.
4. Assign PR to reviewer(s) when it's ready to be merged.
   - If you are working as part of a group, ask for code reviews from the rest of the group. Once everyone in the group is happy, proceed to the next step.
   - Only Oz (@ozgurakgun) can merge PRs, so add him as a reviewer when you want your PR to be merged.
5. Getting your PR merged.
   - We squash-merge PRs by default. Do not worry about keeping history on your PR branch "clean".
   - During reviewing, avoid force-pushing to the pull request, as this makes reviewing more difficult. You may force-push on draft PRs if you really want to.
   - If you keen to keep history clean in the PR, consider using fixup commits using Git's [built-in support for fixups](http://git-scm.com/docs/git-commit#Documentation/git-commit.txt---fixupamendrewordcommit).
     - You can make changes to a commit by running `git commit --fixup <commit>`.
     - You can squash fixup commits on top of their original commits by running `git rebase --autosquash main` and then force pushing to the branch.
     - We have CI checks to block accidental merging of `fixup!` commits.
     - Also see [this](https://rietta.com/blog/git-rebase-autosquash-code-reviews/) and [this](https://git-scm.com/docs/git-commit#Documentation/git-commit.txt---fixupamendrewordltcommitgt).

## Reporting bugs
We use GitHub issues to track public bugs. Report a bug by [opening a new issue](https://github.com/conjure-cp/conjure-oxide/issues/new); it's that easy!

Write bug reports with detail, background, and sample code. Take a look at [this advice from Simon Tatham](https://www.chiark.greenend.org.uk/~sgtatham/bugs.html) for how to effectively report Bugs. [This is an example](https://stackoverflow.com/q/12488905/180626) of a well written bug report. 

Great bug reports tend to have:
  - A quick summary and/or background
  - Specific steps to reproduce
  - What you expected would happen
  - What actually happens
  - Notes (possibly including why you think this might be happening, or stuff you tried that didn't work)

People *love* thorough bug reports. I'm not even kidding.

## Hygiene: code layout, style, linting
- Run `make check` in the project directory to automatically check for hygiene.
- Run `make fix` to apply fixes automatically.
- There is also `make fix-dirty` which works when there are uncommitted changes.

## House style

These conventions are more specific than the general coding style above.

### Language

- Use British spelling in prose, comments, rustdocs, commit messages, and user-facing text.
- Keep comments and rustdocs brief. Explain intent or behaviour, not obvious syntax.
- Use ASCII characters unless there is a justified need to use non-ASCII characters.

### Rust Code

- Document all top-level public functions, structs, enums, and type aliases with brief rustdocs.
- Document public fields and enum variants when their meaning is not completely obvious.

### Test Runs

- Use `MAX_EXPECTED_TIME=N` to only run test cases that are expected to take up to `N` seconds. Omit or use `MAX_EXPECTED_TIME=0` to run all tests.
- Use `TEST_CASE_TIMEOUT=N` to set a per-test timeout for integration tests.
- When recording artefacts from a timeout-bounded run, record the timeout value explicitly in the relevant `config.toml` file.

### Commits

- Never commit code changes and test artefact/config updates in the same commit.
- Commit code and harness changes first, with a normal semantic commit message.
- Commit generated or recorded test updates separately, using
  `chore(test-suite): ...` in the subject line.
- Reserve `test(test-suite):` for changes to test code or harness logic.






# The Book

Exists.

## What We Didn't Do
To prevent unknown unknowns, skim the documentation and [What We Didn't Do](https://github.com/conjure-cp/conjure-oxide/wiki/What-We-Didn%27t-Do).

## License
Any contributions you make will be under the Mozilla Public License. When you submit a PR, your submissions will be understood to be under the same [Mozilla Public License](https://www.mozilla.org/en-US/MPL/2.0/) that covers the project. Feel free to contact the maintainers if that's a concern.

## References
This document was adapted from [this template](https://gist.github.com/briandk/3d2e8b3ec8daf5a27a62) and then significantly edited with project specific information.
