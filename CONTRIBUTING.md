# Contributing to Conjure Oxide
We love your input! We want to make contributing to this project as easy and transparent as possible, whether it's:

- Reporting a bug
- Discussing the current state of the code
- Submitting a fix
- Proposing new features
- Becoming a maintainer

## We Develop with Github
We use github to host code, to track issues and feature requests, as well as accept pull requests.

## Setting up your Development Environment
For information on how to set up your environment, go to the [Contributor's Guide](https://github.com/conjure-cp/conjure-oxide/wiki/Setting-up-your-development-environment)

## We Use [Github Flow](https://docs.github.com/en/get-started/using-github/github-flow), so All Code Changes Happen Through Pull Requests
Pull requests are the best way to propose changes to the codebase (we use [Github Flow](https://docs.github.com/en/get-started/using-github/github-flow)). We actively welcome your pull requests:

1. Make a fork.
2. Create a branch on your fork, do not develop on main.
3. Create a pull request as soon as you want others to be able to see your progress, comment, and/or help. Err on the side of creating the pull request too early instead of too late. Having an active PR makes your work visible, allows others to help you and give feedback. Request reviews from people who have worked on similar parts of the project.
4. Keep the PR in draft status until you think it's ready to be merged.
5. Assign PR to reviewer(s) when it's ready to be merged.
    - Only Oz (@ozgurakgun) can merge PR's, so add him as a reviewer when you
      want your PR to be merged.
    - During reviewing, avoid force-pushing to the pull request, as this makes
      reviewing more difficult. Details on how to update a PR are given below.
6. Once Oz has approved the PR:
    * Cleanup your git history (see below) or request your PR to be squash merged.
    * Update your PR to main by rebase or merge. This can be done through the
      Github UI or locally.

### Rebasing Pull Requests and Force Pushes

You should avoid rebasing, amending, and force-pushing changes during PR
review. This makes code review difficult by removing the context around code
review comments and changes to a commit. 

Doing this is probably OK on a WIP PR which hasn't had any reviews yet.

### Updating a Pull Request 

You should avoid rebasing, amending, or force-pushing to a PR. When updating a
pull request you should push additional "fixup" commits to your branch instead.

Once your PR is ready to merge (i.e. approved by Oz), you should cleanup and
rebase your PR and force push. 

The recommended way to update PRs is to use gits [built-in support for
fixups](http://git-scm.com/docs/git-commit#Documentation/git-commit.txt---fixupamendrewordcommit).

To make a change to a commit (e.g. addressing a code review comment):

```
git commit --fixup <commit>
git push
```

Once your PR is ready to merge, these fixup commits can be merged into their
original commits like so: 

```
git rebase --autosquash main
git push --force
```

We have CI checks to block accidental merging of `fixup!` commits.


See: 
 * https://rietta.com/blog/git-rebase-autosquash-code-reviews/
 * https://git-scm.com/docs/git-commit#Documentation/git-commit.txt---fixupamendrewordltcommitgt



## What We Didn't Do
To prevent unknown unknowns, skim the documentation and [What We Didn't Do](https://github.com/conjure-cp/conjure-oxide/wiki/What-We-Didn%27t-Do).

## Any contributions you make will be under the Mozilla Public License
In short, when you submit code changes, your submissions will be understood to be under the same [Mozilla Public License](https://www.mozilla.org/en-US/MPL/2.0/) that covers the project. Feel free to contact the maintainers if that's a concern.

## Report bugs using Github's [issues](https://github.com/conjure-cp/conjure-oxide/issues)
We use GitHub issues to track public bugs. Report a bug by [opening a new issue](https://github.com/conjure-cp/conjure-oxide/issues/new); it's that easy!

## Write bug reports with detail, background, and sample code
Take a look at [this advice from Simon Tatham](https://www.chiark.greenend.org.uk/~sgtatham/bugs.html) for how to effectively report Bugs. [This is an example](https://stackoverflow.com/q/12488905/180626) of a well written bug report. 

**Great Bug Reports** tend to have:

- A quick summary and/or background
- Steps to reproduce
  - Be specific!
  - Give sample code if you can. [My StackOverflow question](https://stackoverflow.com/q/12488905/180626) includes sample code that *anyone* with a base R setup can run to reproduce what I was seeing
- What you expected would happen
- What actually happens
- Notes (possibly including why you think this might be happening, or stuff you tried that didn't work)

People *love* thorough bug reports. I'm not even kidding.

## Coding Style
- Run `cargo fmt` in the project directory to automatically format code
- Use `cargo clippy` to lint the code and identify any common issues

## License
By contributing, you agree that your contributions will be licensed under its Mozilla Public License.

## References
This document was adapted from [this template](https://gist.github.com/briandk/3d2e8b3ec8daf5a27a62).
