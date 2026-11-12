# CLAUDE.md

A summary of [AGENTS.md](AGENTS.md), which is canonical. Keep the two in sync.

## What this is

pr-results is a Liminal HQ Labs experiment: calm test and check results for pull requests: one sticky comment and a check run per suite, from JUnit and step reports. Experimental and only partly built (the model, `Safe`, the JUnit parsers, the renderer and the GitHub client with the publish flow; the command line does not post anything yet), so anything may change. Behaviour lives in `SPEC.md`, decisions in `docs/decisions.md`.

## Rules to remember

- **Canadian English.** Real em dashes. Sentence case. No hard-wrapped markdown prose. Ship the result, not the journey: no "this PR" or reviewer names in code or docs.
- **Conventional Commits.** Every commit except the first (GitHub's `Initial commit`). Never a Conventional Commit prefix in a pull request title. Body in markdown with bold labels and no headings, hard-wrapped, written to a file and committed with `git commit -F`. End with a `Co-Authored-By` trailer.
- **Git.** Topic branches, merge commits only, branches deleted on merge. Never push, tag, publish or dispatch workflows unless asked. Never `git add -A` or `git commit -a`. `PLAN.md` stays out of git.
- **Formatting.** Tabs everywhere except YAML. Rust uses rustfmt with hard tabs (`rustfmt.toml`).
- **Headers.** SPDX header on every source, script, YAML, TOML and SVG file: summary line, blank, "(c) Copyright 2026 Liminal HQ, Scott Morris", `SPDX-License-Identifier: Apache-2.0 OR MIT`.
- **CI.** One job per check. Shared images come from `liminal-hq/.github`, pinned by digest. Every action is pinned by full commit SHA with the version in a comment. Dependabot is on (GitHub Actions, and Cargo once there is a manifest).
- **Architecture.** Untrusted input is escaped only through `Safe`. No network outside `pr-results-github`. No pull request code in privileged workflows, and no secret beyond `GITHUB_TOKEN`. In the renderer, HTML lines hold only trusted literals, names go through `code_span` or `code_block`, and lines are joined by `cat`, never `Safe::concat`. SPEC first. Deterministic output. No unsafe code.
- **Docs.** Update `SPEC.md` and `docs/decisions.md` in the same change as the behaviour.

## Commands

```sh
bash scripts/check-headers.sh
bash scripts/check-banner.sh     # add --shots /tmp/shots for review frames
docker run --rm -v "$PWD:/workdir" davidanson/markdownlint-cli2:v0.24.0 "**/*.md"
# once there are crates:
cargo nextest run --workspace --locked
cargo deny check
PR_RESULTS_BLESS=1 cargo nextest run   # rewrite the goldens, then review the diff
```
