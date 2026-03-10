[//]: # (Author: Sebastian El Khoury)
[//]: # (Last Updated: 12/02/2026)

# Test GitHub Actions Locally With act

This guide shows how to run GitHub Actions workflows locally using act. It is useful for fast feedback on CI changes without waiting for a remote run.

For details on installation and advanced usage, see <https://nektosact.com/>.

## Quick start

1. Install `act` (see the official site for the latest install steps).
2. From the repo root, run a workflow event, for example:

```bash
act --container-architecture linux/amd64
```

This repo works best with the `catthehacker/ubuntu:act-latest` image. It includes common CI tooling and matches GitHub-hosted Ubuntu runners more closely than the default image. The `act` default image can be configured at runtime; see <https://nektosact.com/> for details.

## Common usage patterns

- Run a specific workflow file:

```bash
act -W .github/workflows/ci.yml --container-architecture linux/amd64
```

- Run a specific job:

```bash
act -j test --container-architecture linux/amd64
```

- Provide secrets locally (example):

```bash
act --secret-file .secrets --container-architecture linux/amd64
```

- Run the Vale job for pull requests:

```bash
act pull_request --job vale --container-architecture linux/amd64
```

In this repo, the `vale` job is part of the pull request workflows, so this command runs that job locally against a simulated `pull_request` event. It uses the recommended `catthehacker/ubuntu:act-latest` image by default, and you can configure the image at runtime; see <https://nektosact.com/usage/runners.html#alternative-runner-images/> for details.

## Limitations to keep in mind

- Workflow triggers: some events (`workflow_run`, `pull_request_target`, scheduled jobs) do not behave exactly like GitHub and may need extra inputs or manual flags.
- GitHub API usage: steps that call the GitHub API (releases, issues, PR comments, status checks) require valid tokens and may still diverge from real GitHub behavior.
- Deployment workflows: jobs that deploy (cloud credentials, OIDC, environment protection rules) are hard to reproduce locally and are better validated in a real GitHub run.
- Hosted services and caches: actions depending on GitHub-hosted caches or services may be no-ops or behave differently.
- Matrix and concurrency: complex matrices, concurrency groups, and reusable workflows can be partially supported but may require extra configuration or run differently.

When in doubt, use `act` for fast iteration, then confirm changes with a real GitHub Actions run.
