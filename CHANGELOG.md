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

There is no released version, action or binary yet, and the tool does not post anything.
