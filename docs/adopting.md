# Adopting pr-results

> **Experimental.** v0.1.0 is the first release. The publisher runs on Linux, `x86_64` and `aarch64`, as a static musl binary. On other platforms build from source with `cargo install --git https://github.com/liminal-hq-labs/pr-results pr-results --locked`. macOS and Windows binaries are not shipped in v0.1, and Windows path handling in the zip reader and the file checks is untested. The command line, the action and the workflows are tested against an in-memory GitHub, and none has yet run on GitHub Actions. The behaviour is specified in [SPEC.md](../SPEC.md).

The publisher runs after your CI finishes, in a `workflow_run` workflow, so it works for pull requests from forks and from Dependabot, where a `pull_request` job's token is read-only. Your CI only needs to upload the results as artefacts.

## 1. Upload the results from CI

In each job that produces results, upload them with `if: always()`, so a failing run still uploads. The default names below need no configuration file:

| Artefact | Holds |
| --- | --- |
| `rust-test-results` | nextest JUnit, `target/nextest/ci/junit.xml` |
| `frontend-test-results` or `js-test-results` | vitest or bun JUnit |
| `e2e-test-results-1`, `e2e-test-results-2` | Playwright JUnit, one artefact per shard, read as one suite |
| `steps-test-results` | step-results JUnit from a shell-step project |
| `event-file` | `${{ github.event_path }}`, a tie-breaker only: its pull request number chooses among pull requests that GitHub's own data already matched to the run, and is ignored otherwise |

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

### Comment styles

The comment comes in three styles, set by `comment.style`. **`guided`** is the default: a summary in plain sentences, a list of what needs attention, a command to run a failed nextest test again, and the slowest tests. **`plain`** is the table, the failures and the footer, as the tool wrote them before the styles. **`enhanced`** is guided with emoji, a line from Jax, a progress bar and GitHub alert blocks, which suits a Liminal HQ project that wants them. Each extra is also a setting of its own, so a project can take one without the others:

```toml
# pr-results settings: the enhanced style, without Jax.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1

[comment]
style = "enhanced"  # plain | guided | enhanced
mascot = false      # any of mascot, progress and alerts can be switched off, or on in guided
symbols = "glyphs"  # text | glyphs | emoji: ✓ ✗ ↻ – in place of the emoji
```

`mascot`, `progress` and `alerts` need the guided or the enhanced style: a file that turns one on in the plain style is refused. The progress bar writes its words first (`737 of 738 tests passed`) and the shapes after, so a screen reader reads the sentence before it reads the bar; set `symbols = "text"` for words with no marks at all. Alert blocks are written for GitHub only. Nothing from a report is put in an alert, in Jax's line or in the summary sentence: they hold fixed sentences and numbers, and the names are in the list under them.

The command to run a failed test again is written for nextest suites, from the suite's binary and test name, and only when both are made of letters, digits and the few separators a Rust path has: a name that is not gets no command. It starts with `cargo nextest run`. A project that runs its tests under a feature set says so in the suite, and an empty string turns the command off:

```toml
# pr-results settings: a command that matches how the job ran the tests.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1

[[suite]]
name = "Tests (all-features)"
artifact = "rust-test-results-all-features"
flavour = "nextest"
run_locally = "cargo nextest run --all-features"
```

The command makes the shape of what you paste safe. It does not make the code safe: the test is code from the pull request, so run it as you would anything from a pull request.

### Other checks in the comment

A project whose CI is several jobs (format, clippy, audit) alongside the tests can have the comment say how each one ended, so it stands in for a script that posts such a summary:

```toml
# pr-results settings: the other jobs of the commit, in a table under the suites.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1

[comment]
other_checks = true
```

When every one of the other checks passed, the guided and enhanced styles fold the table into one line, `Other checks: 5 passed`, since there is nothing to act on. A project whose comment stands in for a script that gave every job a line can keep the table open with `other_checks_open = true` under `[comment]`. It makes no difference when a check is failing, because the table is open then.

When the test jobs and the suites show the same thing, list the checks once. A project that runs its tests in a job per feature set has a suite and a check for each, and a failing test fails both, so the table would count the same problem twice. Tell each suite which job it comes from with `job`, and that job's row is left out. The row is left out only while the job and the suite agree: a job that failed and uploaded no results, or passed while its tests failed, still gets its row, since those are what the table is for. Give the checks that stay a command, which the guided style shows beside one that failed:

```toml
# pr-results settings: the other jobs, without the ones the suites already show.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1

[comment]
other_checks = true

[comment.check_commands]
"Security audit" = "cargo audit"
"Format" = "cargo fmt --all -- --check"

[[suite]]
name = "Tests (default)"
artifact = "rust-test-results-default"
flavour = "nextest"
job = "Test (default)"

[[suite]]
name = "Tests (all-features)"
artifact = "rust-test-results-all-features"
flavour = "nextest"
job = "Test (all-features)"
```

