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

### Changed

- Merging same-named suites no longer takes quadratic time: 100,000 distinct suites went from 10.6 s to 17 ms.
- `Safe` now spells out every format and default-ignorable character, including the tag block, variation selectors, the soft hyphen and the fillers, as `\u{…}`.
- A suite's `file` is stored once and shared by its cases: 200,000 cases under a 1 KiB path peak at 64 MiB instead of 273 MiB.
- `Limits` gains `max_suites` (10,000) and `max_text_bytes` (64 MiB of kept text plus a fixed overhead per piece of text, case and attempt, so it bounds the report's memory and not only its bytes of text), each refused with an error that names the limit, and `max_bytes` also applies after invalid UTF-8 is replaced.
- `Safe::text` also writes a hyphen between a letter or digit and a digit (`GH-123`) as the fullwidth U+FF0D, because a backslash escape is decoded before GitHub's reference filter.
- `Safe::text` and `Safe::log_line` write `@` and `#` as the fullwidth U+FF20 and U+FF03 instead of HTML entities, which GitHub probably decodes before its mention filters run. This is not verified against GitHub.
- `Debug` for `Untrusted` prints its size only, never the text.
- Retries past `max_attempts` are counted in `Case::attempts_omitted` instead of being dropped silently.

There is no released version, action or binary yet, and the tool does not post anything.
