# Adopting pr-results

> **Planned.** `pr-results` is not released yet: there is no action, reusable workflow or binary to pin. This page describes the intended setup so the CI of a product can be prepared now. The behaviour is specified in [SPEC.md](../SPEC.md).

The publisher runs after your CI finishes, in a `workflow_run` workflow, so it works for pull requests from forks and from Dependabot, where a `pull_request` job's token is read-only. Your CI only needs to upload the results as artefacts.

## 1. Upload the results from CI

In each job that produces results, upload them with `if: always()`, so a failing run still uploads. The default names below need no configuration file:

| Artefact | Holds |
| --- | --- |
| `rust-test-results` | nextest JUnit, `target/nextest/ci/junit.xml` |
| `frontend-test-results` or `js-test-results` | vitest or bun JUnit |
| `steps-test-results` | step-results JUnit from a shell-step project |
| `event-file` | `${{ github.event_path }}`, a hint for finding the pull request (verified, never trusted on its own) |

```yaml
- name: Upload Rust test results
  if: always()
  uses: actions/upload-artifact@<full commit sha> # vX.Y.Z
  with:
    name: rust-test-results
    path: target/nextest/ci/junit.xml
    if-no-files-found: ignore
```

Keep the CI workflow at `contents: read`.

## 2. Add the publisher workflow

Planned, not yet available. Create `.github/workflows/publish-test-results.yml`:

```yaml
# Posts the results of a finished CI run to its pull request: one comment and a check run per suite.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

name: Publish Test Results

on:
  workflow_run:
    workflows: [CI]
    types: [completed]

permissions: {}

jobs:
  publish:
    if: github.event.workflow_run.conclusion != 'skipped' && github.event.workflow_run.conclusion != 'cancelled'
    permissions:
      actions: read
      checks: write
      pull-requests: write
      contents: read
    uses: liminal-hq-labs/pr-results/.github/workflows/publish.yml@<full commit sha> # v0.1.0
```

Pin the `uses:` line by full commit SHA and keep the version in the comment, so Dependabot's `github-actions` ecosystem can update both. Pinning the commit also pins the exact binary, because `action.yml` at that commit carries its SHA-256. The workflow takes two inputs: `config` (default `.github/pr-results.toml`) and `build` (`release`, the default, or `source` for a runner the binaries do not cover).

The publisher is a `workflow_run` workflow, so GitHub only runs it once the file is on the default branch. Merge it before expecting comments on other branches.

## 3. Configure it (optional)

With the default artefact names, no file is needed: suites are inferred from artefacts matching `*-test-results*`. To name check runs, map suites or set limits, add `.github/pr-results.toml` on the default branch. Unknown keys are an error. Every setting and its default is in [SPEC.md](../SPEC.md#settings-and-defaults); validate a file with `pr-results validate-config .github/pr-results.toml`.

```toml
# pr-results settings: one comment, a check run per suite.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1
title = "Example checks"

[[suite]]
name = "Rust tests"
check = "Example Rust tests"
artifact = "rust-test-results"
files = ["**/junit.xml"]
flavour = "nextest"
```

## 4. Try it without publishing

Render the Markdown from local files with no network, to see what a pull request would get:

```sh
pr-results render target/nextest/ci/junit.xml --out out/
```

`pr-results publish --dry-run DIR` runs the whole flow without writing and saves the would-be API requests in `DIR`.

## What the comment looks like

Layout rules: a heading with the product's title and a one-phrase outcome, one table row per suite, failures first, everything long collapsed, no emoji. This is the Markdown the comment would carry for a product with one failing step and a flaky Rust test (a mock; the hash and run id are placeholders):

`````markdown
<!-- pr-results:1 key=ci sha=3f9c2e1d… run=19283746 -->
### Example checks: 1 failure in 2 suites

| Suite | Result | Passed | Failed | Skipped | Flaky | Time | Against `main` |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `check.sh` steps | **Failed** | 23 | 1 | 4 | 0 | 6 m 12 s | 1 new failure · 41 s slower |
| Rust tests | Passed | 672 | 0 | 0 | 1 | 47 s | no change |

#### Failures

<details open><summary><code>dos</code> › Example binary in a headless emulator</summary>

exit status 1 · [`tools/check.sh`](https://github.com/example/app/blob/3f9c2e1d…/tools/check.sh) · 18.2 s

````text
emulator: program exited with code 3
expected frame 12 to match dist/frames/0012.png
```
(a triple backtick in the log does not end this block)
````

</details>

<details><summary>Flaky: passed after a retry (1)</summary>

- `core` › `blit::clips_at_the_right_edge`: failed once, passed on attempt 2

</details>

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d…) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · compared with `main` at [`a1b2c3d`](https://github.com/example/app/commit/a1b2c3d…) · pr-results 0.1.0
`````

When everything passes, the comment shrinks to the heading, the table and the footer. When no results were uploaded it says so and links the run. When the CI run was cancelled or skipped, nothing is posted.

## Migrating from another reporter

- **Reporters that run in the test job** (a unit-test reporter action, a script that posts a comment) cannot post on fork or Dependabot pull requests. Move the upload to artefacts and use the publisher, then delete the in-job reporter steps.
- **Several reporter steps in one publisher workflow**, one per suite, post one comment each. The publisher replaces them all: delete them and keep one comment.
- **A script that builds its own comment** can be retired once `other_checks = true` (v0.3) lists the commit's other check runs in the same comment.
- **Check names.** Default check names are `<title> <suite name>`, so they do not collide with job names. The job summary warns when a configured name equals a job name.
