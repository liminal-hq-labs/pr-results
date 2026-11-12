# AGENTS.md

## Table of contents

- [Project status](#project-status)
- [Spelling and writing](#spelling-and-writing)
- [Markdown formatting](#markdown-formatting)
- [Authoring voice](#authoring-voice)
- [Commit messages](#commit-messages)
- [Pull requests](#pull-requests)
- [Git workflow](#git-workflow)
- [Local tooling](#local-tooling)
- [Licence headers](#licence-headers)
- [Architecture rules](#architecture-rules)
- [Testing](#testing)
- [Documentation](#documentation)
- [Repository layout](#repository-layout)

## Project status

pr-results is a [Liminal HQ Labs](https://github.com/liminal-hq-labs) experiment: calm test and check results for pull requests: one sticky comment and a check run per suite, from JUnit and step reports. It is **experimental**; v0.1.0 is the first release (the model, `Safe`, the JUnit parsers, the renderer, the GitHub client with the publish flow, the command line, the action and the workflows are built and tested against an in-memory GitHub; none has run on GitHub before the first release), so interfaces, file formats and behaviour may change between any two commits. `SPEC.md` is the source of truth for behaviour, and `docs/decisions.md` records why. The [Labs charter](https://github.com/liminal-hq-labs/.github/blob/main/docs/labs-charter.md) covers what that status means and how a project graduates.

## Spelling and writing

**REQUIREMENT:** Use **Canadian English** in UI strings, comments, commit messages, pull request text and documentation: `colour`, `centre`, `behaviour`, `cancelled`, `licence` (noun) and `license` (verb), and `-ize` endings such as `customize`. Keep American spellings where an API, crate, CSS property or protocol requires them (`color`, `background-color`).

- Use a real em dash (`—`) in prose, never `--`. A command-line flag such as `--check` is not prose.
- Sentence case in the UI and in headings. Title Case for window titles.
- Write tight. No padding, no restating the heading.

## Markdown formatting

**REQUIREMENT:** Do not hard-wrap markdown prose. Write each paragraph or bullet as one line and let the renderer reflow it. This applies to `README.md`, `SPEC.md`, `docs/` and pull request descriptions. Commit message bodies are the one exception: hard-wrap them at roughly 72 characters. Code comments wrap normally.

## Authoring voice

**REQUIREMENT:** Ship the result, not how it was arrived at. Write comments, identifiers, docs and pull request text as the author of the artifact, for the person who meets it later.

- No "this PR", reviewer names or commit hashes in comments or docs. State the fact or the reasoning directly.
- When a comment has been edited more than once, rewrite it as one clean explanation.
- Commit messages and a pull request's `## Test plan` are the exceptions: they are the right place to record why a change happened and how it was checked.

## Commit messages

**Format:** Conventional Commits (`feat`, `fix`, `docs`, `test`, `ci`, `build`, `chore`, `refactor`, `perf`, `style`), with an optional scope. The one exception is the repository's first commit, GitHub's `Initial commit`, which records the plumbing the repository started with.

- The body explains what and why, not how. Use markdown with **bold labels** and no headings. Hard-wrap.
- Backtick every code-level reference (identifiers, paths, config keys, flags, crate names). User-facing strings use quotes.
- End with a `Co-Authored-By:` trailer for the model that wrote the change.
- Write the message to a file with a single-quoted heredoc and commit with `git commit -F <file>`, so backticks and `$()` are not expanded. Check the result with `git log -1 --pretty=fuller`.

## Pull requests

- **Title:** capitalized, imperative, about 70 characters, outcome-focused. **Never a Conventional Commit prefix** (no `feat:` or `docs:`): the prefix belongs to commit messages only.
- **Body:** `## Summary` with flat bullets that open with a **bold lead-in**, then `## Test plan` with `- [x]` or `- [ ]` items naming the commands run. Finish with what was not verified.
- **Labels:** one of `enhancement`, `bug`, `documentation`, `testing`, `ci`, `build`, `chore`, plus scope labels where useful.
- Fix a bug with a test that fails before the fix, and state exactly what was and was not run.

## Git workflow

- `main` is the default branch. Work on topic branches (`feat/`, `fix/issue-<n>-…`, `docs/`, `chore/`, `ci/`, `build/`). Never commit to `main`.
- **Merge commits only.** No squash and no rebase merges. Branches are deleted on merge.
- **Never** push, tag, publish or dispatch workflows unless asked.
- **Never** run `git add -A` or `git commit -a`, because checkouts may be shared. Stage explicit paths.
- Prefer the `gh` CLI for GitHub work.
- Planning files such as `PLAN.md` stay out of git.

## Local tooling

- **Editor settings:** `.editorconfig` is authoritative: tabs everywhere except YAML, which uses spaces.
- **Rust**, if the project uses it, is formatted with rustfmt using hard tabs (`rustfmt.toml`). Strictness comes from CI: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and the test suite.
- **Shared CI images** come from [`liminal-hq/.github`](https://github.com/liminal-hq/.github) (`ci-rust`, `ci-web`, `tauri-ci-desktop` and others). Pick the leanest tier that fits and pin it by digest. A project image belongs here only when no shared tier covers it.
- **Dependabot** is on for GitHub Actions, and for Cargo from the change that adds `Cargo.toml` (decision D12). The labels are `ci` and `build`.
- **Pins.** Pin every action by full commit SHA with the version in a comment (`uses: owner/repo@<sha> # v1.2.3`), and every container image by digest. Look the SHA up with `gh api repos/<owner>/<repo>/git/ref/tags/<tag>` (dereference an annotated tag) and never guess one. The current `ci-rust` digest is in the [recommended image pins](https://github.com/liminal-hq/.github/blob/main/docs/reference/recommended-image-pins.md).
- **Toolchain.** Rust is built in the shared `ci-rust` image. The local gate, once there is code, is `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo nextest run --workspace --locked` and `cargo deny check`.

## Licence headers

Every source, script, YAML, TOML and SVG file starts with a one-line summary, a blank comment line and the copyright block:

```rust
// One-line summary of the file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
```

Use `#` comments for shell, YAML, TOML and Dockerfiles, and an XML comment for SVG and `.ui` files. Markdown, JSON and lockfiles don't need one. `scripts/check-headers.sh` enforces it. The project is licensed `Apache-2.0 OR MIT`.

## Architecture rules

- **Untrusted input is escaped only through `Safe`.** Everything from an artefact (names, messages, paths, the event file) is untrusted. The renderer accepts only `Safe` fragments, so no raw string reaches Markdown or HTML. Do not add a second escaping path.
- **No network outside `pr-results-github`.** The core and render crates are pure and tested offline. Anything that talks to GitHub sits behind the `Forge` trait, with an in-memory fake for tests.
- **No pull request code in privileged workflows.** A workflow that holds a write token never checks out or runs code from a pull request head, and takes no secret beyond `GITHUB_TOKEN`. The publisher uses the latest release pinned by SHA, never the pull request's own build.
- **Words that differ by forge live in one place.** The renderer takes a `Target`; a fixed sentence that names check runs or GitHub's limits exists only in `wording.rs` and `check.rs`, chosen by a `match`, and a Forgejo comment never says either. GitHub's output stays byte-identical when a target is added.
- **Deterministic output.** The same inputs give byte-identical Markdown: stable ordering, no clocks beyond the run times in the inputs.
- **No unsafe code.** Each crate has `#![forbid(unsafe_code)]`.
- **SPEC first.** A change to behaviour, interfaces, settings or defaults updates `SPEC.md` in the same change.

## Testing

- `bash scripts/check-headers.sh` — SPDX headers
- `bash scripts/check-pins.sh` — the release tag and binary digests agree between `action.yml` and `publish.yml`, and every action and image is pinned
- `bash scripts/check-doc-yaml.sh` — actionlint over the workflows and steps the documents show
- `bash scripts/check-banner.sh` — hero banner rules and text fit (needs `xmllint`, `rsvg-convert`, `python3` and Chromium; `--shots DIR` writes frames to look at)
- `docker run --rm -v "$PWD:/workdir" davidanson/markdownlint-cli2:v0.24.0 "**/*.md"` — markdown
- `docker run --rm -v "$PWD:/mnt" -w /mnt koalaman/shellcheck:stable scripts/*.sh` — shell scripts
- `docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint` — workflows
- `docker run --rm -v "$PWD:/repo" -w /repo ghcr.io/zizmorcore/zizmor --offline .github` — workflow audit
- `docker run --rm -v "$PWD:/input" -w /input lycheeverse/lychee --config .lychee.toml '**/*.md'` — links

Once the crates exist, add:

- `cargo nextest run --workspace --locked` — tests
- `cargo deny check` — dependency policy
- `PR_RESULTS_BLESS=1 cargo nextest run` — rewrite the goldens in `tests/golden/`, then review the diff as Markdown. CI fails on any difference. The goldens are exact output, so markdownlint and lychee skip them. The same variable rewrites the request goldens (`crates/pr-results-forge/tests/golden/*.json`), the sequence of API calls the publisher makes against the in-memory GitHub.

State exactly what was and was not run when reporting results.

## Documentation

- Update `SPEC.md` when behaviour changes, and add or amend a `docs/decisions.md` entry when a decision is made. A change that alters behaviour without touching the doc that describes it is incomplete.
- `README.md` is for people who want to use or understand the project: short, scannable, no padding. Keep architecture and history out of it, and keep the `> **Status:**` line honest.
- `CLAUDE.md` is a summary of this file. Keep them in sync.

## Repository layout

```text
README.md SPEC.md AGENTS.md CLAUDE.md CHANGELOG.md LICENSE-APACHE LICENSE-MIT
.editorconfig .markdownlint.jsonc .lychee.toml .gitignore rustfmt.toml
assets/        animated hero banner
docs/          decisions, adopting, inputs, security and releasing, and the sandbox probe workflow
scripts/       header check, banner check and its Chromium text-fit harness
.github/       CI workflow, Dependabot and zizmor configuration, issue and pull request templates, release notes configuration
```

Built: `Cargo.toml` and `Cargo.lock`, `crates/pr-results-core/` (the model, `Safe`, the trusted values and the parsers), `crates/pr-results-render/` (the comment, check runs and annotations, commit statuses for Forgejo, and the `mention_probe` example), `crates/pr-results-forge/` (the `Forge` trait, its real, fake and dry-run forges, `Client`, the `Host` trait with its GitHub implementation, the zip reader and the publish flow, with request goldens in its `tests/golden/`), `crates/pr-results/` (the command line: `publish`, `render` and `validate-config`, and the settings file), `action.yml` (the composite action), `.github/workflows/` (CI, the reusable `publish.yml`, the dogfood `publish-test-results.yml`), `.github/pr-results.toml`, `tests/fixtures/`, `tests/golden/`, `deny.toml` and `.config/nextest.toml`. `scripts/build-release.sh` (the reproducible musl build), `.github/workflows/release.yml` (the tag workflow) and `docs/releasing.md` carry the release.