The job is the name GitHub shows for it, matrix legs included (`Test (default)`), and is matched exactly. A job that is renamed stops matching and shows up in the table again. `comment.other_checks_ignore` still leaves checks out by pattern, for a job that no suite stands in for, but it leaves them out whatever their result. The patterns of `other_checks_ignore` are the language of the file patterns above: `*` matches any run of characters but `/`, so a name such as `build / test` needs `**`. A check that does not exist is not an error, so a pattern can stay when a job is renamed, and the cost is that a job that failed under a name nothing matches any more is shown again. In the guided style a table of other checks that all passed is folded into one line, `Other checks: 5 passed`, since there is nothing to act on.

It needs nothing new in the publisher workflow: `checks: write` covers the read. The table lists the commit's other check runs, failed ones first, and leaves out the suites' own check runs and the publisher's own job. A failed check is also said in the heading, for example `All 696 passed, 1 other check needs attention`. It works in the `workflow_run` publisher on GitHub only: on Forgejo, and when publishing from the job itself (`--from`), it has no effect and the notes in the job summary say so.

### Tests added and removed

With the guided or enhanced style the comment can say how many tests the pull request adds and removes, compared with the base branch:

> This pull request adds 3 tests and removes 2 against `main`. A renamed test counts as both.

It is on by default, and says nothing when nothing differs or when there is nothing to compare with.

It needs the base branch's own CI to have run the same workflow on a push and uploaded the same artefact names as the pull request's run, so a repository that already runs its CI on `main` gets it with no change. The publisher takes the run on the pull request's base commit, else the newest on its base branch, else the newest on `baseline.branch`. A pull request stacked on another branch with no run therefore compares against the default branch, and the sentence says so:

> This pull request adds 3 tests against `main`, because `feature/base` had no results to compare with.

A failed base run is fine, since it still ran its tests. A run from a fork, a pull request's own run, a cancelled run and an expired artefact are never used, and in each case the comment simply has no sentence (the job summary says why).

Only JUnit suites that are not `steps` are compared, and only the suites both runs have: a suite the base run did not upload is left out on both sides, so a new suite is not reported as hundreds of added tests. A test is its suite's name and project, its class name and its name, so renaming a suite's title in the settings changes nothing, and a renamed test counts as one removed and one added.

```toml
# pr-results settings: the added and removed tests line.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1

[comment]
test_delta = false   # turn the sentence off

[baseline]
branch = "main"      # last place to look; empty turns baselines off
```

`comment.test_delta = true` with an empty `baseline.branch` is refused, since the sentence could never appear. `baseline.workflow`, which earlier settings files may carry, is ignored: the workflow is the one that triggered the publisher. It uses the `actions: read` permission the publisher already has, and nothing more.

## 4. Try it without publishing

Render the Markdown from local files with no network, to see what a pull request would get:

```sh
pr-results render target/nextest/ci/junit.xml --out out/
```

This writes `out/comment.md` and `out/checks/0.md`, one file per suite. Without `--out` it prints the comment. With a settings file (`--config`), each file goes to the first suite whose `files` patterns match its path. The footer links point at a placeholder repository, because there is no run.

To try the publisher itself, add `args: --dry-run out` to a publisher step in a branch. It reads from GitHub for real, writes nothing, and saves the would-be API requests in `out/requests.json`, the comment in `out/comment.md` and each check run in `out/checks/`.

## Recipes for each test runner

