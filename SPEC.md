# pr-results specification

The source of truth for behaviour. A change that alters behaviour updates this file in the same change. Why each choice was made is in [docs/decisions.md](docs/decisions.md); the per-format input notes are in [docs/inputs.md](docs/inputs.md), the threat model in [docs/security.md](docs/security.md) and the adoption guide in [docs/adopting.md](docs/adopting.md).

The data model, `Safe`, the trusted values a renderer needs, the JUnit parsers, the renderer (the comment, the check runs, their annotations, the commit statuses for Forgejo and the size budget), the artefact zip reader, the glob matcher, the GitHub client behind a `Forge` trait with its in-memory fake, the publish flow, the command line (`publish`, `render` and `validate-config`), the settings file, the composite action and the reusable workflow exist (see the notes in [Components](#components) and [the normalized model](#the-normalized-model)). They are tested against an in-memory GitHub and loopback servers; the workflows have not run on GitHub before the first release. A commit that is not a release carries the word `PLACEHOLDER` for the binary digest, and the action and the reusable workflow stop with a message until the release pull request writes the digest in. Everything else described here is not built yet. Where a decision is still the owner's, the text says **Open question** and names its number in the [list at the end](#open-questions).

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

- **C1.** Read a `CHECK_TIMES` tab-separated file directly (`format = "steps-times"`), so shell projects need no converter. Decided against for v0.1: a shell-step product keeps its own converter ([D40](docs/decisions.md)).
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
| `pr-results-core` | The normalized model (below), the `Safe` type, the trusted values (`Sha`, `RepoSlug`, `RunId`, `CommentKey`, `ServerUrl`, `RepoPath`, `TrustedUrl`, `Marker`) and the text built from them (`Safe::link`, `grouped`, `duration_ms`), the JUnit and step-results parsers with flavour detection and location derivation, and `Glob` (`*`, `**` and `?`) for artefact names and file paths | none, pure |
| `pr-results-render` | The comment, the check runs and their annotations from the model, and for a Forgejo target the comment and commit statuses, built only from `Safe`, with budgets | none, pure |
| `pr-results-forge` | The `Forge` trait (the transport) with `UreqForge` (the real client), `FakeForge` (an in-memory GitHub) and `DryRun`; `Client<F>` (spacing between writes, retries, the retry budget and paging, over any `Forge`); the `Host` trait (what the publisher needs from a forge's API) with `GitHub<F>` as its one implementation (artefacts, pull requests, tree, check runs, the sticky comment); the zip reader; and `publish`, the flow, generic over `Host` | network, only in `UreqForge` |
| `pr-results` | The command line: arguments, settings, the event and the environment, wiring, `--dry-run` files and `main` | files and network |

Everything except `UreqForge` and the command line's `main` is pure and tested offline. A `Host` turns the publisher's questions and writes into one forge's routes and shapes on a `Client`, and never sees the token: the token is added in one place, `UreqForge::send`, which a test of the source enforces. The safety of the sticky comment (whose comments are ours, which to update, when to leave it, what may be deleted) is in each host's `upsert_comment`, so every host runs a conformance suite (`crates/pr-results-forge/tests/common/conformance.rs`) over its own in-memory forge: an impostor with our marker is never updated or deleted, another key is left alone, a newer run for the same commit is not overwritten, duplicates go only if ours, and 250 comments over many pages still find ours. A stub host that refuses everything proves that `publish` asks only through `Host` ([D57](docs/decisions.md)). `pr-results-forge` sits behind the `Forge` trait, with an in-memory fake that follows GitHub's REST documentation, pages lists two to a page, imposes GitHub's size limits, scripts faults and records requests, and a loopback server that tests `UreqForge` itself. Each crate has `#![forbid(unsafe_code)]`, and library code denies `unwrap`, `expect`, `panic` and unchecked indexing, so hostile input cannot panic the tool.

Dependencies, all permissively licensed and checked by `cargo deny`, each added with the code that needs it and pinned exactly: `quick-xml`, `serde_json` (no `derive`, so no `serde` of our own), `miniz_oxide` (inflating, under our own zip reader instead of the `zip` crate) and `ureq` with `rustls` on the `ring` provider (and its `webpki-roots`) are in; `toml` (`parse` only: its span-keeping `DeTable`, for line numbers, and no `serde`) is in with the command line. There is no `sha2`: the action checks the binary with `sha256sum`. Globs are our own `Glob`, not `globset`. No async runtime, no `reqwest`, no OpenSSL, no `aws-lc`, no argument-parsing crate. The licences, the crate count and the binary size are in [D44](docs/decisions.md) and [D51](docs/decisions.md).

### Command line

```text
pr-results publish [--config PATH] [--dry-run DIR] [--token-env NAME] [--bot-login LOGIN] [--fail-on-failures]
pr-results render  [--config PATH] [--title TEXT] [--flavour NAME] FILE... [--out DIR]
pr-results validate-config PATH
pr-results --version | --help
```

`collect` (normalize in a CI job) is deferred, and refused with a message. A flag takes its value as `--flag value` or `--flag=value`.

**`publish`** runs in a `workflow_run` workflow on the default branch and nowhere else. It exits `2` before any request unless `GITHUB_EVENT_NAME` is `workflow_run`, `GITHUB_REF` is `refs/heads/<default branch>` (the default branch is the one the event names), the event's `repository` and its run's `repository` are `GITHUB_REPOSITORY` (without regard to case), `GITHUB_API_URL` and `GITHUB_SERVER_URL` are `https://` and the token is in the variable `--token-env` names (default `GITHUB_TOKEN`). Publishing from the test job itself is v0.2. The settings file is `--config`, which must exist, or `.github/pr-results.toml` when that exists, or the defaults. `--bot-login` is the login the sticky comment must be authored by (default `github-actions[bot]`).

`--dry-run DIR` reads from GitHub for real and writes nothing. It saves `requests.json` (every write it would have sent, with its body), `comment.md` (the first comment body) and `checks/<n>.md` (each check run's conclusion, title, summary and text, in the layout of the goldens) in `DIR`, and prints the comment.

**`render`** has no network. Without `--out` it prints the comment. With `--out DIR` it writes `comment.md` and `checks/<n>.md`, numbering the sections from 0. With no `[[suite]]` it is one section, "Tests", holding every file. With suites, each file goes to the first suite whose `files` patterns match its path or any tail of it, and a file no suite claims is an error. `--flavour` overrides the flavour of every suite. Its footer links point at a placeholder repository, because there is no run.

**`validate-config`** reads a settings file and exits `0` with the number of suites, or `2` with the key and the line.

**Exit status.**

| Code | Meaning |
| ---: | --- |
| `0` | Published, rendered or validated, even if tests failed; or nothing to do (a cancelled or skipped run, a head repository that has been deleted) |
| `1` | Publishing failed: artefacts could not be listed, or a check run or comment was refused. The message says which |
| `2` | The arguments, the settings file, a file given to `render` or the environment are wrong |
| `4` | Tests failed, and only with `--fail-on-failures` (never when publishing itself failed: that is `1`) |

**Defaults the command line does not expose.** There are no flags or settings for the hosts a blob may be fetched from, the app whose check runs are ours or the time allowed for retries. The client uses its defaults: `*.blob.core.windows.net` and `*.githubusercontent.com` for github.com, the API host alone for any other `GITHUB_API_URL` (a GitHub Enterprise Server whose artefacts live elsewhere is refused until a setting for it exists), `github-actions` as the check app, and a ten-minute budget for waiting to retry. When that budget is used up the log says "Retries stopped because the time allowed for waiting was used up." and the writes that then failed make the exit status 1 as any refused write does; the note alone does not.

**Logs.** Every line on standard output or standard error passes through `Safe::log_line`, so a name cannot start a workflow command. Markdown printed to standard output (the comment of `--dry-run`, and `render` without `--out`) has a backslash and a space put in front of any line that starts with `::` or `##[`; the files it writes are exact. The token is masked with `::add-mask::` before the first request.

### Packaging

A statically linked Linux binary (`x86_64-unknown-linux-musl`; and `aarch64-unknown-linux-musl`) attached to each GitHub release as `pr-results-<target>`, run by a thin composite action whose `action.yml` carries the SHA-256 of each binary. Pinning the action by commit SHA therefore pins the exact binary. A reusable workflow in the same repository wraps the whole `workflow_run` publisher, so a product adopts it in about fifteen lines.

The composite action takes `command`, `args`, `config`, `build` and `token`. Its `build: release` step picks the target with `uname -m`, downloads `https://github.com/liminal-hq-labs/pr-results/releases/download/<tag>/pr-results-<target>` with `curl --proto '=https'`, verifies the digest with `sha256sum -c` and runs the file. The tag and the digests are `env` values of the step, `PR_RESULTS_TAG`, `PR_RESULTS_SHA256_X86_64` and `PR_RESULTS_SHA256_AARCH64` (any other architecture gets a message that says to use `build: source`). A digest that is not 64 hexadecimal digits (in a commit that is not a release it is the word `PLACEHOLDER`) stops the step with a message that says so, and the check cannot be skipped. `build: source` installs a pinned toolchain with `rustup` and runs `cargo build --locked --release` from the action's own checkout under `$RUNNER_TEMP`. Inputs reach the shell only through `env`.

A reusable workflow cannot name its own commit, so `publish.yml` cannot call the action at the same revision. It downloads and verifies the binary inline, with the same tag and digests, and `scripts/check-pins.sh` fails if the two differ. It has no `build` input: building from source is a composite-action feature.

## Inputs

Every input becomes the normalized model, which is also the format of `pr-results collect` output and of baselines. Per-flavour notes (what each reporter writes and where) are in [docs/inputs.md](docs/inputs.md).

| Format id | Flavours | Notes |
| --- | --- | --- |
| `junit` | `nextest`, `steps`, `vitest`, `bun`, `jest`, `playwright`, `python`, `generic` | Surefire-style attributes; the flavour decides how names and locations are read |
| `steps-times` (could) | none | A `CHECK_TIMES` file of `start` and `end` lines |
| `steps-json` (could) | none | The generic step JSON, schema `pr-results-steps/1` |

With `flavour = "auto"`, detection picks by the root's `name` attribute (`nextest-run`, `vitest tests`, `bun test`), by `assertions` attributes (bun) and by `[[ATTACHMENT|` (Playwright), and falls back to `generic`.

The `workflow_run` event is trusted. The uploaded pull request event file (`event-file` artefact, `event.json`) is **untrusted**: a fork controls it, and it is used only as a tie-breaker for the pull request number. It is read only when more than one pull request has already passed the checks of the trusted event and the API (open, at the run's head commit, from the run's head repository, into this one), and only its `pull_request.number` (a whole number) is used, from an `event.json` of at most 1 MiB in an artefact of at most 2 MiB. If that number is one of the pull requests that passed, it chooses it; if not, it is ignored and up to five get the comment. It never adds a pull request, and is never checked for being true, only for being one of the candidates. Base-branch results are trusted only when the run's `event` is `push`, its `head_repository` is the base repository and its branch is the base branch. Configuration is trusted and is never read from the pull request head.

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
- One `<testsuite name="<group>">` per group of steps that ran, or one `<testsuite name="<job>">` per job, in script order, with `tests`, `failures`, `skipped` and `time` in seconds with millisecond precision. Both shapes are read.
- One `<testcase classname="<script>:<group>" name="<step title>" time="…">` per step, in order. Desk's converter writes `classname="check.<group>"`. The group is what follows the last colon of the class name, or failing that its last full stop.
- The failing step has `<failure message="exit status N">` whose body is the tail of that step's log (the CI captures the log; the converter tails it: 200 lines in this contract, 80 in Desk's converter, which may add `type="step"`). Steps after the failure in the same run are `<skipped message="not run: an earlier step failed"/>`, or are simply absent, as in Desk's converter.
- Groups not selected for this job are absent, not skipped, so a matrix of group jobs merges cleanly.
- No `hostname`, no absolute paths outside the workspace.

`pr-results` renders a `steps` suite with step titles as written and no `classname` prefix, and treats `time` as the main signal for duration tracking. A step's excerpt is the last lines of its log, where a test's is the first lines of its failure. A configured suite spans many `<testsuite>` elements and many artefacts, because a job can upload several files (Desk's are `results-<job>`, holding `steps.xml` and `nextest-*.xml`), so the unit the renderer works on is the section: one configured suite, one check run, one row of the table, holding all the suites read for it.

## Outputs

### The sticky comment

Layout rules: a heading with the product's title and a one-phrase outcome; one table row per suite; failures first and expanded only as far as the budget allows; everything long inside `<details>`; no emoji. Status is in words, with an optional `symbols = "glyphs"` setting that adds ✓ ✗ ↻ – before them. Times are written `1 m 47 s` and `0.4 s`. Numbers use a thin space as the thousands separator (`1 978`), as Canadian style allows. A column of deltas against the base branch is added only when a baseline was found, which arrives with baseline comparison in v0.3 and does not exist before it.

The full layout, with a worked example, is in [docs/adopting.md](docs/adopting.md#what-the-comment-looks-like). Each failure is a `<details open>` block: `<summary>Failure N of M</summary>`, then on Markdown lines the name as `` `suite` › `group` › `name` `` (the section's title in front when the comment has more than one section, and a group that repeats the suite's name left out), the message and the time, the number of earlier attempts and of attempts the report could not keep, and the excerpt in a code block, headed "First N of M lines" or "Last N of M lines" when it is cut. **A raw HTML line holds only trusted words and numbers**, because Markdown inside a raw HTML line is not rendered: names are never put in a `<summary>`, but on a Markdown line after a blank line. Flaky tests are a collapsed section, one line each. Every name, message and path is shown in a code span or a code block. The footer links the commit, the workflow run, its artefacts and shows the version. When everything passes, the comment shrinks to the heading (`All 696 passed`), the table and the footer. When no results were uploaded it says `No results were uploaded for this run.` and links the run. When the CI run was cancelled or skipped, the publisher does nothing.

**Marker.** The first line is `<!-- pr-results:1 key=<comment key> sha=<head sha> run=<run id> -->`. `key` lets one repository keep two comments (for example `ci` and `e2e`). `pr-results` finds the comment whose body starts with the marker `<!-- pr-results:1 key=<key>` and a space **and** whose author is the token's own identity (`github-actions[bot]` for `GITHUB_TOKEN`). It pages through all comments (at most 100 pages of 100). A comment for the same key from a later run for the same commit is left alone. Otherwise the oldest of the bot's comments for the key is patched, any other the bot wrote for the key is deleted, and a 404 on the patch creates one. With none, it creates one, lists again, and deletes the bot's duplicates so that concurrent runs converge on one: the oldest stays, and is patched with this run's text unless a later run for the same commit wrote it. Every comment is written with the bot's login checked, so a user's comment that copies the marker is never touched. A POST that failed with a 5xx or a dropped connection is not repeated until the list has been read for the marker it would have written. Listing keeps only each comment's id, login, account type, creation time and the first 256 bytes of its body, which holds the marker. The comment is ours when its author's login is the token's (`github-actions[bot]`); the `[bot]` suffix cannot be registered by a person, so the login alone is enough. **The race is not closed:** GitHub has no conditional `PATCH` for a comment, so an older run that read the comments before a newer run wrote, and writes after it, still overwrites it until the next run. The stale rule prevents an older run that finishes late from overwriting a newer one it can see, not that window of a few hundred milliseconds. If the re-list after a create does not yet show the new comment, a duplicate may remain until the next run removes it.

### Check runs

- One per suite, named by `suite.check` (default `"<title> <suite name>"`, for example `Example Rust tests`), so it never collides with a job name. The job summary warns when a configured check name equals a job name in the triggering run.
- `head_sha` is `workflow_run.head_sha`, `external_id` is `pr-results:1:<comment key>:<check name>`, `status` is `completed`, and `conclusion` is `failure` if any case failed or errored, `success` if all passed or were skipped, and `neutral` if the suite has no results and `required = false`. With `required = true` and no results it is `failure` with "No results were uploaded".
- `output.title` reads like `672 passed, 1 flaky` or `1 failed, 23 passed, 4 skipped`.
- `output.summary` is the suite's row, a table of its `<testsuite>` elements with their times (the step durations of a steps suite, at most 50 rows), its failures (same rendering as the comment, with its own budget), the flaky list, and a line stating how many annotations were sent and how many were not. A link back to the pull request comment is not part of v0.1.
- `output.text` is every failure's full detail up to the budget, with "…and N more failures" when some do not fit, then the slowest ten cases.
- The optional combined check (`[checks] combined = "…"`) carries the comment's table and the worst conclusion.

### Annotations

- One per failed or errored case with a known repository path, `annotation_level: failure`, in the order of the report. Flaky cases follow as `warning` when `annotations.flaky = true`. A path that is not whole, or breaks the location rules, is dropped.
- `path` must exist in the commit's tree. `pr-results` checks paths with one `GET /repos/{o}/{r}/git/trees/{sha}?recursive=1` per run and drops annotations for paths that do not exist, rather than letting the API reject the batch.
- `start_line` comes from the input or a derived location, and is `1` when only the file is known. `title` is the case name (plain text through `Safe::plain_text`, capped at 255 characters) and `message` is the message and detail as plain text (`Safe::plain_block`, which keeps line breaks), cut visibly so that the whole, marker included, is at most 65 535 bytes, and the title at most 255 characters and 255 UTF-16 units. The comment, summary and text limits are held to 65 535 whatever the setting.
- Sent 50 per request, and capped at `annotations.max_per_suite` (default 50, hard maximum 500). The cap and the count dropped are stated in the check summary, which counts the annotations that survive the tree check. A batch GitHub refuses with 422 is left out and noted, and the run is not failed for it. The check run is found by `GET /repos/{o}/{r}/commits/{sha}/check-runs?check_name=…&filter=latest`, its `external_id` and its app (`github-actions`, or the configured app), so another tool's check, or another app's with a copied `external_id`, is left alone. The run is written without annotations and each batch goes in a request of its own: GitHub appends them, so a batch that may have been applied is not sent again until the run's annotation count has been read back, and a run that already has annotations is sent only those beyond its count. If GitHub refuses the check run itself with 422 (too large), it is written once more with a minimal summary and text and no annotations.

### Forgejo

Forgejo has no check runs and no annotations. With `Target::Forgejo` (decision D64):

- **Commit statuses** replace check runs: `status` gives one per section, `success` or `failure` (a neutral section is `success`, since `skipped` and `warning` are not known to satisfy branch protection), with the check's title as the description in plain text, at most 140 characters. `check` gives nothing.
- **The sticky comment carries what a check run's summary holds.** Its fixed words name no check runs and no GitHub limit ("to fit the size limit", "…and N more failures, not listed here."), and each failure shows `` at `path:line` `` after its message when the case has a whole repository path, because there are no annotations. GitHub's comment is unchanged, as its annotations carry the location.
- **The footer** links the commit and the run (by its number when known), with no artefacts link.
- **Inertness.** Outside code Forgejo also links a bare commit hash (7 to 64 hexadecimal digits), `!12`, `$…$` and `:emoji:`. Names, messages and paths are in code, where it links nothing; the only ordinary text is fixed words and the heading's title. A title that holds `$`, `!` before a digit, seven hexadecimal digits in a row or `:word:` (checked with the colon and space that follow it) is written in a code span; `forgejo_inert_title` tells a loader which.

### Size budget and degradation

Every renderer works to a limit (comment 60 000, summary 60 000, text 60 000). A size is the larger of the text's characters and its UTF-16 units, because GitHub may count either; the 5 000-character headroom covers the marker and the footer. Rendering degrades in this order until it fits, each step adds one line saying what was left out and where to find it, and a step that would change nothing is not taken and not recorded (`Comment::steps_taken` lists the ones that were):

1. Drop the flaky list to its count (the slower-than-base section joins it with v0.3).
2. Shorten each failure excerpt from `excerpt_lines` (default 20) to 8, then to 3.
3. Show at most `max_failures` failures (default 20) from the start, then at most 20 if `max_failures` is higher, then 5, then none, with "…and N more failures in the check runs." (on Forgejo, "…and N more failures, not listed here.")
4. Collapse passing suites into one line: "12 other suites passed".
5. As a last resort, the marker, the heading, the totals and the footer, which fit in any limit worth setting. A check run's summary takes steps 1 to 3 and the last one. Its text is filled failure by failure and its tail is the slowest ten.

Truncation cuts at a `char` boundary and never inside an escape sequence, because escaping happens before measuring.

### Job summary

The publisher writes the comment body without its marker (`job_summary`) to `GITHUB_STEP_SUMMARY` (else `FORGEJO_STEP_SUMMARY`), then a short diagnostics block (`### pr-results`): the lines of the log, which are the checks written and their annotation counts, the comments, and every note such as "no open pull request matches this run", each a count and fixed words. The summary is held to 900 KiB (`JOB_SUMMARY_CAP_BYTES`), under the 1 MiB a step may write on either forge. A body over the cap is cut after a whole line, any open code fence or `<details>` is closed, and a last line says how many lines were left out; the comment limit of 65 535 characters keeps the comment far under the cap, so the cut is the guard for a limit raised later. A cancelled or skipped run writes none. Inputs found per suite, files skipped and the baseline run used join the diagnostics with the features that produce them.

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

A setting for a later version is an error, not a setting that does nothing: `comment.mode` of `changes` or `failures`, `comment.other_checks = true`, `checks.per_suite = false`, a non-empty `checks.combined` (v0.3), and a non-empty `suite.path_prefix` (v0.2). The `[baseline]` keys are checked and have no effect until v0.3. `suite.format` must be `junit`. Numbers are limited: `max_failures` and `excerpt_lines` to 1000, `annotations.max_per_suite` to 500, `limits.max_files` to 10 000, `limits.max_file_bytes` to 256 MiB, `limits.max_cases` to a million and `limits.max_artifact_bytes` to a gibibyte. A title, a suite name and a check name are 1 to 100 printable characters, two suites may not share a check name, and there are at most 50 suites. The limits map to the downloads and the parser: `max_artifact_bytes` bounds the bytes downloaded for a run and the inflated bytes of an artefact, `max_files` the entries of an artefact, and `max_file_bytes` the inflated size of one file and the parser's input.

Defaults when there is no file: `title` is the repository name in title case, and suites are inferred from artefacts matching `*-test-results*`, one suite per artefact, named from the artefact (`rust-test-results` becomes "Rust tests", `frontend-test-results` becomes "Frontend tests"), with `flavour = "auto"` and `required = false`, and the check named `<title> <suite name>`. The part before `-test-results` names the suite, so `e2e-test-results-1` and `e2e-test-results-2` are one suite, "E2e tests", read from `e2e-test-results*`. Only artefact names of letters, digits, `.`, `_` and `-` are used, because whoever wrote the workflow chose them (a fork included) and they become check names, and at most 20 suites are inferred. The suites are inferred at publish time from the run's artefact list, so they depend on the run.

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
3. Uses the event file's `pull_request.number` (read from `event.json` in the `event-file` artefact, at most 1 MiB, only if it is needed) only as a tie-breaker among those candidates, never on its own. If several pull requests verify and the event file picks none, the comment goes on each, up to five, and the job summary says so. Repository names are compared without regard to case.
4. If no pull request verifies, posts check runs (they are tied to the SHA) and no comment, and says so in the job summary.

Before writing the comment it re-reads the pull request and skips it if it is closed or if `head.sha` has moved on. If the existing comment's marker carries a later run id for the same commit, it leaves the comment alone (S9). The check runs are written first and the comment last, so a refused comment (a 403 from a read-only token) still leaves the checks posted and makes the exit status non-zero. A push gets check runs and no comment (D39), and a cancelled or skipped run gets nothing. The reusable workflow sets `concurrency: pr-results-${{ github.event.workflow_run.head_repository.full_name }}-${{ github.event.workflow_run.head_branch }}` with `cancel-in-progress: false`.

### Untrusted input rules

Everything from artefacts is untrusted: file names, XML content, artefact names and the event file.

- **XML.** `quick-xml` with DTDs rejected (`<!DOCTYPE` is an input error), no entity expansion beyond the five predefined and numeric references, a depth limit of 64, and attribute and text caps applied while streaming.
- **Archives.** Artefact zips are read in memory and nothing is written to disk, one at a time: each is downloaded, inflated only for the entries a section's file globs match, parsed and dropped, and the inflated bytes kept across the whole run are capped by `limits`. Our own reader takes the central directory only and stored and deflated entries only (what `upload-artifact` writes), and rejects zip64, encryption, split archives, other methods, absolute, `..`, backslash, drive-letter, control-character, empty and repeated names, duplicate entries, symlinks, devices, offsets outside the archive or into the directory, a local header that disagrees with the directory, and a bad CRC-32. At most `max_files` entries are read, the declared sizes are checked against `limits` before anything is inflated, and inflating stops at the declared size plus one byte (a zip-bomb guard).
- **Text.** Invalid UTF-8 is replaced with U+FFFD. C0 and C1 controls other than tab and newline are removed. Unicode bidirectional controls (U+202A–U+202E, U+2066–U+2069) and U+200B–U+200F and U+FEFF are shown as `\u{…}`, so a name cannot visually reorder a line.
- **One escaping type.** The renderer accepts only `Safe` fragments. `Safe::text(&str)` escapes for inline Markdown inside HTML-capable contexts: the five HTML specials (ampersand, less-than, greater-than, both quotes) become entities, and the Markdown specials (backslash, backtick, asterisk, underscore, braces, brackets, parentheses, hash, plus, minus, full stop, exclamation mark, pipe, tilde, equals) are backslash-escaped wherever they could start syntax, with newlines replaced by a space. `@` is rendered as the fullwidth U+FF20 and `#` as the fullwidth U+FF03, and a hyphen between a letter or digit and a digit (`GH-123`) as the fullwidth U+FF0D, so text outside code carries no mention or issue reference (**Open question Q9**: the forms the renderer emits produced no mention, issue link or autolink in one sandbox, see D43, but real teams, cross-repository references, notifications, annotations, check-run text and the heading are untested; entities were rejected because GitHub probably decodes them before its filters run). Because that depends on GitHub's filters, the renderer does not rely on it: it puts test names, file paths and failure text in `code_span` or `code_block`, and uses `text` only for the heading's title. `Safe::code_span` writes inline code on one line, with a delimiter one backtick longer than the longest run in the content, and GitHub does not look for mentions in code. `Safe::code_block(&str)` picks a fence one backtick longer than the longest run in the content (at least four) with the `text` info string. `Safe::link(url, label)` accepts only a `TrustedUrl`, which can be made only from the server, repository, commit hash and run id (`commit`, `run`, `run_artifacts`), each parsed with strict rules. Annotation titles and messages are plain text (`plain_text`, `plain_block`), because an annotation is not Markdown.
- **HTML lines hold only trusted words and numbers.** The renderer writes `<details>`, `<summary>` and `</details>` from literals and counts, never around a name, because Markdown inside a raw HTML line shows literally and untrusted text there would be markup.
- **Fragments never join into a reference.** `Safe::text` breaks `GH-123` inside one fragment, but `Safe::concat` can build it from `GH` and `-123`. Every line of the renderer is joined by `cat`, which puts a space between two fragments that would form a reference across the join, and a test fails if the renderer calls `Safe::concat` or `Safe::join` anywhere else.
- **Paths for annotations** are normalized, must be relative, must not contain `..`, and must exist in the commit's tree.
- **No hostnames, timestamps, properties or environment values** from the inputs are rendered.
- **Secrets.** `pr-results` reads only `GITHUB_TOKEN` (or `--token-env NAME`), `GITHUB_EVENT_NAME`, `GITHUB_REF`, `GITHUB_EVENT_PATH`, `GITHUB_REPOSITORY`, `GITHUB_API_URL`, `GITHUB_SERVER_URL`, `GITHUB_STEP_SUMMARY` and `FORGEJO_STEP_SUMMARY`. It sends the token only to `GITHUB_API_URL`, never forwards the `Authorization` header across a cross-host redirect (the artefact download redirects to a presigned blob URL that needs no token), never logs headers, and masks the token with `::add-mask::` defensively. In the code the token is added in one place, to a URL made of the API base and a path type that cannot hold a host; a `Link` header is followed only if it begins with the API base exactly; the HTTP library follows no redirect, and the blob is fetched by a separate call that carries no credentials and follows at most three more redirects, each to an `https://` URL (an `http://` redirect is refused) on a host from an allowlist: `*.blob.core.windows.net` and `*.githubusercontent.com` for github.com, the API host alone for another base, and configurable for GitHub Enterprise Server. Errors name a status or a fixed phrase, never text from a response or the library.
- **Workflow commands.** Untrusted names and messages written to a log or to an annotation command such as `::error file=…::` can inject `::` commands. They are escaped through the one `Safe` type: `%`, CR and LF always, and `:` and `,` as well inside annotation properties. The publisher's log holds only counts, fixed phrases and, for a section's title, `Safe::log_line`, so it needs no `::stop-commands::` wrapper. Any other untrusted log output must be wrapped in `::stop-commands::<random token>` and resumed with the same token.
- **No execution.** Nothing from an artefact is run, sourced or used as a path outside the temporary directory. The reusable workflow never checks out the pull request head.
- **Network.** HTTPS only, `User-Agent: liminal-hq-pr-results/<version>` (no address of any person), timeouts (15 s to connect, 60 s a call, 180 s for a blob), and at most two writes a second. A read or an idempotent write is retried on a 5xx or a dropped connection after 1, 2, 4 and 8 s; a 429 or a secondary limit is retried the same way for every method, honouring `retry-after` up to 60 s; a primary limit is waited out only if it resets within 60 s. Other 4xx are final, and a POST is never repeated blindly (see the marker above), nor is a request that appends annotations. Pages are followed up to 100. One publish spends at most ten minutes waiting to retry, after which failures are reported, not retried.

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

The reusable workflow runs on `ubuntu-24.04` without a container so it starts fast, does the sparse checkout of the default branch's `.github/`, verifies and runs the binary, and holds exactly the four permissions above on its job. Its one input is `config` (path, default `.github/pr-results.toml`).

### The composite action (in-job use or custom flows)

```yaml
- uses: liminal-hq-labs/pr-results@<full commit sha> # v0.1.0
  with:
    command: publish           # publish | render | validate-config
    config: .github/pr-results.toml
    args: --dry-run out        # split on whitespace
```

`publish` runs only from a `workflow_run` workflow on the default branch in v0.1. In-job publishing on `pull_request` (same-repository pull requests and pushes) is v0.2, and forks and Dependabot need the `workflow_run` form in any case. The documentation says so plainly.

### Why not `liminal-hq/.github`

The shared `.github` repository holds CI images and a reusable packaging workflow, and its workflow audit has an untriaged backlog. Putting a privileged publisher there would couple its release cadence to the images. The reusable workflow lives in this repository, versioned with the binary. A pointer from `liminal-hq/.github` and from `labs-template`'s commented CI follows once v0.1 ships.

### Migration order

1. A shell-step product (v0.1), with the uploads, the publisher and a `pr-results.toml` for its two suites. Desk goes first, after this repository's own dogfood ([D38](docs/decisions.md)): its CI already produces `results-*`, `event-file` and step JUnit.
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

`.github/workflows/ci.yml` has one job per check: `Markdown lint`, `Link check`, `Licence headers`, `Hero banner`, `Workflow lint` (actionlint, over the workflows and over the YAML in the documents), `Workflow audit` (zizmor, blocking from day one), `Format` (`cargo fmt --all -- --check`), `Clippy` (`-D warnings`, all targets), `Tests` (`cargo nextest run --workspace --locked --profile ci`, uploading `rust-test-results` and `event-file`), `Goldens` (every test that compares output with a golden file, which cannot be blessed under `CI`), `Dependency policy` (`cargo deny check`), `Action smoke` (runs the composite action with `build: source` for `validate-config` and `render`) and `Release pins` (`scripts/check-pins.sh`: the tag and digests in `action.yml` and `publish.yml` agree, a placeholder digest is allowed only while the version ends in `-dev` or with `--unreleased`, and every action and image is pinned). `Reproducible build` (`scripts/build-release.sh` builds each musl binary twice in the image on a runner of its architecture, the second time from a copy of the tree at another path, compares the digests, prints them and uploads the binary, and on a release pull request (a head branch named `release/*` or `chore/release-*`) and on a tag also fails if they differ from the ones pinned in `action.yml` and `publish.yml`, because on any other change the pins are the last release's and differ by design; `scripts/check-toolchain.sh` fails first if the `musl` packages differ from `scripts/musl-packages.txt`). The release pull request fails `Release pins` until the digests are written in, because a release version allows no `PLACEHOLDER` (`check-pins.sh --unreleased` allows it). Rust jobs run in the shared `ci-rust` image by digest, taken from the recommended pins of `liminal-hq/.github` at the time.

**Dogfood.** `publish-test-results.yml` in this repository is a `workflow_run` workflow with exactly the four permissions, which checks out the default branch with no credentials and runs `uses: ./` with `build: source`. Until the first release it therefore runs the reviewed source of the default branch, never the pull request's build, so a pull request cannot change the code that holds the write token; it is inert until the file is on the default branch. A follow-up after the v0.1.0 tag pins the **latest release by SHA**, as every adopter does ([D17](docs/decisions.md)). The `workflow_run` trigger carries an inline `# zizmor: ignore[dangerous-triggers]` with its reason.

### Release

1. A release pull request (`chore: release v0.1.0`) bumps `Cargo.toml` and `Cargo.lock`, moves `CHANGELOG.md`'s `Unreleased` section under the version, and writes the `x86_64` musl binary's SHA-256 (from the `Reproducible build` job of that pull request) into `action.yml` and `publish.yml`.
2. The owner merges with a merge commit and asks for the tag.
3. On `push: tags: v*`, `release.yml` checks that the tag is a plain version equal to `Cargo.toml`'s and on the default branch, rebuilds with `scripts/build-release.sh` in the same pinned image and **fails if the digest differs** from the one in `action.yml` or `publish.yml`. Its release job, with `contents`, `id-token` and `attestations` write, attests build provenance (`actions/attest-build-provenance`, which is available for public repositories on GitHub Free) for `SHA256SUMS` and creates the GitHub release with `pr-results-x86_64-unknown-linux-musl`, `SHA256SUMS` and notes generated from `.github/release.yml` categories. `docs/releasing.md` has the commands, including `gh attestation verify`.

If the build proves not reproducible, fall back to a two-commit release (tag, build, then a "Pin the v0.1.0 binaries" commit that consumers pin) and record the decision in `docs/decisions.md`.

## Test plan

### Unit tests (no network)

- **Parsers.** Each flavour's fixture parses to the expected model: nested suites, bare `<testsuite>` roots, empty files, files with only skipped tests, nextest `flakyFailure` and `rerunFailure`, merged Playwright shards, and step suites merged across group jobs.
- **Location derivation.** Rust panic lines (`panicked at crates/x/src/lib.rs:12:5`, with and without the thread name), vitest stack frames, bun `file` and `line`, and paths with `path_prefix`.
- **Rendering.** The `Safe` type, fence selection, number and time formatting, and Canadian spellings in every fixed string (a test scans the renderer's source and every golden for `color`, `canceled`, `artifact` in prose and `behavior`). Every golden passes an inert check: outside code there is no `<img`, no link the tool did not build, no marker after the first line, no `@`, no `#` outside a heading, no URL to autolink, no `GH-123` shape and no HTML but the trusted lines, and every `<details>` is closed.
- **Budget.** Each degradation step in order. Output never exceeds the limit for any fixture.
- **Pull request verification.** Same-repository, fork, forged event file, moved head, closed pull request, and two pull requests with the same head.
- **Marker matching.** A user comment with a forged marker is ignored, the bot's comment is updated, and a stale run does not overwrite a newer one.

### Fixtures

`tests/fixtures/`, real outputs trimmed and hostnames replaced:

- `nextest/{pass,fail,flaky,retries-exhausted,panic-location}.xml`
- `steps/{pass,step-fail,groups-split-a,groups-split-b,desk-job}.xml`
- `vitest/{pass,fail-with-stack,describe-nesting}.xml`, `bun/{pass,fail-file-line,nested}.xml`, `jest-junit/default.xml`, `playwright/{shard-1-of-2,shard-2-of-2,attachments}.xml`, `python/{xmlrunner,pytest}.xml` and `generic/{gradle,gut}.xml`
- `events/{workflow_run-pr,workflow_run-fork,workflow_run-push,uploaded-event-forged}.json`

### Goldens

`tests/golden/<scenario>/{comment.md,check-<n>.md,annotations.txt}`, produced by the render crate's tests from scenarios written in Rust (one `truncated-<step>` for each step of the budget). `requests.json` is written by `publish --dry-run`. `PR_RESULTS_BLESS=1 cargo nextest run` rewrites them, and CI fails on any difference. Goldens are reviewed as Markdown in the pull request diff.

### Fork-safety and hostile-input tests

Fixtures whose names, messages and file paths contain: `</details><img src=x onerror=…>`, `<!-- pr-results:1 key=ci -->`, `[link](https://example.invalid)`, `![](…)`, `@everyone` and `@owner`, `#1`, `| table | breakout |`, a line of ```` ``` ```` and of `~~~`, bidi overrides, zero-width joiners, NUL and ESC bytes, a 10 MB message, 100 000 cases, 64-deep nesting, a `<!DOCTYPE>` with an entity bomb, invalid UTF-8, and zips with `../` entries, absolute paths, symlinks and a 1 GB inflated entry. Each must be rejected or rendered inert, within the budget, with the escaping visible in the golden.

A manual workflow (`workflow_dispatch`, owner only) publishes the hostile fixtures to a pull request in a sandbox repository and checks through the API that the rendered HTML contains no `<img>`, no links but this tool's own and no mentions.

### Integration and end to end

- `publish` over `FakeForge` from each of `tests/fixtures/events/{workflow_run-pr,workflow_run-fork,workflow_run-push}.json` and the forged `uploaded-event-forged.json`, and `publish --dry-run`, asserting the request sequence (list artefacts, download, list pull requests, create check runs, annotations in batches of 50, list comments with pagination, update comment), the exit codes and the refusals.
- A sandbox repository in `liminal-hq-labs` (private or public, the owner's choice) with a CI that uploads the fixtures and a publisher pinned to the pull request's build SHA, run manually: a same-repository pull request, a fork pull request and a Dependabot pull request.
- The tool's own dogfood publisher on every pull request, pinned to the latest release.

## Roadmap

| Version | Scope |
| --- | --- |
| **v0.1** | JUnit `generic`, `nextest` and `steps` flavours; `publish` in `workflow_run` with verified pull request, sticky comment and one check run per suite; `pr-results.toml` and defaults; `Safe` escaping, limits and budgets; `render`; the composite action and the reusable workflow; Linux `x86_64` and `aarch64` binaries; goldens and hostile fixtures |
| v0.2 | vitest, bun, jest-junit and Playwright flavours with `path_prefix`; annotations with file and line (attributes, Rust panics, JavaScript stacks); flaky from nextest retries; job summary |
| v0.3 | Baseline comparison against the base branch (new and pre-existing failures, added and removed tests, slower suites and steps); combined check; `other_checks`; `comment.mode = changes`; stale-run guard hardening |
| v0.4 | Trends over N base runs (flaky rate, duration drift); Playwright attachment links; `steps-times` and `steps-json` inputs; Python, GUT and Gradle documented; graduation review |

## Open questions

Each carries the default this specification assumes. Settled questions are in [docs/decisions.md](docs/decisions.md).

- **Q9. Mentions (verified in part, D43).** Verified on one sandbox: `text`, `code_span`, `code_block` and the failure layout produced no anchor for a user mention, `@everyone`, `#1`, `GH-1` (read as an issue reference) or a URL, while the raw controls did. Still open: real team mentions, cross-repository references to a real repository, notifications, annotations, check-run text and the heading, as v0.2 follow-ups. The probe now covers all of them, and `docs/probe-workflow.yml` posts it from a sandbox as `github-actions[bot]` and reads it back (D66); it has not been run.
  Original question: Do the fullwidth `@`, `#` and hyphen-before-a-digit of `Safe::text`, and code formatting, stop mentions, issue links and `GH-123`-style autolinks? The renderer puts every name in code, so the answer decides what is left to fix. The sandbox test must cover `@user`, `@org/team`, `@everyone`, `#1`, `org/repo#1`, `GH-1`, a URL and `www.`, through `cargo run -p pr-results-render --example mention_probe` posted as the bot, and must pass before the renderer merges. If a code form fires, such names become "a name that could not be shown safely (N)" and the question returns to the owner. If only `text` fires, `@` is written `(at)` and `#` `No.`, and the probe is repeated. Entities were dropped because GitHub's filters probably run on rendered text, so an entity decodes back to `@`.

Q5, Q8, Q10 and Q11 are answered in [docs/decisions.md](docs/decisions.md) (D38 to D41).

## What is not verified

- The workflow-command annotation limits per step and per job (GitHub's page does not state them), and the exact comment-body cap of 65 536 characters.
- Whether the fullwidth `@`, `#` and hyphen, and code formatting, reliably suppress mentions and issue links in GitHub comments. This is to be tested in a sandbox, with the owner's own account, before the renderer merges.
- Whether GitHub counts a comment's length in characters or in UTF-16 units. The renderer keeps both under the limit.
- That Desk's converter writes the shape this renderer reads. The `desk-job` fixture is written from its plan, not captured.
- Whether GitHub Actions resolves `uses:` through a repository transfer redirect, which matters when the project graduates from Labs.
- Start-up times for the packaging options are estimates, not measurements.
- That the release build reproduces at the tag. Both binaries reproduced twice on GitHub's runners in a pull request (decision D56), but the first tag run is the proof. `musl-tools` is installed with `apt`; `scripts/musl-packages.txt` records the versions and the build fails on drift, but a package that is withdrawn from the archive would block a release until the build is re-pinned.
- That `actions/attest-build-provenance` and `gh attestation verify` work end to end for this repository. The action's documentation says public repositories on every current plan can use attestations; no attestation has been made.
- Whether the shared `ci-rust` image carries `cargo-nextest`. Its recommended-pins note lists Rust and `cargo-deny` only.
- GitHub Marketplace availability of the action name.
- That `FakeForge` behaves as GitHub does. It follows the REST documentation for the dozen routes used, and nothing has talked to the real API: the order and shape of the `Link` header, a 422 for an annotation on a path outside the tree (assumed, and switchable in the fake), whether a later update with annotations keeps an earlier `text`, which is why every update resends the title, summary and text, and the exact 403 bodies of the rate limits.
- That the fork fallback (`pulls?head=owner:branch`) finds fork pull requests where the other two routes do not, and that `workflow_run.pull_requests[]` and `commits/{sha}/pulls` are empty for them.
- The real TLS handshake to `api.github.com` and the real redirect target of an artefact download. The loopback tests prove where the token goes, not what GitHub answers.
- GitHub Enterprise Server: the base accepts a path (`https://host/api/v3`), and nothing else was tried. The blob host allowlist for github.com is from GitHub's documented storage names and was not checked against a live download; a name GitHub adds outside it would be refused until the list is widened.
- That any workflow here runs on GitHub: `action.yml`, `publish.yml`, `publish-test-results.yml` and the new `ci.yml` jobs are checked by actionlint, zizmor (also `--pedantic`) and a local test of their shell steps, and nothing else. In particular: `workflow_run` runs the file from the default branch only, so the dogfood publisher is inert until merged; `uses: ./` and `github.action_path` in a composite action; `rustup` and a pinned toolchain on the runner for `build: source`; the sparse checkout of `.github`; `actions/upload-artifact` inside the `ci-rust` container (it needs the runner's Node); the concurrency group expression in a reusable workflow; and that the caller's four permissions reach the called job.
- That `toml` reports a line for every kind of mistake in a settings file. Spans were checked for the cases in the tests.
- That the fixtures of the `workflow_run` event match a real payload. They are written by hand from GitHub's documentation with only the fields the tool reads.
