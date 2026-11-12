# Changelog

All notable changes to this project are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows [Semantic Versioning](https://semver.org/). While the version is `0.y`, anything may change between minor versions, and this file says what.

## Unreleased

### Added

- The project skeleton: the specification in `SPEC.md`, the decisions in `docs/decisions.md`, the adoption, input and security guides, and the animated hero.
- Continuous integration for what exists: markdown lint, link check, licence headers, the hero banner, `actionlint` and `zizmor`, with every action pinned by full commit SHA.
- Dependabot for GitHub Actions, and for Cargo.
- The data model (`Report`, `Suite`, `Case`, `Attempt`) and `Safe`, the one type through which untrusted text is escaped for Markdown, HTML, mentions and workflow commands, with explicit size budgets, in the new `pr-results-core` crate.
- JUnit parsers for nextest (including `flakyFailure` and `rerunFailure`), vitest, jest-junit, bun, Playwright and Desk's step format, with limits on size, nesting, cases and attributes, no document types or custom entities, and no panics on hostile input.
- Per-producer fixtures in `tests/fixtures/`, hostile-input and truncation tests, and continuous integration for format, clippy and tests in the pinned `ci-rust` image, with a nextest `ci` profile.
- A `pr-results` command line with `--version` and `--help` only.
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
- Review fixes to the GitHub crate: a cap on inflated bytes across a run with each archive dropped once parsed (`read_zip_filtered`), comments kept as a 256-byte prefix while paging, a ten-minute budget for waiting to retry, annotations sent in separate requests that are read back rather than repeated, check runs matched by app as well as `external_id`, and blobs fetched only from GitHub's storage hosts (`UreqForge::with_blob_hosts` for GitHub Enterprise Server).
- New dependencies, recorded with licences, crate count and binary size in decision D44: `ureq` `=3.5.1` (`rustls` with `ring`, no `aws-lc`), `serde_json` `=1.0.154` and `miniz_oxide` `=0.9.1`. `deny.toml` allows ISC, BSD-3-Clause and CDLA-Permissive-2.0 for the crates that need them.

### Changed

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

There is no released version, action or binary yet, and the command line does not post anything: the publisher exists as a library, tested against an in-memory GitHub and loopback servers, and has not talked to the real API. The mention and link forms were tested in one sandbox (decision D43); real teams, cross-repository references, notifications, annotations and the heading have not been.