Each recipe is the upload step to add to your CI and, where the defaults do not fit, the suite for `.github/pr-results.toml`. The publisher is the same for all of them. The JavaScript flavours (vitest, bun, jest, Playwright) were tuned against reports the reporters wrote ([docs/inputs.md](inputs.md#what-the-reporters-write)). Their paths are relative to where the tests ran, so a project below the repository root sets `path_prefix` on its suite (the directory, such as `apps/web/`), and an annotation lands on the right file. vitest, bun, jest-junit and Playwright do not write a test that failed and then passed as anything but a pass, so a retry never shows as flaky for them; only nextest does.

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

A monorepo package gets its own suite and the directory its paths are relative to:

```toml
# pr-results settings: a vitest package below the repository root.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1
title = "Example checks"

[[suite]]
name = "Web tests"
check = "Example web tests"
artifact = "frontend-test-results"
files = ["**/junit.xml"]
flavour = "vitest"
path_prefix = "apps/web/"
```

For Playwright, `path_prefix` is the repository path of `testDir`. A suite whose spec runs in several projects shows one line for each project, the project in brackets after the spec.

### JavaScript: jest

Add jest-junit with `addFileAttribute`, which gives each case its file (jest writes absolute paths in its stacks, and the file is what lets the line be used):

```js
// jest settings: write JUnit with the file of each test.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

module.exports = {
	reporters: ['default', ['jest-junit', { outputDirectory: 'test-results', outputName: 'junit.xml', addFileAttribute: 'true' }]],
};
```

```yaml
- name: Run the tests
  run: npx jest --ci

- name: Upload JavaScript test results
  if: always()
  uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
  with:
    name: js-test-results
    path: test-results/junit.xml
    if-no-files-found: ignore
```

If the `suiteName` option is set, add `flavour = "jest"` to the suite: the file no longer says which reporter wrote it.

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

### Static-analysis results: SARIF

A scanner that writes SARIF needs no wrapper and no converter. Upload the file and read it as a suite of its own. A finding becomes a failure at the line it names, in the comment and as an annotation in the diff:

```yaml
- name: Scan
  run: semgrep --config auto --sarif --output results/semgrep.sarif

- name: Upload the scan
  if: always()
  uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
  with:
    name: semgrep-test-results
    path: results/semgrep.sarif
    if-no-files-found: ignore
```

```toml
# pr-results settings: a scanner's SARIF as a suite.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1

[[suite]]
name = "Semgrep"
artifact = "semgrep-test-results"
format = "sarif"
fail_on = "warning"  # note | warning | error: findings below it are counted as skipped
job = "Scan"
```

`fail_on` decides which findings fail the check: at or above it, a finding fails; below it, it is skipped and does not turn the job red. A scan with no findings shows as one passing case. Only a relative path inside the repository gets an annotation, so set `path_prefix` when the tool reports paths from a subdirectory. Findings are counted as tests in the comment's sentences and table.

### Clippy and the audit, with nothing else installed

A Rust project can send both to the comment from what cargo writes. The lint reports each diagnostic at its line, and the audit reports each advisory at `Cargo.lock`:

```yaml
- name: Clippy
  run: |
    mkdir -p results
    cargo clippy --workspace --all-targets --locked --message-format=json -- -D warnings > results/clippy.json

- name: Audit
  run: |
    mkdir -p results
    cargo audit --json > results/audit.json

- name: Upload the results
  if: always()
  uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
  with:
    name: lint-test-results
    path: results/
    if-no-files-found: ignore
```

```toml
# pr-results settings: clippy and the audit, each a suite of its own.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1

[[suite]]
name = "Clippy"
artifact = "lint-test-results"
files = ["**/clippy.json"]
format = "clippy-json"
job = "Clippy"

[[suite]]
name = "Audit"
artifact = "lint-test-results"
files = ["**/audit.json"]
format = "cargo-audit-json"
fail_on = "error"  # cargo audit itself fails only on vulnerabilities
job = "Security audit"
```

Both files go to one artefact and each suite takes its own by name. The step still fails the job when the tool fails, since the redirect does not hide the exit status, and `if: always()` uploads the file either way. `--message-format=json` puts JSON on standard output in place of the readable text, so the job's log loses it: run the lint again without the flag (it is cached), or use `pr-results wrap` below when the last lines of the log are enough.

### Any command: wrap it

A job that runs a formatter, a linter or an audit already knows whether it passed and what it printed. `pr-results wrap` runs the command once, exactly as before, and writes down how it ended, so the comment shows why it failed and nobody has to open the job. Use the action, and put the command after `--`:

```yaml
- name: Clippy
  uses: liminal-hq-labs/pr-results@<full commit sha> # a release that has wrap
  with:
    command: wrap
    args: --name Clippy --out results/clippy.xml -- cargo clippy --workspace --all-targets --locked -- -D warnings

- name: Upload the clippy result
  if: always()
  uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
  with:
    name: clippy-test-results
    path: results/clippy.xml
    if-no-files-found: ignore
```

The step still fails when the command does: `wrap` exits with the command's own status (`127` if it was not found, `128` plus the signal if it was killed), and the output still goes to the job's log, so a workflow command it writes keeps working. The arguments are split on whitespace, so a quoted argument with a space in it needs the binary instead. A suite reads what it wrote as the `steps` flavour, and `job` ties it to the job so the table of other checks does not list it twice:

```toml
# pr-results settings: a wrapped command as a suite of its own.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1

[[suite]]
name = "Clippy"
artifact = "clippy-test-results"
flavour = "steps"
required = true
job = "Clippy"
```

When it fails, the comment shows the step with the first and last lines of what the command printed (`--head` 5 and `--tail` 40 by default). `required = true` makes a job that died before it uploaded a failure, not a missing row. Clippy prints its first error early and ends with only "could not compile", so `--message-format=short` keeps each diagnostic on one line and fits the tail better.

**Wrap only commands whose output is safe to publish.** GitHub masks a secret in a log, not in a file, so the kept output is written only when the command fails, never when it passes, and the step's title is the program and its first word (`cargo clippy`), not the rest of the command line. Formatting, linting and auditing are safe; a deploy or a login is not. Use `--step` to name a step yourself and `--no-output` to keep only the status and the time.

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

Layout rules, for every style: a heading with the product's title and a one-phrase outcome, one table row per suite, failures first, everything long collapsed. There are no emoji unless the settings ask for them. This is the Markdown of the **plain** style for a product with one failing step and a flaky Rust test (a hand-made example in the shape of the outputs in `tests/golden/`; the hash and run id are placeholders):

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

Two rules shape it. A raw HTML line (`<details>`, `<summary>`, `</details>`) holds only fixed words and numbers, because Markdown inside it is not rendered, so every name sits on a Markdown line of its own, in a code span. And the names are the suite's title, the test suite, its group and the test, with the section's title in front only when the comment has more than one section. A column comparing each suite with the base branch is not built; the only use of the base branch's results is the sentence about added and removed tests, described under [Tests added and removed](#tests-added-and-removed).

The **guided** style, the default, writes the same run like this. The Desk steps are not nextest tests, so a failed step has no command; a failed nextest test would have a `Run it locally:` block under its excerpt:

`````markdown
<!-- pr-results:1 key=ci sha=3f9c2e1d4b5a69788796a5b4c3d2e1f001122334 run=19283746 -->
### Example checks: 1 failure in 2 suites

1 failed test needs attention. 696 tests passed, 1 of them only after a retry.

- ✗ 1 test failed in `Desk steps`.
- ↻ 1 test in `Rust tests` passed only after a retry, so it may be flaky.

| Suite | Result | Passed | Failed | Skipped | Flaky | Time |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `Desk steps` | ✗ **Failed** | 23 | 1 | 4 | 0 | 6 m 12 s |
| `Rust tests` | ✓ Passed | 672 | 0 | 0 | 1 | 47.0 s |

#### Failures

(the failure and the flaky list are as in the plain style)

<details>
<summary>Slowest tests (5)</summary>

| Test | Time |
| --- | ---: |
| `Desk steps` › `check (dos)` › `Emulator` › `Example binary in a headless emulator` | 18.2 s |
| … | … |

</details>

This comment is updated in place with the latest results.
`````

The top of the **enhanced** style for a run where everything passed, with Jax, an alert block and the bar:

`````markdown
### ✅ Example checks: All 3 passed, 1 skipped

> [!TIP]
> 🦦 **Jax:** All clear. Every test passed.
>
> Everything passed: 3 tests (1 skipped) in 1 suite and 5 other checks, in 0.3 s of test time.

3 of 3 tests passed ▰▰▰▰▰▰▰▰▰▰
`````

When the comment is too long for GitHub it gives things up in a fixed order and says what it left out in a line of its own: the passing other checks become one row, the flaky list becomes a count and the slowest tests are left out, failure excerpts shrink to 8 and then 3 lines, failures are listed up to 20, then 5, then not at all, passing suites become one row, and last of all only the heading, the totals and the footer remain.

When everything passes, the plain comment shrinks to the heading, the table and the footer, and the guided one adds its summary sentence. When no results were uploaded it says so and links the run, and the guided style also says what to look at. When the CI run was cancelled or skipped, nothing is posted.

## Migrating from another reporter

- **Reporters that run in the test job** (a unit-test reporter action, a script that posts a comment) cannot post on fork or Dependabot pull requests. Move the upload to artefacts and use the publisher, then delete the in-job reporter steps.
- **Several reporter steps in one publisher workflow**, one per suite, post one comment each. The publisher replaces them all: delete them and keep one comment.
- **A script that builds its own comment**, such as a summary of which jobs passed, can be retired once `comment.other_checks = true` lists the commit's other check runs in the same comment ([below](#other-checks-in-the-comment)). The publisher reads them once, when it runs, so a check from a slower workflow that is still going is shown as Running and is not updated later.
- **Check names.** Default check names are `<title> <suite name>`, so they do not collide with job names. The job summary warns when a configured name equals a job name.

## Publishing in the same run

For a same-repository pull request or push, download each job's artefacts into a directory with one subdirectory per artefact, then run `pr-results publish --from results`. The publisher reads the event supplied by the runner, verifies the pull request with the host and skips fork pull requests without a request. It refuses `pull_request_target`; use the separate `workflow_run` publisher above for forks on GitHub. The job's checkout supplies the settings, so use this mode only when the job's code and token are trusted. The collector requires a Unix runner and skips symbolic links.
