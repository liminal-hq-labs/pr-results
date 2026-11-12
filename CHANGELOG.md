# Changelog

All notable changes to this project are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows [Semantic Versioning](https://semver.org/). While the version is `0.y`, anything may change between minor versions, and this file says what.

## Unreleased

### Added

- `Target` (`GitHub` or `Forgejo`) and the run's number in the renderer's `RunContext`. For Forgejo the comment names no check runs and no GitHub limit, shows each failure's `path:line` (there are no annotations), links the commit and the run and not the artefacts, and writes a heading title that Forgejo would link (a bare commit hash, `!12`, `$`, `:emoji:`) in a code span (`forgejo_inert_title` says which). `status` returns a commit status per section: `Success` or `Failure` (a neutral section is `Success`) and the check's title, in plain text, at most 140 characters; `check` returns nothing for Forgejo (decision D64).
- Goldens `forgejo-all-pass`, `forgejo-failures`, `forgejo-hostile` and `forgejo-truncated-minimal` with a `status-N.txt` for each section, an inert check with Forgejo's extra rules, and budget tests for both targets. Every golden for GitHub is unchanged.
- The job summary carries the comment: `job_summary` is held to 900 KiB (`JOB_SUMMARY_CAP_BYTES`) by cutting after a whole line, closing any open code fence or `<details>` and saying how many lines were left out, and `PublishReport::job_summary` carries it. `publish` appends it, then its diagnostics, to `GITHUB_STEP_SUMMARY`, else `FORGEJO_STEP_SUMMARY`, both read through the environment it was given (decision D65).
- The job summary's cut understands indented and `~~~` code fences as well as backtick ones (`Safe::tildes` closes the latter), and a heading title with `://` or `www.` is written in a code span on Forgejo.
- `TrustedUrl::forgejo_run` and `TrustedUrl::comment_anchor`, and `Safe::first_lines` and `Safe::backticks`.
- `mention_probe` takes a team and a second repository and, with `--check-run --head-sha SHA`, writes a check run with annotations. `docs/probe-workflow.yml` is the owner's gate for the rest of Q9: it posts the comment as `github-actions[bot]` from a sandbox, reads back the rendered HTML, the timelines, the check run and its annotations (decision D66). It has not been run.

### Changed

- The `pr-results-github` crate is now `pr-results-forge`. The transport's spacing, retries, retry budget and paging moved into a host-neutral `Client<F>`, the publisher asks a forge only through a new `Host` trait, and the GitHub behaviour is `Host` implemented for `GitHub<F>`. `publish` and the command line are generic over `Host`. The output, the request goldens and the behaviour are unchanged.
- The `Reproducible build` job compares the digest it builds with the one pinned in `action.yml` and `publish.yml` only on a release pull request and on a tag; on any other pull request it proves that two builds agree and prints the digest, since the pins are the last release's.

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
