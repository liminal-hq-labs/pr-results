# Adopting pr-results

> **Not released.** pr-results has no release yet, so there is no binary to pin: the reusable workflow and the composite action stop with a message that says so until a release writes its SHA-256 into them, and `build: source` in the composite action builds the checkout instead. The command line, the action and the workflows are built and tested against an in-memory GitHub, but none of them has run on GitHub. The behaviour is specified in [SPEC.md](../SPEC.md).

The publisher runs after your CI finishes, in a `workflow_run` workflow, so it works for pull requests from forks and from Dependabot, where a `pull_request` job's token is read-only. Your CI only needs to upload the results as artefacts.

## 1. Upload the results from CI

In each job that produces results, upload them with `if: always()`, so a failing run still uploads. The default names below need no configuration file:

| Artefact | Holds |
| --- | --- |
| `rust-test-results` | nextest JUnit, `target/nextest/ci/junit.xml` |
| `frontend-test-results` or `js-test-results` | vitest or bun JUnit |
| `e2e-test-results-1`, `e2e-test-results-2` | Playwright JUnit, one artefact per shard, read as one suite |
| `steps-test-results` | step-results JUnit from a shell-step project |
| `event-file` | `${{ github.event_path }}`, a hint for finding the pull request (verified, never trusted on its own) |

Any artefact named `<name>-test-results` or `<name>-test-results-<more>` becomes a suite called "<Name> tests", with its check run named "<Title> <Name> tests". Names with anything but letters, digits, `.`, `_` and `-` are ignored, because a fork chooses its artefact names. Name your suites in a settings file (below) when you want something else.

```yaml
- name: Upload Rust test results
  if: always()
  uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
  with:
    name: rust-test-results
    path: target/nextest/ci/junit.xml
    if-no-files-found: ignore

- name: Upload the event file
  if: always()
  uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
  with:
    name: event-file
    path: ${{ github.event_path }}
```

