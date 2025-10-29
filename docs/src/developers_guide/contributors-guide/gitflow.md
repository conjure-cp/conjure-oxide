<!-- TODO: Edit this -->

# Pull Requests

> [!TIP] 
> We Use [Github Flow](https://guides.github.com/introduction/flow/index.html), so All Code Changes Happen Through Pull Requests

Our development process is as follows:

1. Make a fork.
2. Create a branch on your fork, do not develop on main.
3. Create a pull request as soon as you want others to be able to see your progress, comment, and/or help:
   - Err on the side of creating the pull request too early instead of too late. 
     Having an active PR makes your work visible, allows others to help you and give feedback. Request reviews from people who have worked on similar parts of the project.
   - Keep the PR in draft status until you think it's ready to be merged.
4. Assign PR to reviewer(s) when it's ready to be merged.
   - Only Oz (@ozgurakgun) can merge PR's, so add him as a reviewer when you want your PR to be merged.
   - During reviewing, avoid force-pushing to the pull request, as this makes reviewing more difficult. 
     Details on how to update a PR are given below.
5. Once Oz has approved the PR:
   - Update your PR to main by rebase or merge. This can be done through the Github UI or locally.
   - Cleanup your git history (see below) or request your PR to be squash merged.

# Style

- Run `cargo fmt` in the project directory to automatically format code
- Use `cargo clippy` to lint the code and identify any common issues

See: [[Documentation Style]] and [[Rust Coding Style]] (TODO)

# Commit and PR Titles 

We use [Semantic PR / Commit messages](https://gist.github.com/joshbuchea/6f47e86d2510bce28f8e7f42ae84c716).

Format: `<type>(<scope>): <subject>`
(`<scope>` is optional)

## Example 

```
feat(parser): add letting statements
^--^ ^----^   ^--------------------^
|    |        |
|    |        +--> Summary in present tense.
|    |
|    +--> Area of the project affected by the change.
|
+-------> Type: chore, docs, feat, fix, refactor, style, or test.
```

## Types 

- `feat`: new features for the end user
- `chore`: changes to build scripts, CI, dependency updates; does not affect production code
- `fix`: fixing bugs in production code
- `style`: purely stylistic changes to the code (e.g. indentation, semicolons, etc) that do not affect behaviour
- `refactor`: changes of production code that do not add new features or fix specific bugs
- `test`: adding, updating, or refactoring test code
- `doc`: adding or updating documentation

# PR Messages

Your pull request should contain a brief description explaining:
- What changes you are making
- Why they are necessary
- Any significant changes that may break other people's work

Additionally, you can link your PR to an issue. For example: `closes issue #42`.

# Amending your PR and Force Pushes

You should avoid rebasing, amending, and force-pushing changes during PR review.
This makes code review difficult by removing the context around code review comments and changes to a commit.

The recommended way to update PRs is to use git's [built-in support for fixups](https://git-scm.com/docs/git-commit#Documentation/git-commit.txt---fixupamendrewordltcommitgt).

To make a change to a commit (e.g. addressing a code review comment):

```sh
git commit --fixup <commit>
git push
```

Once your PR is ready to merge, these fixup commits can be merged into their original commits like so:

```sh
git rebase --autosquash main
git push --force
```

We have CI checks to block accidental merging of `fixup!` commits.

See:
- https://rietta.com/blog/git-rebase-autosquash-code-reviews/
-  https://git-scm.com/docs/git-commit#Documentation/git-commit.txt---fixupamendrewordltcommitgt

# Before your PR is merged

When your PR is approved, you may need to [rebase](https://git-scm.com/docs/git-rebase#_description) your branch onto main before it can be merged. Rebasing essentially adds all the latest commits from main to your branch if it has fallen behind main.

To do this:

1. Make sure that your `main` branch is synced to the main repo
2. Switch to the branch you're making the PR from
3. Do:
   ```sh
   git rebase main
   git push --force
   ```

# (Optional) Cleaning up your Git history

Additionally, if you are proficient with git, you can use interactive rebase to clean up your commit history.
This allows you to reorder, drop, or amend commits arbitrarily.

See:

- [How to keep your Git history clean with interactive rebase](https://about.gitlab.com/blog/2020/11/23/keep-git-history-clean-with-interactive-rebase/)
- [7.6 Git Tools - Rewriting History](https://git-scm.com/book/en/v2/Git-Tools-Rewriting-History)

There are some GUI tools to help you do that, such as the [GitHub Client](https://github.com/apps/desktop), [GitKraken](https://www.gitkraken.com/), various VS Code extensions, etc.

> [!WARNING]  
> Interactive rebase and force-pushing overwrites your git history, so it can be destructive.
> This is also not a requirement!

# Squashing PRs

Alternatively, you can ask for the PR to be "squashed".
This combines all your commits into one merge commit.
Squashing PRs helps keep the commit history on main clean and logical without requiring you to go back and  manually edit your commits!