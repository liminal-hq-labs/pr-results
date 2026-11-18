# Changelog

All notable changes to this project are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows [Semantic Versioning](https://semver.org/). While the version is `0.y`, anything may change between minor versions, and this file says what.

## Unreleased

## 0.4.1 — 2026-11-17

### Fixed

- The sentence on the tests a pull request adds and removes counted a test once for each section it ran in, so a project that runs the same tests under several feature sets saw its counts multiplied (`jira-tui` saw 6 added and 4 removed for 3 and 2) and each name listed once for each section. The comparison is now of the distinct tests, so a test that runs in several sections is one test. Found on the first real pull request after v0.4.0. Decision D79 says so.

## 0.4.0 — 2026-11-17

### Added

- `comment.other_checks_open` (decision D76): keeps the table of other checks open under `#### Other checks`, not folded into `Other checks: 5 passed`, when every check passed. It is for guided and enhanced and is refused with `style = "plain"`, which never folds the table. It changes nothing when a check is not passing, and the size budget's own fold of passing rows still applies.
- The goldens `guided-other-checks-open` and `enhanced-other-checks-open`.
- `comment.updated_note` (decision D77): one fixed line before the footer, `This comment is updated in place with the latest results.` (with ♻️ in front for emoji symbols), so a reader knows the comment is edited on every run. It is on in the guided and enhanced styles and refused as `true` with `style = "plain"`. It is left out of the job summary, which is written once. `Safe::without_line` in the core crate, and the guided and enhanced goldens carry the line.
- `comment.test_delta` (decision D79, issue #37): one sentence after the summary, "This pull request adds 3 tests and removes 2 against `main`. A renamed test counts as both.", with a collapsed list of the names. It needs a baseline, which the publisher now reads (see the next entries), and it is on in the guided and enhanced styles and refused as `true` with `style = "plain"`. `pr_results_core::test_delta`, `TestDelta` and `TestId` compare two runs' tests, and `pr_results_render::Baseline` carries the result into `Input`.
- Finding a baseline (decision D78, issue #37). `Host::baseline_run`, with `BaselineQuery` and `RunRef`, finds the newest completed `push` run of a workflow in the repository itself that ended in `success` or `failure`, by exact commit or by branch, so a fork's run of the same name and a cancelled run are never taken for a baseline. GitHub implements it with one read of `actions/workflows/{id}/runs` and no new permission, and the other hosts find none. `FakeForge` serves the route (`FakeRun`, `add_workflow_run`).
- The publisher now compares a pull request's tests with its base's (decision D80, issue #37). For each pull request it looks for a baseline run on the base commit, then the base branch, then `baseline.branch`, reads that run's artefacts like the run's own with a budget of its own, and the comment says which tests were added and removed, and which branch it compared with when that was not the base branch. Only JUnit suites that are not `steps` are compared, and a suite the baseline cannot supply is left out on both sides. A base that is shared by several pull requests costs one lookup and one download, and a comment is rendered once for each distinct outcome. When there is no baseline the comment says nothing and the job summary says why. Plain output, Forgejo, `comment.test_delta = false` and an empty `baseline.branch` make the same requests as before, and no new permission is needed.
- `baseline.branch` is read (default `main`; empty turns baselines off), and `comment.test_delta = true` written with an empty `baseline.branch` is an error naming both keys. `Plan::baseline_branch`, `Settings::baseline_branch`, the notes `BaselineNotFound`, `BaselineUnavailable`, `BaselineUnusable` and `BaselineUsed`, and the request goldens `baseline-by-commit`, `baseline-fallback` and `baseline-not-found`.
- `Pull::base_ref` and `Pull::base_sha`, read from the API's `base.ref` and `base.sha` and left `None` when absent or malformed, and `WorkflowRun::workflow_id`, read from the trusted event's `workflow_run.workflow_id`.

### Changed

- **The default comment has one more line.** A settings file that says nothing about `comment.updated_note` now gets the note in the guided style, which is the default. Set `updated_note = false` to drop it. The plain style is unchanged byte for byte.
- **Breaking for library users (0.x):** `Options` has new public fields, `other_checks_open`, `updated_note` and `test_delta`, and `Settings` has the same three. `pr_results_render::Input` has a new public field, `baseline` (`None` for no baseline).
- **`baseline.workflow` is deprecated.** It is still accepted so an existing file loads, and has no effect: the baseline is taken from the workflow of the triggering run, which comes from the trusted event. `baseline.slower_percent` and `baseline.slower_min_seconds` are still checked and have no effect.
- **Breaking for library users (0.x):** `Plan` has a new public field, `baseline_branch`, and `Note` has four new variants.
- **Breaking for library users (0.x):** `Pull` has the fields `base_ref` and `base_sha`, `WorkflowRun` has `workflow_id` and `FakePull` has `base_ref` and `base_sha`. None of the crates is published.

## 0.3.0 — 2026-11-17

### Added

- `comment.other_checks` (SPEC S6, decision D69): with it on, the comment gains an `Other checks` table after the suite table, from the commit's check runs, ordered by what needs attention and then by name. It leaves out this tool's own check runs and the publisher's own workflow run, and a failed or action-required check is also said in the heading (`All 696 passed, 1 other check needs attention`). The first step of the budget folds five or more passing rows into one (`Degradation::OtherChecksCollapsed`). It is GitHub only, from `workflow_run` only, and says so in the notes where it does nothing; a listing that fails leaves the table out and still posts the results.
- `Host::check_runs`, `CheckRunInfo`, the renderer's `OtherCheck` and `CheckState`, `FakeForge::add_other_check`, and the goldens `other-checks`, `other-checks-glyphs`, `other-checks-all-pass` and `other-checks-hostile`.

- Comment styles (decisions D70 and D71): `comment.style` is `plain`, `guided` or `enhanced`. **Guided is the new default.** It adds a summary in a closed set of sentences that never say a pull request is ready, a list of what needs attention, glyphs on by default, a table of other checks that is folded when all of them passed, a collapsed list of the five slowest tests (at least 100 ms), and for a run with no results a short account of what to look at. Under a failed nextest test it shows `cargo nextest run -E 'binary_id(=…) & test(=…)'`, built only from names that pass a character allowlist; `suite.run_locally` sets the start of that command or turns it off.
- The enhanced style, and the extras it is made of, each a setting of its own: `comment.symbols = "emoji"`, `comment.mascot` (one line from Jax, from five fixed sentences), `comment.progress` (`737 of 738 tests passed ▰▰▰▰▰▰▰▰▰▱`, words first, never full while a test failed) and `comment.alerts` (the summary in a GitHub alert block, on GitHub only, holding fixed sentences only). The three extras are refused in the plain style.
- `comment.other_checks_ignore` (patterns for the names of checks to leave out of the table, such as the jobs whose failure the suites already show) and `[comment.check_commands]` (how to run a failed check on your own machine).
- `suite.job` (decision D72): a suite names the job its results come from, and that job's row leaves the table of other checks while the two agree (both passed or both failed), so a failing test is not counted once as a suite and again as a job. A job that failed with no results, or passed while its tests failed, keeps its row.
- `pr-results wrap --out FILE -- COMMAND` (decision D73) and `command: wrap` on the action: runs a command in the job that already runs it, passes its output through, exits with its own status, and writes the step JUnit of the `steps` contract, with the head and tail of the output of a failed command (and none of a passing one), so the comment can show why a format, lint or audit job failed without a trip to the job page. Bounded in memory, with escape sequences removed and invalid UTF-8 replaced.
- `suite.format = "sarif"` and `suite.fail_on` (decision D74): SARIF 2.x from a scanner, read as a suite of its own. A run is a suite and a result is a case with a file and a line, so the comment and the check run's annotations work as for tests. A finding at or above `fail_on` (default `warning`) fails and one below it is skipped; a clean run is one passing case. Typed, bounded reading that skips unused fields without storing or recursing into them. `parse_report` is the one entry point every reader goes through.
- `suite.format = "clippy-json"` and `"cargo-audit-json"` (decision D75): `cargo clippy --message-format=json` and `cargo audit --json` as findings, with the threshold and the clean-run case of SARIF. Clippy's diagnostics are placed at their primary span, de-duplicated across targets, with the summary lines left out; a file without `build-finished` is refused as cut short. The audit's advisories are at `Cargo.lock`, a vulnerability an error and a warning a warning. `strip_ansi` is shared with `wrap`.
- The smallest comment keeps a one-line count of the other checks (`Other checks: 1 failing, 6 passed.`) beside the heading and the totals.
- Goldens for the guided and enhanced styles, including a report of awkward test names, hostile check names and the budget's steps in the guided style; the inert check allows a quote only as one of the five GitHub alert kinds and never on Forgejo.

### Changed

- This repository's own pull request comment uses the enhanced style and lists its other jobs, so the project shows what the style looks like on GitHub.
- **The default comment is different.** A settings file that says nothing about `comment.style` now gets the guided style, with glyphs where the plain style had none. Set `style = "plain"` to keep the earlier layout, which is unchanged byte for byte. The renderer's own `Options::default()` is still plain; the settings are what default to guided.
- **Breaking for library users (0.x):** `Style` and `Symbols::Emoji` are new, `Options` has the fields `style`, `mascot`, `progress` and `alerts`, `Section` has `run_locally`, `OtherCheck` has `command`, `SectionPlan` has `run_locally`, `Plan` has `other_checks_ignore` and `check_commands`, `SuiteSettings` has `run_locally`, and `Settings` has `style`, `mascot`, `progress`, `alerts`, `other_checks_ignore` and `check_commands`. `Degradation::ExtrasToCounts` also drops the slowest tests. None of the crates is published.
- **Breaking for library users (0.x):** `pr_results_render::Input` has a new public field, `other_checks` (`&[]` for none), `Degradation` has a new first variant and `Degradation::ALL` has nine steps, `Plan` has the public fields `other_checks` and `own_run`, `FakeCheck` has `status` and `details_url`, and `Settings` has `other_checks`. None of the crates is published (`publish = false`).
- `comment.other_checks = true` is no longer refused by the settings loader.

## 0.2.0 — 2026-11-12

### Added

- Forgejo publishing with one commit status per suite and an author-verified sticky comment, plus an explicit HTTP opt-in for trusted networks.
- In-job publishing from bounded local artefacts on GitHub and Forgejo, with fork writes skipped and privileged events refused.
- Release toolchain checks for the recorded Rust and Cargo versions.

- `Target` (`GitHub` or `Forgejo`) and the run's number in the renderer's `RunContext`. For Forgejo the comment names no check runs and no GitHub limit, shows each failure's `path:line` (there are no annotations), links the commit and the run and not the artefacts, and writes a heading title that Forgejo would link (a bare commit hash, `!12`, `$`, `:emoji:`) in a code span (`forgejo_inert_title` says which). `status` returns a commit status per section: `Success` or `Failure` (a neutral section is `Success`) and the check's title, in plain text, at most 140 characters; `check` returns nothing for Forgejo (decision D64).
- Goldens `forgejo-all-pass`, `forgejo-failures`, `forgejo-hostile` and `forgejo-truncated-minimal` with a `status-N.txt` for each section, an inert check with Forgejo's extra rules, and budget tests for both targets. Every golden for GitHub is unchanged.
- The job summary carries the comment: `job_summary` is held to 900 KiB (`JOB_SUMMARY_CAP_BYTES`) by cutting after a whole line, closing any open code fence or `<details>` and saying how many lines were left out, and `PublishReport::job_summary` carries it. `publish` appends it, then its diagnostics, to `GITHUB_STEP_SUMMARY`, else `FORGEJO_STEP_SUMMARY`, both read through the environment it was given (decision D65).
- The job summary's cut understands indented and `~~~` code fences as well as backtick ones (`Safe::tildes` closes the latter), and a heading title with `://` or `www.` is written in a code span on Forgejo.
- `TrustedUrl::forgejo_run` and `TrustedUrl::comment_anchor`, and `Safe::first_lines` and `Safe::backticks`.
- `mention_probe` takes a team and a second repository and, with `--check-run --head-sha SHA`, writes a check run with annotations. `docs/probe-workflow.yml` is the owner's gate for the rest of Q9: it posts the comment as `github-actions[bot]` from a sandbox, reads back the rendered HTML, the timelines, the check run and its annotations (decision D66). It has not been run.
- The vitest, bun, jest-junit and Playwright readers were tuned against reports the reporters wrote for sample projects (decision D63): `tests/sample-projects`, the generator `scripts/gen-js-fixtures.sh` (pinned images, no network for the runs) and the `real-*.xml` fixtures with a model of each in `tests/models/`. `docs/inputs.md` records what each reporter writes, including that none of them records a test that failed and then passed.
- `suite.path_prefix` is read: it is put in front of every path a report names, so annotations land on repository paths for a package below the repository root, for bun's working directory and for Playwright's `testDir`.
- JavaScript stack frames give a failure its file and line: the first frame that is a repository path outside `node_modules`. An absolute or `file://` frame counts only when it ends with the case's own file.
- Playwright's project (`hostname`) is part of a suite's identity and is shown in brackets after the spec, so one spec run in two projects stays two suites.
- Extra `failure` or `error` elements in a case are attempts: vitest writes one for each attempt of a test that keeps failing.
- `scripts/anonymize-junit.sh` and `anonymize_junit`, which rewrite a JUnit report so that it keeps its shape and none of its words, for sharing a private project's report as a fixture (`docs/anonymizing-reports.md`). Tokens are an HMAC-SHA-256 of the word under a required salt (`--salt`, or `--random-salt`, which prints the one it made), element and attribute names outside the JUnit vocabulary are replaced, and addresses, ports and long numbers are replaced.
- An ignored 5 200-case measurement of a vitest report.

### Changed

- The `pr-results-github` crate is now `pr-results-forge`. The transport's spacing, retries, retry budget and paging moved into a host-neutral `Client<F>`, the publisher asks a forge only through a new `Host` trait, and the GitHub behaviour is `Host` implemented for `GitHub<F>`. `publish` and the command line are generic over `Host`. The output, the request goldens and the behaviour are unchanged.
- The `Reproducible build` job compares the digest it builds with the one pinned in `action.yml` and `publish.yml` only on a release pull request and on a tag; on any other pull request it proves that two builds agree and prints the digest, since the pins are the last release's.
- **Breaking for library users (0.x):** `pr_results_core::Suite` has a new public field, `project`, so code that builds a `Suite` with a struct literal must set it (`None` for every flavour but Playwright), and `Report::merge_same_named` now also keys on it. `pr_results_github::SectionPlan` has a new public field, `path_prefix`, and `pr_results::config::SuiteSettings` too. Neither crate is published (`publish = false`).
- A stack frame whose path holds an invisible or format character is skipped, and a frame without parentheses keeps the spaces in its path.
- bun's describe blocks read outermost first, jest-junit's repeated class name and leading space are dropped, a failure with no message (or bun's multi-line one) shows its first line, and Playwright is detected from the empty `id` and `name` on its root.

### Fixed

- The tag check of `release.yml` marks the checkout as a safe directory for git, which the pinned `ci-rust` container needs because its user does not own the checkout, and reports a comparison git could not make apart from a commit that is not on the default branch. The first run on `v0.1.0` failed closed on this and published nothing. `scripts/test-release-checks.sh` runs the check in that container as another user, and the `Release pins` job runs it.

## 0.1.0 — 2026-11-12

The first release: experimental, with Linux `x86_64` and `aarch64` binaries.

### Added

- The project skeleton: the specification in `SPEC.md`, the decisions in `docs/decisions.md`, the adoption, input and security guides, and the animated hero.
- Continuous integration for what exists: markdown lint, link check, licence headers, the hero banner, `actionlint` and `zizmor`, with every action pinned by full commit SHA.
- Dependabot for GitHub Actions, and for Cargo.
- The data model (`Report`, `Suite`, `Case`, `Attempt`) and `Safe`, the one type through which untrusted text is escaped for Markdown, HTML, mentions and workflow commands, with explicit size budgets, in the new `pr-results-core` crate.
- JUnit parsers for nextest (including `flakyFailure` and `rerunFailure`), vitest, jest-junit, bun, Playwright and Desk's step format, with limits on size, nesting, cases and attributes, no document types or custom entities, and no panics on hostile input.
- Per-producer fixtures in `tests/fixtures/`, hostile-input and truncation tests, and continuous integration for format, clippy and tests in the pinned `ci-rust` image, with a nextest `ci` profile.
- A `pr-results` command line with `--version` and `--help`.
- `Safe::code_span`, inline code on one line with an adaptive delimiter, as the form for test names and paths, since GitHub does not look for mentions in code.
- The trusted values a renderer needs, in `pr-results-core`: `Sha`, `RepoSlug`, `RunId`, `CommentKey`, `ServerUrl`, `RepoPath`, `TrustedUrl` (made only from a server, repository, commit or run) and `Marker`, which reads back only the exact line the tool writes. `Safe` gains `link`, `grouped` (`1 978`), `duration_ms` (`18.2 s`, `1 m 47 s`), `sha_short`, `path`, `plain_text`, `plain_block` and `after_first_line`.
- The `pr-results-render` crate: the sticky comment, a check run (title, summary, text) per suite with annotations, the no-results comment and the job summary, all built from `Safe` with no new external crate. Failures are `<details open>` blocks with names on Markdown lines, flaky tests are a collapsed list, and attempts the report could not keep are always shown.
- A size budget that counts the larger of characters and UTF-16 units and gives things up in a fixed order (flaky list to a count, excerpts of 8 then 3 lines, failures capped at 20, 5, none, passing suites in one row, then only the heading, totals and footer), saying in a line each what it left out.
- Goldens for 20 scenarios in `tests/golden/` (`PR_RESULTS_BLESS=1` rewrites them) with an inert check on each, budget tests at 2 000, 10 000 and 60 000 characters, and tests that the source never joins fragments except through `cat` and spells nothing the American way.
- The `mention_probe` example, which writes the Markdown that tests whether GitHub turns names into mentions or links, for the sandbox test of decision Q9. It also puts every target in the heading and adds targets with a pipe, a backtick and an escaped pipe.
- The result of the Q9 gate (decision D43): in a throwaway repository, the `text`, `code_span`, `code_block` and failure-layout forms made no mention, issue link or autolink, while the raw controls did. Real team mentions, cross-repository references, notifications, annotations, check-run text and the heading remain for v0.2.
- A `steps/desk-job.xml` fixture: Desk's one suite per job, with the group in the class name. The steps reader accepts a suite per group or per job.
- The `pr-results-github` crate: the `Forge` trait (the only way to the network), `UreqForge` on `ureq` with `rustls` and `ring`, `FakeForge`, an in-memory GitHub that follows the REST documentation and imposes GitHub's size limits, and `DryRun`, which keeps the writes it would have made. `GitHub<F>` reads artefacts, pull requests and the commit's tree and writes check runs (found by `external_id`, annotations 50 to a request) and the sticky comment (found by marker and author, duplicates removed so concurrent runs converge), and `publish` runs the whole flow with the pull request verified from the trusted event, retries with backoff, `retry-after` and rate limits honoured, no blind POST retries, and a minimal form retried once on a 422.
- A zip reader of our own on `miniz_oxide` for artefacts: central directory only, stored and deflated entries, every offset checked, CRC-32 verified, and zip64, encryption, split archives, other methods, hostile names, links, devices, bombs and lying headers refused. A real artefact written by `actions/upload-artifact` is read in a test.
- `Glob` in `pr-results-core` (`*`, `**` and `?`), for artefact names and file paths.
- `pr_results_render::check_with_paths`, which leaves out annotations for paths the commit's tree lacks.
- The command line: `pr-results publish` (`--config`, `--dry-run DIR`, `--token-env`, `--bot-login`, `--fail-on-failures`), `render` (local files to the comment and `checks/N.md`, no network) and `validate-config`, with no argument-parsing crate. `publish` runs only from a `workflow_run` workflow on the default branch and exits 2 before any request otherwise; exit status is 0 (published or nothing to do), 1 (publishing failed), 2 (usage, settings or environment) or 4 (tests failed, with `--fail-on-failures`). `--dry-run` writes `requests.json`, `comment.md` and `checks/`. Every line it prints passes through `Safe::log_line`.
- The settings file, `.github/pr-results.toml`, read with `toml` `=1.2.1` (`parse` only, five crates, decision D51): unknown keys, wrong types and out-of-range numbers are errors that name the key and the line, and so is a setting for a later version. Without suites, one is inferred from each plain-named `<stem>-test-results*` artefact, so Playwright shards merge.
- The composite action (`action.yml`) and the reusable workflow (`.github/workflows/publish.yml`): the binary of a release is downloaded over https and checked with `sha256sum -c` before it runs, and the check fails with a message while the digests are still placeholders. `build: source` builds the action's checkout with a pinned toolchain. Inputs reach a script only through `env`.
- This repository publishes its own results: `publish-test-results.yml` (a `workflow_run` workflow with exactly four permissions, running the default branch's source), fed by `rust-test-results` and `event-file` uploads in the `Tests` job.
- CI jobs `Goldens`, `Dependency policy`, `Action smoke` and `Release pins`, `scripts/check-pins.sh`, and `scripts/check-doc-yaml.sh`, which runs actionlint over the YAML in the documents.
- Adoption recipes for nextest, vitest, Playwright shards, bun and Desk in `docs/adopting.md`, and fixtures for the `workflow_run` event and a forged uploaded event file.
- Review fixes to the GitHub crate: a cap on inflated bytes across a run with each archive dropped once parsed (`read_zip_filtered`), comments kept as a 256-byte prefix while paging, a ten-minute budget for waiting to retry, annotations sent in separate requests that are read back rather than repeated, check runs matched by app as well as `external_id`, and blobs fetched only from GitHub's storage hosts (`UreqForge::with_blob_hosts` for GitHub Enterprise Server).
- New dependencies, recorded with licences, crate count and binary size in decision D44: `ureq` `=3.5.1` (`rustls` with `ring`, no `aws-lc`), `serde_json` `=1.0.154`, `miniz_oxide` `=0.9.1` and `toml` `=1.2.1` (decision D51). `deny.toml` allows ISC, BSD-3-Clause and CDLA-Permissive-2.0 for the crates that need them.
- The release: `scripts/build-release.sh`, a reproducible `x86_64-unknown-linux-musl` build (remapped paths, a constant `SOURCE_DATE_EPOCH`, `musl-tools` for `ring`), the `Reproducible build` CI job that builds it twice from two paths, compares the digests, prints them and uploads the binary, and `release.yml`, which on a `v*` tag rebuilds, fails if the digest is not the one `action.yml` and `publish.yml` pin, attests build provenance and creates the GitHub release with the binary, `SHA256SUMS` and generated notes (decisions D55 and D56). `docs/releasing.md` says how a release is cut and verified.
- `[profile.release]`: `opt-level = "z"`, fat LTO, one codegen unit, `panic = "abort"` and stripped symbols, which take the `x86_64` binary from 3 646 360 to 2 167 632 bytes.
- `scripts/check-pins.sh --unreleased`, which allows `PLACEHOLDER` digests in a release version.
- An `aarch64` binary beside the `x86_64` one: the `Reproducible build (aarch64)` job builds it on `ubuntu-24.04-arm`, and on GitHub's runners it reproduced twice (decision D56). `release.yml` builds and compares both targets and attests one `SHA256SUMS` for both.
- `scripts/check-toolchain.sh` and `scripts/musl-packages.txt`: the build jobs fail with "toolchain drift" if the installed `musl` packages differ from the recorded versions, or if a cargo configuration file or a `RUSTFLAGS`-like variable could override the build flags. The `Reproducible build` job also fails if its digest is not the one pinned.

### Changed

- The workspace version is `0.1.0`.
- `action.yml` and `publish.yml` carry a digest for each binary, `PR_RESULTS_SHA256_X86_64` and `PR_RESULTS_SHA256_AARCH64`, and the `Action smoke` job uses its own settings (`tests/fixtures/smoke.toml`) and checks the comment it renders.
- The action gives `GITHUB_TOKEN` to `publish` only.
- Decision D17 is amended: until v0.1.0 the dogfood publisher runs the default branch's reviewed source (`build: source`), and a follow-up after the tag pins the release.
- An annotation title is at most 255 characters and its message at most 65 535 bytes, the omitted-characters marker included (a 1,000-character name used to render 286), and the comment, summary and text limits are held to 65 535 whatever the setting.
- `Marker::parse` accepts a first line that ends in `\r\n`, as `Marker::prefix_matches` already did.
- Merging same-named suites no longer takes quadratic time: 100,000 distinct suites went from 10.6 s to 17 ms.
- `Safe` now spells out every format and default-ignorable character, including the tag block, variation selectors, the soft hyphen and the fillers, as `\u{…}`.
- A suite's `file` is stored once and shared by its cases: 200,000 cases under a 1 KiB path peak at 64 MiB instead of 273 MiB.
- `Limits` gains `max_suites` (10,000) and `max_text_bytes` (64 MiB of kept text plus a fixed overhead per piece of text, case and attempt, so it bounds the report's memory and not only its bytes of text), each refused with an error that names the limit, and `max_bytes` also applies after invalid UTF-8 is replaced.
- `Safe::text` also writes a hyphen between a letter or digit and a digit (`GH-123`) as the fullwidth U+FF0D, because a backslash escape is decoded before GitHub's reference filter. This also changes dates and versions in text outside code (`2026-11-02` and `v1-2` come out with fullwidth hyphens), which is why the renderer puts names in code.
- `Safe::text` and `Safe::log_line` write `@` and `#` as the fullwidth U+FF20 and U+FF03 instead of HTML entities, which GitHub probably decodes before its mention filters run. This is not verified against GitHub.
- `Safe` also spells out the `Zs` spaces other than U+0020 and the Braille blank U+2800, which can make two names look alike, and `merge_same_named` no longer joins names that were cut differently.
- `Debug` for `Untrusted` prints its size only, never the text.
- Retries past `max_attempts` are counted in `Case::attempts_omitted` instead of being dropped silently.

Not verified: the first run of the workflows on GitHub Actions and of the release workflow, the real GitHub API (the fork pull request fallback, rate-limit bodies, redirects), real team mentions, cross-repository references and notifications (decision D43 covers one sandbox), and artifact attestation end to end. The vitest, bun, jest and Playwright readers are parsed but not tuned until v0.2.
