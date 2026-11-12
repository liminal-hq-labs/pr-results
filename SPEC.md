# pr-results specification

The source of truth for behaviour. A change that alters behaviour updates this file in the same change. Why each choice was made is in [docs/decisions.md](docs/decisions.md); the per-format input notes are in [docs/inputs.md](docs/inputs.md), the threat model in [docs/security.md](docs/security.md) and the adoption guide in [docs/adopting.md](docs/adopting.md).

The data model, `Safe` and the JUnit parsers exist (see the notes in [Components](#components) and [the normalized model](#the-normalized-model)); everything else described here is not built yet. Where a decision is still the owner's, the text says **Open question** and names its number in the [list at the end](#open-questions).

## Contents

- [Purpose](#purpose)
- [Users and scenarios](#users-and-scenarios)
- [Non-goals](#non-goals)
- [Requirements](#requirements)
- [Components](#components)
- [Inputs](#inputs)
- [Outputs](#outputs)
- [Settings and defaults](#settings-and-defaults)
- [The workflow_run flow and security](#the-workflow_run-flow-and-security)
- [Adoption](#adoption)
- [Versioning, pinning, CI and release](#versioning-pinning-ci-and-release)
- [Test plan](#test-plan)
- [Roadmap](#roadmap)
- [Open questions](#open-questions)
- [What is not verified](#what-is-not-verified)

## Purpose

Liminal HQ products run their tests in several languages and report them in several inconsistent ways. A pull request may get four bot comments or none, forks and Dependabot get failed reporting steps, and a project that runs shell steps shows only one red or green job. Existing reporters each cover part of the need (see [docs/decisions.md](docs/decisions.md#what-the-existing-options-do)), and a home-grown poster that builds Markdown from test names is one escaping mistake away from injecting content into a comment.

`pr-results` is one tool that reads every format the products produce, posts one calm sticky comment per pull request and one check run per suite, and runs safely on untrusted fork input through a `workflow_run` publisher. A small file in each repository configures it, and it is built in Rust and pinned by commit SHA, the way Liminal HQ builds its own tooling.

## Users and scenarios

- **A maintainer of a Rust and TypeScript product** wants one comment on each pull request with the nextest and vitest results, failures first, instead of one comment per suite.
- **A contributor opening a pull request from a fork** gets the same comment and checks, without the maintainer loosening any permission.
- **A project that runs shell steps** (Desk's `tools/check.sh`) wants each group of steps to appear as a suite and each step as a case, so a red job names the step that broke.
- **A reviewer** wants the failing file and line annotated in the diff, and a flaky test listed apart from a real failure.
- **A security-minded owner** wants every string from a test report escaped by one audited path, no secret beyond `GITHUB_TOKEN`, and a publisher pinned to a commit.
- **Someone trying a change locally** wants to render the Markdown from JUnit files with no network.

## Non-goals

- Publishing from the `pull_request` job itself for forks. It is impossible with a read-only token. In-job publishing is supported only for same-repository pull requests and pushes.
- Storing history in a database, a branch or a gist. History comes from the artefacts Actions already keeps.
- Running any code from the pull request.
- A dashboard. Trends over several base-branch runs are the furthest it goes.

## Requirements

**Must** is v0.1; **should** is v0.2 and v0.3; **could** is later.

### Must

- **M1.** Parse JUnit XML from nextest, including `<flakyFailure>` and `<rerunFailure>`, nested `<testsuite>`, `<testsuites>` or a bare `<testsuite>` root, `<skipped>`, `<error>`, `<system-out>` and `<system-err>`.
- **M2.** Parse the step-results JUnit that a product's converter writes (the contract is in [Inputs](#the-step-results-contract)), so each group of steps is a suite and each step a case.
- **M3.** Run as a publisher in a `workflow_run` workflow: download the triggering run's artefacts through the API, find and verify the pull request, and post.
- **M4.** One sticky pull request comment per (pull request, comment key), created once and updated in place, found by a hidden marker **and** an author of `github-actions[bot]` (or the configured app).
- **M5.** One check run per configured suite on the head SHA, with a conclusion (`success`, `failure`, or `neutral` for no results when not required) and a Markdown summary.
- **M6.** Escape every untrusted string through a single rendering type. No raw string reaches Markdown or HTML output.
- **M7.** Stay inside GitHub's limits with a budget and a degradation ladder: comment at most 60 000 characters, check summary at most 60 000, check text at most 60 000, annotations in batches of 50.
- **M8.** Configuration from `.github/pr-results.toml` on the default branch, with defaults that work for the usual artefact names when the file is absent.
- **M9.** Minimal permissions: `actions: read`, `checks: write`, `pull-requests: write`, `contents: read`. No secret other than `GITHUB_TOKEN`, and the token is never printed, logged or written to disk.
- **M10.** A composite action and a reusable workflow, both pinnable by commit SHA, with the binary verified by SHA-256.
- **M11.** A `render` subcommand that turns local JUnit files into the comment and check Markdown with no network, for goldens and local use.
- **M12.** Deterministic output: the same inputs give byte-identical Markdown (stable sort order, no clocks except the run times in the inputs).
- **M13.** Canadian English in every string `pr-results` writes.
- **M14.** Link the workflow run, its artefacts section and the commit in the footer.
- **M15.** Exit status `0` when it published (even if tests failed) and non-zero only when publishing itself failed. The check runs carry the test outcome. A `--fail-on-failures` flag exists for local use.

### Should

- **S1.** vitest, bun, jest-junit and Playwright JUnit flavours, with path mapping so annotations land on repository paths (v0.2).
- **S2.** Annotations with file and line: from `file` and `line` attributes (bun, jest-junit), from Rust panic locations in failure text (`panicked at src/x.rs:12:5`), and from the first repository-relative frame of a JavaScript stack (v0.2).
- **S3.** Flaky tests: passed after a retry in this run (nextest `flakyFailure`), listed in their own collapsed section (v0.2).
- **S4.** Base-branch comparison: new failures, failures already on the base branch, tests added and removed, and suites that got slower beyond a threshold, from the results artefact of the latest completed CI run on the base branch (v0.3).
- **S5.** An optional combined check run across all suites, for a single required status (v0.3).
- **S6.** An optional table of every other check run on the commit, so the comment is the pull request's one summary (v0.3).
- **S7.** A job summary written by the publisher with the same layout, so the publish run's own page is useful.
- **S8.** A `comment.mode` of `always`, `changes` (only when something differs from the base or the previous comment), `failures` or `off`.
- **S9.** A stale-run guard: a run for an older head SHA never overwrites a comment written for a newer one.

### Could

- **C1.** Read a `CHECK_TIMES` tab-separated file directly (`format = "steps-times"`), so shell projects need no converter. **Open question Q10.**
- **C2.** A generic step-results JSON (`pr-results-steps/1`) for projects that write steps from a script.
- **C3.** Trends over the last N base-branch runs: flaky rate and duration drift per test.
- **C4.** Playwright attachments (`[[ATTACHMENT|path]]`) linked to the run's artefacts.
- **C5.** Python (`unittest-xml-reporting` or `pytest --junitxml`), Godot GUT and Gradle JUnit documented and covered by fixtures.
- **C6.** A badge endpoint JSON written as an artefact.
- **C7.** macOS and Windows binaries for in-job use.

## Components

A Cargo workspace, added crate by crate with the code that needs it:

| Crate | Role | I/O |
| --- | --- | --- |
| `pr-results-core` | The normalized model (below), the `Safe` type, and the JUnit and step-results parsers with flavour detection and location derivation | none, pure |
| `pr-results-render` | Markdown from the model, built only from `Safe`, with budgets (not built yet) | none, pure |
| `pr-results-github` | A `Forge` trait and its `ureq` implementation, the artefact download (not built yet) | network |
| `pr-results` | The command line: configuration, wiring and `main` | files and network |

Everything except `pr-results-github` and the command line's `main` is pure and tested offline. `pr-results-github` sits behind the `Forge` trait, with an in-memory fake that records requests. Each crate has `#![forbid(unsafe_code)]`, and library code denies `unwrap`, `expect`, `panic` and unchecked indexing, so hostile input cannot panic the tool.

Dependencies, all permissively licensed and checked by `cargo deny`, each added with the code that needs it and pinned exactly: `quick-xml` is in; `serde`, `serde_json`, `toml`, `ureq` with `rustls`, `zip` (deflate only), `globset` and `sha2` follow. No async runtime, no `reqwest`, no OpenSSL. The command line has only `--version` and `--help` so far.

### Command line

```text
pr-results publish [--config PATH] [--event PATH] [--dry-run DIR]   # in workflow_run
pr-results render  [--config PATH] FILE...  [--out DIR]             # local, no network: comment.md, checks/*.md
pr-results collect [--config PATH] FILE...  --out report.json       # normalize in a CI job (optional)
pr-results validate-config PATH
pr-results --version
```

`--dry-run DIR` does everything except the writes and puts the would-be API requests as JSON in `DIR`, for testing a repository's setup.

### Packaging

A statically linked Linux binary (`x86_64` and `aarch64`, musl) attached to each GitHub release, run by a thin composite action whose `action.yml` carries the SHA-256 of each binary. Pinning the action by commit SHA therefore pins the exact binary. A reusable workflow in the same repository wraps the whole `workflow_run` publisher, so a product adopts it in about fifteen lines. A from-source fallback (`build: source`) covers any runner the binaries do not.

The composite action is about 40 lines of YAML that pick the target, download from the release whose tag is recorded in `action.yml`, verify the digest with `sha256sum -c` and run it. `build: source` runs `cargo build --locked --release` from `${{ github.action_path }}`.

## Inputs

Every input becomes the normalized model, which is also the format of `pr-results collect` output and of baselines. Per-flavour notes (what each reporter writes and where) are in [docs/inputs.md](docs/inputs.md).

| Format id | Flavours | Notes |
| --- | --- | --- |
| `junit` | `nextest`, `steps`, `vitest`, `bun`, `jest`, `playwright`, `python`, `generic` | Surefire-style attributes; the flavour decides how names and locations are read |
| `steps-times` (could) | none | A `CHECK_TIMES` file of `start` and `end` lines |
| `steps-json` (could) | none | The generic step JSON, schema `pr-results-steps/1` |

With `flavour = "auto"`, detection picks by the root's `name` attribute (`nextest-run`, `vitest tests`, `bun test`), by `assertions` attributes (bun) and by `[[ATTACHMENT|` (Playwright), and falls back to `generic`.

The `workflow_run` event is trusted. The uploaded pull request event file (`event-file` artefact, `event.json`) is **untrusted**: a fork controls it, and it is used only as a hint for the pull request number, always verified. Base-branch results are trusted only when the run's `event` is `push`, its `head_repository` is the base repository and its branch is the base branch. Configuration is trusted and is never read from the pull request head.

### The normalized model

```text
Report  { schema: "pr-results-report/1", tool_version, suites: [Suite] }
Suite   { key, display_name, source_files: [String], flavour, cases: [Case], time_ms }
Case    { id, name, classname, file?, line?, status, time_ms, attempts: [Attempt], message?, detail? }
Attempt { status, message?, detail? }   // nextest flakyFailure / rerunFailure
status  = passed | failed | errored | skipped | flaky (passed after a failed attempt)
id      = suite key + "\u{1f}" + classname + "\u{1f}" + name, after whitespace normalization
```

`detail` is capped at 16 KiB per case at parse time and `message` at 1 KiB, and names, class names and paths at 1 KiB. Suites with the same name across files merge, so a matrix of jobs that each upload part of a suite shows as one.

As built in `pr-results-core`: a `Report` holds `Suite`s (`name`, `flavour`, `cases`, `time_ms`), and a `Case` holds `name`, `classname`, `file`, `line`, `status`, `time_ms`, `attempts`, `attempts_omitted` (how many retries past `max_attempts` were not kept, always shown when not zero), `message`, `detail`, and the captured `system_out` and `system_err`. Every string is an `Untrusted`, which cannot be formatted except through `Safe`. `id` is computed by `Case::identity` rather than stored. `schema`, `tool_version`, `key`, `display_name` and `source_files` belong to the serialized report and the settings layer, and arrive with them.

### The step-results contract

For shell-step projects, a converter in the product (Desk's `tools/steps-junit`, once it exists) writes JUnit that `pr-results` reads as the `steps` flavour. The two sides must stay in step.

- Root `<testsuites name="…" time="…">`.
- One `<testsuite name="<group>">` per group of steps that ran, in script order, with `tests`, `failures`, `skipped` and `time` in seconds with millisecond precision.
- One `<testcase classname="<script>:<group>" name="<step title>" time="…">` per step, in order.
- The failing step has `<failure message="exit status N">` whose body is the last 200 lines of that step's log (the CI captures the log; the converter tails it). Steps after the failure in the same run are `<skipped message="not run: an earlier step failed"/>`.
- Groups not selected for this job are absent, not skipped, so a matrix of group jobs merges cleanly.
- No `hostname`, no absolute paths outside the workspace.

`pr-results` renders a `steps` suite with step titles as written and no `classname` prefix, and treats `time` as the main signal for duration tracking.

## Outputs

### The sticky comment

Layout rules: a heading with the product's title and a one-phrase outcome; one table row per suite; failures first and expanded only as far as the budget allows; everything long inside `<details>`; no emoji. Status is in words, with an optional `symbols = "glyphs"` setting that adds ✓ ✗ ↻ – before them. Times are written `1 m 47 s` and `0.4 s`. Numbers use a thin space as the thousands separator (`1 978`), as Canadian style allows. Deltas against the base branch go in a final column only when a baseline was found.

The full layout, with a worked example, is in [docs/adopting.md](docs/adopting.md#what-the-comment-looks-like). When everything passes, the comment shrinks to the heading (`All 696 passed`), the table and the footer. When no results were uploaded it says `No results were uploaded for this run.` and links the run. When the CI run was cancelled or skipped, the publisher does nothing.

**Marker.** The first line is `<!-- pr-results:1 key=<comment key> sha=<head sha> run=<run id> -->`. `key` lets one repository keep two comments (for example `ci` and `e2e`). `pr-results` finds the comment whose body starts with the marker `<!-- pr-results:1 key=<key>` and a space **and** whose author is the token's own identity (`github-actions[bot]` for `GITHUB_TOKEN`). It paginates all comments.

### Check runs

- One per suite, named by `suite.check` (default `"<title> <suite name>"`, for example `Example Rust tests`), so it never collides with a job name. The job summary warns when a configured check name equals a job name in the triggering run.
- `head_sha` is `workflow_run.head_sha`, `status` is `completed`, and `conclusion` is `failure` if any case failed or errored, `success` if all passed or were skipped, and `neutral` if the suite has no results and `required = false`. With `required = true` and no results it is `failure` with "No results were uploaded".
- `output.title` reads like `672 passed, 1 flaky` or `1 failed, 23 passed, 4 skipped`.
- `output.summary` is the suite's row, its failures (same rendering as the comment, larger budget) and a link back to the pull request comment.
- `output.text` is every failure's full detail up to the budget, then the slowest ten cases.
- The optional combined check (`[checks] combined = "…"`) carries the comment's table and the worst conclusion.

### Annotations

- One per failed or errored case with a known repository path, `annotation_level: failure`. Flaky cases are `warning` when `annotations.flaky = true`.
- `path` must exist in the commit's tree. `pr-results` checks paths with one `GET /repos/{o}/{r}/git/trees/{sha}?recursive=1` per run and drops annotations for paths that do not exist, rather than letting the API reject the batch.
- `start_line` comes from the input or a derived location, and is `1` when only the file is known. `title` is the case name (plain text, capped at 255 characters) and `message` is the first 64 KiB of message and detail.
- Sent 50 per request, and capped at `annotations.max_per_suite` (default 50, hard maximum 500). The cap and the count dropped are stated in the check summary.

### Size budget and degradation

Every renderer writes into a `Budget` with a character limit (comment 60 000, summary 60 000, text 60 000). The 5 000-character headroom covers the marker, the footer and GitHub's own counting of multi-byte characters. Rendering degrades in this order until it fits, and each step adds one line saying what was left out and where to find it:

1. Drop the slower-than-base and flaky sections to their counts.
2. Shorten each failure excerpt from `excerpt_lines` (default 20) to 8, then to 3.
3. Show at most `max_failures` failures (default 20), then 5, then none, with "…and N more failures in the check runs."
4. Collapse passing suites into one line: "12 other suites passed".
5. As a last resort, the heading, the totals and the footer, which always fit.

Truncation cuts at a `char` boundary and never inside an escape sequence, because escaping happens before measuring.

### Job summary

The publisher writes the comment body (without the marker) to `GITHUB_STEP_SUMMARY`, capped at 900 KiB, plus a short diagnostics block: inputs found per suite, files skipped and why, annotations dropped and the baseline run used.

### Limits

GitHub's limits that shape the budget: `output.summary` and `output.text` are each at most 65 535 characters; a request carries at most 50 annotations, so more need further `PATCH` calls; `annotation_level` is `notice`, `warning` or `failure`; and comment bodies are capped at 65 536 characters. A check run created by `GITHUB_TOKEN` attaches to a check suite of GitHub's choosing, so it can appear under an unexpected workflow in the UI. That is cosmetic.

## Settings and defaults

`.github/pr-results.toml`, read from the default branch's checkout (the reusable workflow checks out the default branch with `persist-credentials: false` and a sparse checkout of `.github/` only). Unknown keys are an error with a clear message, so typos do not silently change behaviour. Every key has a default, and a repository that uses the conventional artefact names needs no file.

```toml
# pr-results settings for an example product: one comment, a check run per suite.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

schema = 1
title = "Example checks"         # heading, and the prefix of default check names

[comment]
key = "ci"                       # marker key; change it only to keep a second comment
mode = "always"                  # always | changes | failures | off
max_failures = 20
excerpt_lines = 20
symbols = "text"                 # text | glyphs
other_checks = false             # list the commit's other check runs (needs checks: read)

[checks]
per_suite = true
combined = ""                    # a name such as "Example results" adds one combined check

[annotations]
max_per_suite = 50
flaky = false

[baseline]
branch = "main"                  # empty disables comparison
workflow = "CI"                  # the workflow whose runs on the branch hold the results
slower_percent = 25              # report a suite or step this much slower…
slower_min_seconds = 5           # …and at least this many seconds slower

[limits]
max_artifact_bytes = 104857600   # 100 MiB downloaded per run
max_files = 500
max_file_bytes = 52428800        # 50 MiB per XML file
max_cases = 200000

[[suite]]
name = "check.sh steps"
check = "Example check steps"
artifact = "steps-test-results"  # artefact name, glob allowed: "steps-test-results-*"
files = ["**/*.xml"]
format = "junit"                 # junit | steps-times | steps-json
flavour = "steps"                # auto | nextest | vitest | bun | jest | playwright | python | steps | generic
required = true

[[suite]]
name = "Rust tests"
check = "Example Rust tests"
artifact = "rust-test-results"
files = ["**/junit.xml"]
flavour = "nextest"
path_prefix = ""                 # added to file paths for annotations, e.g. "apps/web/" for vitest
```

| Setting | Default | Meaning |
| --- | --- | --- |
| `title` | the repository name in title case | Comment heading and prefix of default check names |
| `comment.key` | `ci` | Marker key |
| `comment.mode` | `always` | When to post |
| `comment.max_failures` | `20` | Failures shown before the budget degrades |
| `comment.excerpt_lines` | `20` | Lines of each failure shown |
| `comment.symbols` | `text` | Words only, or glyphs before them |
| `comment.other_checks` | `false` | List the commit's other check runs |
| `checks.per_suite` | `true` | One check run per suite |
| `checks.combined` | empty | Name of an optional combined check |
| `annotations.max_per_suite` | `50` | Annotation cap per suite, at most 500 |
| `annotations.flaky` | `false` | Annotate flaky cases as warnings |
| `baseline.branch` | `main` | Branch to compare against; empty disables |
| `baseline.slower_percent`, `baseline.slower_min_seconds` | `25`, `5` | When a suite or step counts as slower |
| `limits.*` | as above | Download and parse limits |

Defaults when there is no file: `title` is the repository name in title case, and suites are inferred from artefacts matching `*-test-results*`, one suite per artefact, named from the artefact (`rust-test-results` becomes "Rust tests", `frontend-test-results` becomes "Frontend tests"), with `flavour = "auto"` and `required = false`.

## The workflow_run flow and security

### Flow

```text
pull_request (untrusted, read-only token)            workflow_run: completed (trusted, default branch)
┌──────────────────────────────────────────┐         ┌──────────────────────────────────────────────────────────┐
│ CI jobs run tests                         │         │ 1. read the trusted event (run id, head sha, repo)        │
│ upload-artifact: rust-test-results, …     │ ──────▶ │ 2. sparse checkout .github/ of the default branch          │
│ upload-artifact: event-file (event.json)  │         │ 3. pr-results publish: list + download artefacts (limits)  │
└──────────────────────────────────────────┘         │ 4. parse, normalize, find and verify the pull request      │
                                                      │ 5. (should) fetch baseline artefacts from the base branch │
                                                      │ 6. check runs + annotations, sticky comment, job summary  │
                                                      └──────────────────────────────────────────────────────────┘
```

The CI side changes only by uploading artefacts, which a repository that reports results today already does, with `if: always()`.

### Permissions

The publisher job, and nothing broader at workflow level (`permissions: {}` at the top):

```yaml
permissions:
  actions: read        # list and download the run's artefacts; find baseline runs
  checks: write        # create and update check runs and annotations
  pull-requests: write # create and update the sticky comment
  contents: read       # sparse checkout of .github/ (needed in private repositories)
```

`checks: read` is covered by `checks: write` for `other_checks`. No `issues`, `statuses`, `id-token` or `packages` permission. The CI workflow keeps `contents: read` only.

### Finding and verifying the pull request

A fork controls `event.json`, so a forged file could name someone else's pull request. `pr-results`:

1. Uses `workflow_run.pull_requests[]` when it is non-empty (same-repository pull requests), but does not trust it blindly: each entry is re-read and must pass the same checks as step 2.
2. Otherwise lists `GET /repos/{o}/{r}/commits/{head_sha}/pulls` and keeps pull requests that are open, whose `head.sha` equals `workflow_run.head_sha`, whose `head.repo.full_name` equals `workflow_run.head_repository.full_name`, and whose `base.repo` is this repository.

   Pull requests from forks may be missing from both `workflow_run.pull_requests[]` and `commits/{sha}/pulls`. The fallback is `GET /repos/{o}/{r}/pulls?head={owner}:{branch}`, filtered by the same checks. **Not verified:** that the fallback finds fork pull requests where the other two do not.
3. Uses the event file's `pull_request.number` only as a tie-breaker among those candidates, never on its own.
4. If no pull request verifies, posts check runs (they are tied to the SHA) and no comment, and says so in the job summary.

Before writing the comment it re-reads the pull request. If `head.sha` has moved on and the existing comment's marker carries a newer run id, it leaves the comment alone (S9). The reusable workflow sets `concurrency: pr-results-${{ github.event.workflow_run.head_repository.full_name }}-${{ github.event.workflow_run.head_branch }}` with `cancel-in-progress: false`.

### Untrusted input rules

Everything from artefacts is untrusted: file names, XML content, artefact names and the event file.

- **XML.** `quick-xml` with DTDs rejected (`<!DOCTYPE` is an input error), no entity expansion beyond the five predefined and numeric references, a depth limit of 64, and attribute and text caps applied while streaming.
- **Archives.** Artefact zips are read in memory or in a temporary directory under `RUNNER_TEMP`. Entries with absolute paths, `..` components or symlink modes are rejected. The uncompressed total and the per-entry size are checked against `limits` before and during extraction (a zip-bomb guard), and at most `max_files` entries are read.
- **Text.** Invalid UTF-8 is replaced with U+FFFD. C0 and C1 controls other than tab and newline are removed. Unicode bidirectional controls (U+202A–U+202E, U+2066–U+2069) and U+200B–U+200F and U+FEFF are shown as `\u{…}`, so a name cannot visually reorder a line.
- **One escaping type.** The renderer accepts only `Safe` fragments. `Safe::text(&str)` escapes for inline Markdown inside HTML-capable contexts: the five HTML specials (ampersand, less-than, greater-than, both quotes) become entities, and the Markdown specials (backslash, backtick, asterisk, underscore, braces, brackets, parentheses, hash, plus, minus, full stop, exclamation mark, pipe, tilde, equals) are backslash-escaped wherever they could start syntax, with newlines replaced by a space. `@` is rendered as the fullwidth U+FF20 and `#` as the fullwidth U+FF03, so text outside code carries no mention or issue reference (**Open question Q9**: unverified, and entities were rejected because GitHub probably decodes them before its filters run). `Safe::code_span` writes inline code on one line, with a delimiter one backtick longer than the longest run in the content, and GitHub does not look for mentions in code. The renderer puts test names, file paths and failure text in `code_span` or `code_block`, and uses `text` only for the fixed words around them. `Safe::code_block(&str)` picks a fence one backtick longer than the longest run in the content (at least four) with the `text` info string. `Safe::link(url, text)` accepts only URLs built from trusted parts (repository, SHA, run id, repository-relative paths that were checked against the tree).
- **Paths for annotations** are normalized, must be relative, must not contain `..`, and must exist in the commit's tree.
- **No hostnames, timestamps, properties or environment values** from the inputs are rendered.
- **Secrets.** `pr-results` reads only `GITHUB_TOKEN` (or `--token-env NAME`), `GITHUB_EVENT_PATH`, `GITHUB_REPOSITORY`, `GITHUB_API_URL`, `GITHUB_SERVER_URL`, `GITHUB_STEP_SUMMARY` and `RUNNER_TEMP`. It sends the token only to `GITHUB_API_URL`, never forwards the `Authorization` header across a cross-host redirect (the artefact download redirects to a presigned blob URL that needs no token), never logs headers, and masks the token with `::add-mask::` defensively.
- **Workflow commands.** Untrusted names and messages written to a log or to an annotation command such as `::error file=…::` can inject `::` commands. They are escaped through the one `Safe` type: `%`, CR and LF always, and `:` and `,` as well inside annotation properties. Any untrusted log output is wrapped in `::stop-commands::<random token>` and resumed with the same token.
- **No execution.** Nothing from an artefact is run, sourced or used as a path outside the temporary directory. The reusable workflow never checks out the pull request head.
- **Network.** HTTPS only, `User-Agent: liminal-hq-pr-results/<version>`, timeouts, retries with backoff on 5xx and secondary rate limits (`retry-after`), and at most two writes a second.

## Adoption

### The reusable workflow (recommended)

In the product repository, `.github/workflows/publish-test-results.yml`:

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

The reusable workflow runs on `ubuntu-24.04` without a container so it starts fast, does the sparse checkout, runs the composite action and writes the job summary. Its inputs are `config` (path, default `.github/pr-results.toml`) and `build` (`release` or `source`).

### The composite action (in-job use or custom flows)

```yaml
- uses: liminal-hq-labs/pr-results@<full commit sha> # v0.1.0
  with:
    command: publish           # publish | render | collect
    config: .github/pr-results.toml
```

In-job use on `pull_request` works for same-repository pull requests and pushes. Forks and Dependabot need the `workflow_run` form, and the documentation says so plainly.

### Why not `liminal-hq/.github`

The shared `.github` repository holds CI images and a reusable packaging workflow, and its workflow audit has an untriaged backlog. Putting a privileged publisher there would couple its release cadence to the images. The reusable workflow lives in this repository, versioned with the binary. A pointer from `liminal-hq/.github` and from `labs-template`'s commented CI follows once v0.1 ships.

### Migration order

1. A shell-step product (v0.1), with the uploads, the publisher and a `pr-results.toml` for its two suites. **Open question Q5** on which product goes first.
2. Tauri products that run nextest and vitest through `workflow_run` today (v0.2): replace the reporter steps with the reusable workflow, with no configuration file unless names change.
3. A product that posts its own comment from a script (v0.3, with `other_checks = true`): retire the script.
4. Products that report in-job today: move to artefacts plus the publisher, which fixes forks and Dependabot for them.
5. Products with sharded Playwright runs or shell steps and no results shown today.
6. `labs-template`: add the publisher, commented, beside the Rust jobs.

## Versioning, pinning, CI and release

### Versions and pins

- Semantic versions `vX.Y.Z`. `0.y` may break between minor versions, and `CHANGELOG.md` says so.
- Tags only when the owner asks. No floating `v1` tag: consumers pin a full commit SHA with the version in a comment, `@<sha> # v0.1.0`, and Dependabot's `github-actions` ecosystem updates both.
- **Every action this repository uses is pinned by full commit SHA with the version in a comment.** Container images are pinned by digest.
- The marker carries a schema number (`pr-results:1`). A change to its form is a major change, and `pr-results` reads the previous form for one release.

### The tool's own CI

`.github/workflows/ci.yml` has one job per check. Today: `Markdown lint`, `Link check`, `Licence headers`, `Hero banner`, `Workflow lint` (actionlint) and `Workflow audit` (zizmor, blocking from day one). With the code, the PR that adds the crates uncomments and adapts: `Format` (`cargo fmt --all -- --check`), `Clippy` (`-D warnings`, all targets), `Tests` (`cargo nextest run --workspace --locked --profile ci`, uploading `rust-test-results`), `Goldens` (`pr-results render` over `tests/fixtures/`, diffed against `tests/golden/`), `Dependency policy` (`cargo deny check`), `Reproducible build` (builds the musl binaries twice and compares digests) and `Action smoke` (runs the composite action with `build: source` and `command: render`). Rust jobs run in the shared `ci-rust` image by digest, taken from the recommended pins of `liminal-hq/.github` at the time.

**Dogfood.** `publish-test-results.yml` in this repository uses the **latest release pinned by SHA**, never the pull request's own build, so a pull request cannot change the code that holds the write token.

### Release

1. A release pull request (`chore: release v0.1.0`) bumps `Cargo.toml`, moves `CHANGELOG.md`'s `Unreleased` section under the version, and writes the musl binaries' SHA-256 digests (from the `Reproducible build` job of that pull request) into `action.yml`.
2. The owner merges with a merge commit and asks for the tag.
3. On `push: tags: v*`, `release.yml` rebuilds in the same pinned image, **fails if a digest differs** from `action.yml`, attests build provenance (`actions/attest-build-provenance`) and creates the GitHub release with the binaries, `SHA256SUMS` and notes generated from `.github/release.yml` categories.

If the build proves not reproducible, fall back to a two-commit release (tag, build, then a "Pin the v0.1.0 binaries" commit that consumers pin) and record the decision in `docs/decisions.md`.

## Test plan

### Unit tests (no network)

- **Parsers.** Each flavour's fixture parses to the expected model: nested suites, bare `<testsuite>` roots, empty files, files with only skipped tests, nextest `flakyFailure` and `rerunFailure`, merged Playwright shards, and step suites merged across group jobs.
- **Location derivation.** Rust panic lines (`panicked at crates/x/src/lib.rs:12:5`, with and without the thread name), vitest stack frames, bun `file` and `line`, and paths with `path_prefix`.
- **Rendering.** The `Safe` type, fence selection, number and time formatting, and Canadian spellings in every fixed string (a test greps the binary's string table for `color`, `canceled`, `artifact` in prose and `behavior`).
- **Budget.** Each degradation step in order. Output never exceeds the limit for any fixture.
- **Pull request verification.** Same-repository, fork, forged event file, moved head, closed pull request, and two pull requests with the same head.
- **Marker matching.** A user comment with a forged marker is ignored, the bot's comment is updated, and a stale run does not overwrite a newer one.

### Fixtures

`tests/fixtures/`, real outputs trimmed and hostnames replaced:

- `nextest/{pass,fail,flaky,retries-exhausted,panic-location}.xml`
- `steps/{pass,step-fail,groups-split-a,groups-split-b}.xml` and `steps/check-times.tsv`
- `vitest/{pass,fail-with-stack,describe-nesting}.xml`, `bun/{pass,fail-file-line,nested}.xml`, `jest-junit/default.xml`, `playwright/{shard-1-of-2,shard-2-of-2,attachments}.xml`, `python/{xmlrunner,pytest}.xml` and `generic/{gradle,gut}.xml`
- `events/{workflow_run-pr,workflow_run-fork,workflow_run-push,uploaded-event-forged}.json`

### Goldens

`tests/golden/<fixture-set>/{comment.md,check-<suite>.md,annotations.json,requests.json}`, produced by `pr-results render` and by `publish --dry-run` against the fake. `PR_RESULTS_BLESS=1 cargo nextest run` rewrites them, and CI fails on any difference. Goldens are reviewed as Markdown in the pull request diff.

### Fork-safety and hostile-input tests

Fixtures whose names, messages and file paths contain: `</details><img src=x onerror=…>`, `<!-- pr-results:1 key=ci -->`, `[link](https://example.invalid)`, `![](…)`, `@everyone` and `@owner`, `#1`, `| table | breakout |`, a line of ```` ``` ```` and of `~~~`, bidi overrides, zero-width joiners, NUL and ESC bytes, a 10 MB message, 100 000 cases, 64-deep nesting, a `<!DOCTYPE>` with an entity bomb, invalid UTF-8, and zips with `../` entries, absolute paths, symlinks and a 1 GB inflated entry. Each must be rejected or rendered inert, within the budget, with the escaping visible in the golden.

A manual workflow (`workflow_dispatch`, owner only) publishes the hostile fixtures to a pull request in a sandbox repository and checks through the API that the rendered HTML contains no `<img>`, no links but this tool's own and no mentions.

### Integration and end to end

- `publish --dry-run` over each fixture set, asserting the request sequence (list artefacts, download, list pull requests, create check runs, annotations in batches of 50, list comments with pagination, update comment).
- A sandbox repository in `liminal-hq-labs` (private or public, the owner's choice) with a CI that uploads the fixtures and a publisher pinned to the pull request's build SHA, run manually: a same-repository pull request, a fork pull request and a Dependabot pull request.
- The tool's own dogfood publisher on every pull request, pinned to the latest release.

## Roadmap

| Version | Scope |
| --- | --- |
| **v0.1** | JUnit `generic`, `nextest` and `steps` flavours; `publish` in `workflow_run` with verified pull request, sticky comment and one check run per suite; `pr-results.toml` and defaults; `Safe` escaping, limits and budgets; `render`; the composite action and the reusable workflow; Linux `x86_64` binary; goldens and hostile fixtures |
| v0.2 | vitest, bun, jest-junit and Playwright flavours with `path_prefix`; annotations with file and line (attributes, Rust panics, JavaScript stacks); flaky from nextest retries; `aarch64` binary; job summary |
| v0.3 | Baseline comparison against the base branch (new and pre-existing failures, added and removed tests, slower suites and steps); combined check; `other_checks`; `comment.mode = changes`; stale-run guard hardening |
| v0.4 | Trends over N base runs (flaky rate, duration drift); Playwright attachment links; `steps-times` and `steps-json` inputs; Python, GUT and Gradle documented; graduation review |

## Open questions

Each carries the default this specification assumes. Settled questions are in [docs/decisions.md](docs/decisions.md).

- **Q5. First adopter.** Default: a shell-step product, once its step converter lands. A product that already publishes through `workflow_run` would be a quicker first proof, since it needs no new CI work.
- **Q8. Base-branch annotations.** Should pushes to the default branch also get check runs (default yes, no comment)?
- **Q9. Mentions.** Do the fullwidth `@` and `#` of `Safe::text`, and code formatting, stop mentions and issue links? Entities were dropped because GitHub's filters probably run on rendered text, so an entity decodes back to `@`. A sandbox test with the owner's own account must precede the renderer, and the tool refuses a name that still mentions.
- **Q10. `steps-junit` or `steps-times`.** Should a shell-step product keep its own converter (default), or should `pr-results` read `CHECK_TIMES` directly so the converter can go?
- **Q11. Pinning and branch protection.** Should the repository turn on `sha_pinning_required` and add a ruleset for `main` (default yes, once the checks have run green long enough to name as required)?

## What is not verified

- The workflow-command annotation limits per step and per job (GitHub's page does not state them), and the exact comment-body cap of 65 536 characters.
- Whether the fullwidth `@` and `#`, and code formatting, reliably suppress mentions and issue links in GitHub comments. This is to be tested in a sandbox, with the owner's own account, before the renderer is written.
- Whether GitHub Actions resolves `uses:` through a repository transfer redirect, which matters when the project graduates from Labs.
- Start-up times for the packaging options are estimates, not measurements.
- Reproducibility of musl release builds in the shared `ci-rust` image was not tried.
- Whether the shared `ci-rust` image carries `cargo-nextest`. Its recommended-pins note lists Rust and `cargo-deny` only.
- GitHub Marketplace availability of the action name.
