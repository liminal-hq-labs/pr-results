# pr-results

<p align="center"><img src="assets/hero.svg" alt="pr-results — Calm test and check results for pull requests: one sticky comment and a check run per suite, from JUnit and step reports" width="100%"></p>

<p align="center">
  <a href="https://github.com/liminal-hq-labs/pr-results/actions/workflows/ci.yml"><img src="https://github.com/liminal-hq-labs/pr-results/actions/workflows/ci.yml/badge.svg" alt="CI status"></a>
  <img src="https://img.shields.io/badge/status-experimental-a3e635" alt="Status: experimental">
  <img src="https://img.shields.io/badge/licence-Apache--2.0%20OR%20MIT-3fb950" alt="Licence: Apache-2.0 OR MIT">
</p>

pr-results reads the test and check results your CI produces and posts them to the pull request: one sticky comment for the whole run and one check run per suite, with annotations at the failing file and line. It reads JUnit from nextest (tuned) and step reports from shell-script projects (tuned), and from vitest, bun, jest and Playwright (tuned against real output), and it is safe on pull requests from forks by design.

> **Status:** experimental. v0.2.0 adds in-job publishing and Forgejo support, and tunes the JavaScript JUnit readers against real reports. GitHub Actions and a disposable Forgejo runner have exercised publishing; real team mentions and notifications remain unverified. Interfaces and formats may change between minor versions; see [CHANGELOG.md](CHANGELOG.md).

## What it's for

- A pull request that runs several suites and should get one calm comment, not one bot comment per suite.
- Pull requests from forks and Dependabot, where a test job's read-only token cannot post results, and the publisher runs afterwards through `workflow_run` instead.
- A project that runs shell steps, where a red job should name the step that broke and how long each step took.
- A reviewer who wants the failing file and line annotated in the diff, and flaky tests kept apart from real failures.
- An owner who wants every string from a test report escaped by one audited path, with the publisher pinned to a commit.

## What the comment will look like

This is a **mock** of the comment, written by hand in the shape of the renderer's output to show the layout. The example illustrates the layout; the publisher has also run on real pull requests.

> ### Example checks: 1 failure in 2 suites
>
> | Suite | Result | Passed | Failed | Skipped | Flaky | Time |
> | --- | --- | ---: | ---: | ---: | ---: | ---: |
> | `Desk steps` | **Failed** | 23 | 1 | 4 | 0 | 6 m 12 s |
> | `Rust tests` | Passed | 672 | 0 | 0 | 1 | 47.0 s |
>
> #### Failures
>
> Failure 1 of 1
>
> `Desk steps` › `check (dos)` › `Emulator` › `Example binary in a headless emulator`
>
> `exit status 1` · 18.2 s
>
> Flaky: passed after a retry (1)
>
> - `Rust tests` › `core` › `blit::clips_at_the_right_edge`: failed once, passed on attempt 2

In the real comment each failure is a `<details open>` block and the flaky list is a collapsed `<details>`.

The full layout is in [docs/adopting.md](docs/adopting.md#what-the-comment-looks-like).

## Install and run

On **GitHub**, the publisher runs in a `workflow_run` workflow after CI finishes, so it works for fork and Dependabot pull requests. Look the release SHA up with `gh api repos/liminal-hq-labs/pr-results/git/ref/tags/v0.2.0` (an annotated tag needs one more step, to the commit it points to), and read [docs/releasing.md](docs/releasing.md) for how to check the binary's attestation. The publisher runs on Linux, `x86_64` and `aarch64`, as a static musl binary. On other platforms build from source with `cargo install --git https://github.com/liminal-hq-labs/pr-results pr-results --locked`. macOS and Windows binaries are not shipped in v0.2, and Windows path handling in the zip reader and the file checks is untested.

The workflow:

```yaml
# .github/workflows/publish-test-results.yml
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
    uses: liminal-hq-labs/pr-results/.github/workflows/publish.yml@<full commit sha> # v0.2.0
```

On **Forgejo**, which has no `workflow_run` trigger, a final job downloads earlier jobs' artefacts and runs `pr-results publish --from results --forge forgejo` with the job token. It writes one commit status per suite in place of check runs. See [docs/adopting-forgejo.md](docs/adopting-forgejo.md) for the full setup.

Your CI uploads its results as artefacts (`rust-test-results`, `frontend-test-results`, `event-file` and so on), and the publisher does the rest. See [docs/adopting.md](docs/adopting.md) for the steps, a recipe for each test runner (nextest, vitest, Playwright shards, bun and Desk) and the optional `.github/pr-results.toml`.

## Documentation

- [SPEC.md](SPEC.md) — behaviour and interfaces
- [docs/adopting.md](docs/adopting.md) — setting it up on GitHub
- [docs/adopting-forgejo.md](docs/adopting-forgejo.md) — setting it up on Forgejo
- [docs/inputs.md](docs/inputs.md) — the formats it reads
- [docs/security.md](docs/security.md) — how it treats untrusted input
- [docs/releasing.md](docs/releasing.md) — cutting and verifying a release
- [docs/decisions.md](docs/decisions.md) — decisions and why
- [CHANGELOG.md](CHANGELOG.md) — what changed
- [AGENTS.md](AGENTS.md) — contributor and agent rules

## Contributing

Issues and pull requests are welcome. The house rules are in [AGENTS.md](AGENTS.md); run the checks it lists before opening a pull request.

## Part of Liminal HQ Labs

pr-results is a [Liminal HQ Labs](https://github.com/liminal-hq-labs) experiment. Experiments from Liminal HQ — early, rough and open. If it proves itself, it [graduates](https://github.com/liminal-hq-labs/.github/blob/main/docs/labs-charter.md#graduation) to [Liminal HQ](https://github.com/liminal-hq).

## Licence

Licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or the [MIT licence](LICENSE-MIT), at your option.
