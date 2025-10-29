<!-- TODO: Edit this -->

* This document lists common patterns and issues we've had in our Github actions, and practical solutions to them. 
* The first point of reference should be the official documentation, but if that is ever unclear, here would be a good place to look!

Terminology used in this document: 
 * A *workflow* contains multiple independently ran *tasks*. Each task runs a series of *steps*. Steps can call predefined *actions*, or run shell commands.

<details>
<summary><h3>I want to have a step output multilined / complex text</h3></summary>

```yml
- name: Calculate PR doc coverage
  id: prddoc
  run: |
    RUSTDOCFLAGS='-Z unstable-options --show-coverage' cargo +nightly doc --workspace --no-deps > coverage.md
    echo 'coverage<<EOFABC' >> $GITHUB_OUTPUT
    echo "$(cat coverage.md)" >> $GITHUB_OUTPUT
    echo 'EOFABC' >> $GITHUB_OUTPUT```
```

The entire output of `cargo doc` can be substituted into later jobs by using `${{ steps.prdoc.outputs.coverage }}`
</details> 
<details>
<summary><h3>workflow_run: I want a workflow that runs on a PR and can write to the repo</h3></summary>

PR branches and their workflows typically live in on a branch on an external fork. Therefore, they cannot write to the repository. The solution is to split things into two workflows - one that runs on the PR with read-only permissions, and one that runs on main and can write to the repository. This is called a `workflow_run` workflow. Read [the docs](https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#workflow_run).


**The workflow_run workflow should not run any user provided code as it has secrets in scope.**

</details> 
<details>
<summary><h3>I want to access the calling PR in a workflow_run workflow</h3></summary>

`workflow_run` jobs do not get access to the calling workflows detail. While one can access some things via the Github API such as head_sha, head_repo, this may not give any PR information. [Github](https://securitylab.github.com/research/github-actions-preventing-pwn-requests/) recommends saving the PR number to an artifact, and using this number to fetch the PR info through the API:

Example from [this github blog post](https://securitylab.github.com/research/github-actions-preventing-pwn-requests/) - see this for more explanation and details!

```yml
name: Receive PR

# read-only repo token
# no access to secrets
on:
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest

    steps:        
      - uses: actions/checkout@v2

      # imitation of a build process
      - name: Build
        run: /bin/bash ./build.sh

      - name: Save PR number
        run: |
          mkdir -p ./pr
          echo ${{ github.event.number }} > ./pr/NR
      - uses: actions/upload-artifact@v2
        with:
          name: pr
          path: pr/
```

```yml
name: Comment on the pull request

# read-write repo token
# access to secrets
on:
  workflow_run:
    workflows: ["Receive PR"]
    types:
      - completed

jobs:
  upload:
    runs-on: ubuntu-latest
    if: >
      github.event.workflow_run.event == 'pull_request' &&
      github.event.workflow_run.conclusion == 'success'
    steps:
      - name: 'Download artifact'
        uses: actions/github-script@v3.1.0
        with:
          script: |
            var artifacts = await github.actions.listWorkflowRunArtifacts({
               owner: context.repo.owner,
               repo: context.repo.repo,
               run_id: ${{github.event.workflow_run.id }},
            });
            var matchArtifact = artifacts.data.artifacts.filter((artifact) => {
              return artifact.name == "pr"
            })[0];
            var download = await github.actions.downloadArtifact({
               owner: context.repo.owner,
               repo: context.repo.repo,
               artifact_id: matchArtifact.id,
               archive_format: 'zip',
            });
            var fs = require('fs');
            fs.writeFileSync('${{github.workspace}}/pr.zip', Buffer.from(download.data));
      - run: unzip pr.zip

      - name: 'Comment on PR'
        uses: actions/github-script@v3
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          script: |
            var fs = require('fs');
            var issue_number = Number(fs.readFileSync('./NR'));
            await github.issues.createComment({
              owner: context.repo.owner,
              repo: context.repo.repo,
              issue_number: issue_number,
              body: 'Everything is OK. Thank you for the PR!'
            });

```
</details>
<details>
<summary><h3>How do I get x commit inside a PR workflow? What do all the different github sha's mean?</h3></summary>

**If you are running in a workflow_run workflow, you will need to get the calling PR first. See [I want to access the calling PR in a workflow_run workflow](https://github.com/conjure-cp/conjure-oxide/wiki/_new#i-want-to-access-the-calling-pr-in-a-workflow_run-workflow) instead.**


The default `github.sha` is a temporary commit representing the state of the repo should the PR be merged now. You probably want `github.event.pull_request.head.sha`. Read [The many SHAs of a github pull request](https://www.kenmuse.com/blog/the-many-shas-of-a-github-pull-request/).


</details>