Keep the CI workflow at `contents: read`. The recipes for each test runner are [below](#recipes-for-each-test-runner).

## 2. Add the publisher workflow

Create `.github/workflows/publish-test-results.yml`:

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
      actions: read # list and download the run's artefacts
      checks: write # create and update check runs and annotations
      pull-requests: write # create and update the sticky comment
      contents: read # check out .github/ from the default branch
    uses: liminal-hq-labs/pr-results/.github/workflows/publish.yml@<full commit sha> # v0.1.0
```

Pin the `uses:` line by full commit SHA and keep the version in the comment, so Dependabot's `github-actions` ecosystem can update both. Pinning the commit also pins the exact binary, because `publish.yml` at that commit carries its SHA-256 and checks it before running it. The workflow takes one input, `config` (default `.github/pr-results.toml`). It runs on `ubuntu-24.04`, checks out only `.github/` of your default branch with no credentials, and never your pull request.

The publisher is a `workflow_run` workflow, so GitHub only runs it once the file is on the default branch. Merge it before expecting comments on other branches.

### The composite action

The same binary is available as a step, for a custom flow or for `render`:

```yaml
- name: Render the results
  uses: liminal-hq-labs/pr-results@<full commit sha> # v0.1.0
  with:
    command: render
    args: --out out target/nextest/ci/junit.xml
```

The inputs are `command` (`publish`, `render` or `validate-config`), `args` (split on whitespace), `config`, `build` (`release`, the default, or `source`, which builds the action's checkout with a pinned toolchain) and `token` (default `github.token`). `publish` works only in a `workflow_run` workflow on the default branch, and stops with a message anywhere else. Publishing from the test job itself is planned for v0.2.

On GitHub Enterprise Server the publisher reads artefacts only from the API host itself, and check runs are matched to the `github-actions` app. A server whose artefact storage has other host names is refused until a setting for them exists; there is none in v0.1.

## 3. Configure it (optional)

With the default artefact names, no file is needed: suites are inferred from artefacts matching `*-test-results*`. To name check runs, map suites or set limits, add `.github/pr-results.toml` on the default branch. Unknown keys are an error, and so is a setting for a later version. Every setting and its default is in [SPEC.md](../SPEC.md#settings-and-defaults); validate a file with `pr-results validate-config .github/pr-results.toml`.

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

This writes `out/comment.md` and `out/checks/0.md`, one file per suite. Without `--out` it prints the comment. With a settings file (`--config`), each file goes to the first suite whose `files` patterns match its path. The footer links point at a placeholder repository, because there is no run.

To try the publisher itself, add `args: --dry-run out` to a publisher step in a branch. It reads from GitHub for real, writes nothing, and saves the would-be API requests in `out/requests.json`, the comment in `out/comment.md` and each check run in `out/checks/`.

## Recipes for each test runner

Each recipe is the upload step to add to your CI and, where the defaults do not fit, the suite for `.github/pr-results.toml`. The publisher is the same for all of them. The JavaScript flavours (vitest, bun, jest, Playwright) are parsed in v0.1 and tuned in v0.2: results, failures and times show, and annotations at the file and line, path mapping and Playwright's retries follow in v0.2.

### Rust: nextest

Set a `ci` profile that writes JUnit in `.config/nextest.toml`, then run it and upload the file:

```toml nextest
# nextest settings: the `ci` profile writes JUnit that pr-results reads.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

[profile.ci]
retries = 1

[profile.ci.junit]
path = "junit.xml"
```

```yaml
- name: Run the tests
  run: cargo nextest run --workspace --locked --profile ci

- name: Upload Rust test results
  if: always()
  uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
  with:
    name: rust-test-results
    path: target/nextest/ci/junit.xml
    if-no-files-found: ignore
```

`retries = 1` makes a test that passes on its second try show as flaky instead of failing. The artefact name is a default, so no settings file is needed.

### JavaScript: vitest

```yaml
- name: Run the tests
  run: npx vitest run --reporter=default --reporter=junit --outputFile.junit=test-results/junit.xml

- name: Upload frontend test results
  if: always()
  uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
  with:
    name: frontend-test-results
    path: test-results/junit.xml
    if-no-files-found: ignore
```

### JavaScript: Playwright shards

Each shard uploads its own artefact under a name that starts with the suite's, and the publisher reads them as one suite:

```yaml
# Runs the end-to-end tests in two shards.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

name: CI

on:
  pull_request:

permissions:
  contents: read

jobs:
  e2e:
    runs-on: ubuntu-24.04
    strategy:
      fail-fast: false
      matrix:
        shard: [1, 2]
    steps:
      - name: Checkout code
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          persist-credentials: false

      - name: Run the tests
        env:
          PLAYWRIGHT_JUNIT_OUTPUT_FILE: results/junit.xml
        run: |
          npm ci
          npx playwright install --with-deps
          npx playwright test --shard=${{ matrix.shard }}/2 --reporter=list,junit

      - name: Upload end-to-end test results
        if: always()
        uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
        with:
          name: e2e-test-results-${{ matrix.shard }}
          path: results/junit.xml
          if-no-files-found: ignore
```

The artefacts `e2e-test-results-1` and `e2e-test-results-2` become one suite, "E2e tests". To name it and give it the Playwright flavour, add to `.github/pr-results.toml`:

```toml
# pr-results settings: the end-to-end shards as one suite.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1
title = "Example checks"

[[suite]]
name = "End-to-end tests"
check = "Example end-to-end tests"
artifact = "e2e-test-results-*"
files = ["**/junit.xml"]
flavour = "playwright"
```

### JavaScript: bun

```yaml
- name: Run the tests
  run: bun test --reporter=junit --reporter-outfile=test-results/junit.xml

- name: Upload JavaScript test results
  if: always()
  uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
  with:
    name: js-test-results
    path: test-results/junit.xml
    if-no-files-found: ignore
```

### Shell steps and Rust together: Desk

Desk runs its checks as groups of steps and uploads one artefact per job, `results-<job>`, holding the converter's `steps.xml` and nextest's `nextest-*.xml`. The default `*-test-results*` misses those names, so Desk needs a settings file with two suites that each take their own files from every `results-*` artefact:

```yaml
- name: Upload the results
  if: always()
  uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
  with:
    name: results-${{ github.job }}
    path: |
      results/steps.xml
      results/nextest-*.xml
    if-no-files-found: ignore
```

```toml
# pr-results settings for Desk: the step results and the Rust tests, from every results-* artefact.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1
title = "Desk checks"

[[suite]]
name = "Desk steps"
check = "Desk check steps"
artifact = "results-*"
files = ["**/steps.xml"]
flavour = "steps"
required = true

[[suite]]
name = "Rust tests"
check = "Desk Rust tests"
artifact = "results-*"
files = ["**/nextest-*.xml"]
flavour = "nextest"
```

`required = true` makes a run with no step results a failure ("No results were uploaded") where an optional suite is neutral. The `steps` flavour reads one suite per group or one per job, the shape Desk's converter writes (see [docs/inputs.md](inputs.md)). Desk adopts it after this repository's own dogfood, in a small pull request that swaps its current reporter steps for the publisher workflow above.

## What the publisher does with a run

It lists the run's artefacts, downloads the ones your suites name (an expired artefact, one over the download limit and one it will not read are skipped with a line in the log), and writes a check run per suite on the commit. Then it finds the pull request and writes the comment.

- **Finding the pull request.** It never takes the number from the uploaded `event-file` on its own. It re-reads the pull requests that GitHub names for the run, or lists the commit's pull requests, or, for a fork, the open pull requests from that fork's branch, and keeps those that are open, at this run's commit, from this run's repository and into yours. When more than one verifies, the number in `event.json` chooses among them; if it chooses none, the comment goes on each, up to five, and the log says so.
- **A push gets check runs and no comment.** A run that was cancelled or skipped gets nothing.
- **One comment, kept in place.** The comment is found by its hidden first line and by its author, `github-actions[bot]`, so a user's comment that copies the marker is ignored. A re-run updates it, a late run for the same commit never overwrites a later one, and a second comment left by two runs at once is deleted.
- **When something is refused.** The check runs are written first. If GitHub refuses the comment (a read-only token, a locked conversation), the checks stay posted and the publisher exits non-zero so the failure is visible. A result too large for GitHub is written again in a shorter form and the log says so. An annotation on a file that is not in the commit is left out, and a batch GitHub refuses is left out and counted, not retried into a failure.
- **Where artefacts come from.** The download follows GitHub's redirect only to GitHub's own storage (`*.blob.core.windows.net` and `*.githubusercontent.com`). On GitHub Enterprise Server the publisher accepts the server's own host, and the storage hosts of your installation have to be named in its configuration, or the download is refused.
- **What the log shows.** Counts and fixed phrases, and the title of a section; never a test name, a message or a header.

## What the comment looks like

Layout rules: a heading with the product's title and a one-phrase outcome, one table row per suite, failures first, everything long collapsed, no emoji. This is the Markdown the comment carries for a product with one failing step and a flaky Rust test (a hand-made example in the shape of the outputs in `tests/golden/`; the hash and run id are placeholders):

`````markdown
<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### Example checks: 1 failure in 2 suites

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Desk steps` | **Failed** | 23 | 1 | 4 | 0 | 6 m 12 s |
| `Rust tests` | Passed | 672 | 0 | 0 | 1 | 47.0 s |

#### Failures

<details open>
<summary>Failure 1 of 1</summary>

`Desk steps` › `check (dos)` › `Emulator` › `Example binary in a headless emulator`

`exit status 1` · 18.2 s

Last 3 of 31 lines:

````text
emulator: program exited with code 3
expected frame 12 to match dist/frames/0012.png
(a triple backtick in the log does not end this block)
````

</details>

<details>
<summary>Flaky: passed after a retry (1)</summary>

- `Rust tests` › `core` › `blit::clips_at_the_right_edge`: failed once, passed on attempt 2

</details>

Commit [`3f9c2e1`](https://github.com/example/app/commit/3f9c2e1d4b5a69788796a5b4c3d2e1f001122334) · [workflow run](https://github.com/example/app/actions/runs/19283746) · [artefacts](https://github.com/example/app/actions/runs/19283746#artifacts) · pr-results 0.1.0
`````

Two rules shape it. A raw HTML line (`<details>`, `<summary>`, `</details>`) holds only fixed words and numbers, because Markdown inside it is not rendered, so every name sits on a Markdown line of its own, in a code span. And the names are the suite's title, the test suite, its group and the test, with the section's title in front only when the comment has more than one section. A column comparing each suite with the base branch arrives with baseline comparison in v0.3.

When the comment is too long for GitHub it gives things up in a fixed order and says what it left out in a line of its own: the flaky list becomes a count, failure excerpts shrink to 8 and then 3 lines, failures are listed up to 20, then 5, then not at all, passing suites become one row, and last of all only the heading, the totals and the footer remain.

When everything passes, the comment shrinks to the heading, the table and the footer. When no results were uploaded it says so and links the run. When the CI run was cancelled or skipped, nothing is posted.

## Migrating from another reporter

- **Reporters that run in the test job** (a unit-test reporter action, a script that posts a comment) cannot post on fork or Dependabot pull requests. Move the upload to artefacts and use the publisher, then delete the in-job reporter steps.
- **Several reporter steps in one publisher workflow**, one per suite, post one comment each. The publisher replaces them all: delete them and keep one comment.
- **A script that builds its own comment** can be retired once `other_checks = true` (v0.3) lists the commit's other check runs in the same comment.
- **Check names.** Default check names are `<title> <suite name>`, so they do not collide with job names. The job summary warns when a configured name equals a job name.
