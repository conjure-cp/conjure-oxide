<!-- maturity: draft
authors: Niklas Dewally, Georgii Skorokhod
last-updated: 08-02-25
---- -->

# Contributing Process 

- Make a fork of this repository. 
  (Use the "fork" button in the top right corner)
- Clone the forked repository to your machine:
  `git clone https://github.com/<yourname>/conjure-oxide.git`
- Keep your fork in sync with the main repo using the "sync fork" button on GitHub
- When starting work on a new feature, *never* commit directly to main.
  Instead, create a new branch on your fork (using the GitHub UI or the `git branch <branch name>` command)
- Commit your code frequently and use sensible commit messages
  (See: TODO)
- Open a "pull request" (PR) from your branch back to the main repository.
  This can be done via the "contribute! button on GitHub.
- Write a PR message explaining what changes you are making and why.
- Keep the PR in draft status until you think it's ready to be merged.
- When you think your code is ready to be merged:
  - Switch your PR from "draft" to "ready for review"
  - Add Oz as a reviewer so he can review your code, give feedback, and merge your work
- Make sure that all CI check pass before merging any code to the main repo

To keep the commit history on the main repository clean, it is good practice to do one of the following:

- If you're comfortable with using git, you can tidy up your commit history using "fixup commits":
  - To make a change to a commit (e.g. addressing a code review comment):
    ```sh
    git commit --fixup <commit>
    git push
    ```
  - Once your PR is ready to merge, these fixup commits can be merged into their original commits like so:
    ```sh
    git rebase --autosquash main
    git push --force
    ```
  - See [[Git Flow]] for more info!
- Otherwise, make sure that you "squash" all your commits into a single merge commit.
  You can do this by selecting "squash and merge" in the GitHub UI.

Having a clean commit history with sensible messages helps us understand when and why changes were made!


* To prevent unknown unknowns, skim the documentation and [What We Didn't Do](https://github.com/conjure-cp/conjure-oxide/wiki/What-We-Didn't-Do).

---

*This section had been taken from the 'Contributing Process' page of the [conjure-oxide wiki](https://github.com/conjure-cp/conjure-oxide/wiki/Contributing-Process)*