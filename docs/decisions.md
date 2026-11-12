# Decisions

Each decision is numbered and stays here even when superseded, with a note on what replaced it. Open questions move here once they are answered.

| # | Decision | Choice | Why |
|---|---|---|---|
| D1 | Home | A Liminal HQ Labs experiment in `liminal-hq-labs/pr-results`, public from the first commit | Try the idea in the open before it earns a place in Liminal HQ. Public is also what the Labs charter requires, and a private tool would complicate use from public repositories. Graduation to `liminal-hq` follows the charter's criteria |
| D2 | Licence | `Apache-2.0 OR MIT` | House licence, compatible with both ecosystems |
| D3 | Dependency updates | No Dependabot configuration until the project opts in | Labs repositories stay quiet by default. See the [Labs charter](https://github.com/liminal-hq-labs/.github/blob/main/docs/labs-charter.md#dependabot-is-opt-in). **Superseded by D12** |
| D4 | Name | `pr-results` for the repository, the command, the crates, the configuration file, the marker and the action | A plain description that is easy to find and says what the tool does |
| D5 | Language | Rust, one binary with a small, audited dependency set | The house language for tooling. Parsing untrusted XML and making the escaping rules total and testable is easier in Rust, and the same binary renders goldens locally with no network |
| D6 | Packaging | A statically linked musl binary per release, run by a thin composite action that carries the binary's SHA-256, plus a reusable workflow in this repository | Pinning the action by commit SHA also pins the exact binary, with a `build: source` fallback. A TypeScript action would ship a generated `dist/` that reviewers cannot meaningfully read, and a container action would pay a pull on every run and fail inside container jobs. Start-up figures are estimates |
| D7 | One comment, one check run per suite | A single sticky comment per pull request and comment key, and a check run per suite with annotations | Failures stay attributed and annotated while the pull request gets one summary instead of a comment per suite |
| D8 | Where it runs | A `workflow_run` publisher from the default branch | A fork's `pull_request` token is read-only. In-job publishing works only for same-repository pull requests and pushes, and the documentation says so |
| D9 | Configuration | `.github/pr-results.toml`, read from the default branch, with unknown keys an error and defaults that need no file for the usual artefact names | One small reviewed file replaces a dozen YAML inputs repeated per step, and a fork cannot change it |
| D10 | Escaping | Every untrusted string reaches the output through the `Safe` type, in one place | A published advisory about unescaped test names in another reporter's tables shows that escaping must be central, not per table |
| D11 | Pinning | Every action by full commit SHA with the version in a comment, and every container image by digest. No floating major tag for consumers either | Fixes the exact code a write-token workflow runs, and keeps diffs readable |
| D12 | Dependency updates (supersedes D3) | Dependabot on for `github-actions` weekly, and `cargo` weekly from the pull request that adds `Cargo.toml`, labels `ci` and `build` | A tool that holds write tokens should stay patched. The `cargo` entry waits for a manifest, because Dependabot reports a failed run when it finds none |
| D13 | Where the code starts | No crates, tests or `action.yml` until the code that needs them. The skeleton is documentation, the hero and the checks that exist today | Scaffolding written before the code that needs it drifts. The Rust jobs in `ci.yml` are commented, ready for the first crate |
| D14 | Where the reusable workflow lives | In this repository, versioned with the binary, not in `liminal-hq/.github` | The shared repository holds CI images and its workflow audit has an untriaged backlog. A privileged publisher there would couple its release cadence to the images |
| D15 | Release process | A release pull request records the binaries' digests in `action.yml`. A tagged build must reproduce them or fail. Tags only when the owner asks | Pinning by SHA needs the digest in the pinned commit. If builds prove not reproducible, a two-commit release is the fallback, recorded here |
| D16 | Exit status | `0` when it published, even if tests failed. Non-zero only when publishing itself failed | The check runs carry the test outcome. `--fail-on-failures` exists for local use |
| D17 | Dogfood | The tool's own publisher uses the latest release pinned by SHA, never the pull request's build | A pull request must not change the code that holds the write token |
| D18 | Approving reviews (was Q6) | `can_approve_pull_request_reviews` is `false` for this repository, already set | The workflows hold write tokens, and a workflow that can approve pull requests could approve its own changes. The reference repositories leave it on, which is not a reason to copy them |
| D19 | Crate layout (refines the component table in the specification) | `crates/pr-results-core` holds the model, `Safe` and the JUnit parsers, and `crates/pr-results` is the command line. `pr-results-render` and `pr-results-github` are added with the code that needs them | One library crate keeps the type that makes escaping total next to the model it protects, and splitting it later is cheap. The specification's separate `model` and `parse` crates added a boundary with nothing on either side of it |
| D20 | Dependencies in the first code | `quick-xml` `=0.42.0` with default features off (MIT), which brings `memchr` `2.8.3` (Unlicense OR MIT, taken as MIT). No `serde`, `serde_json`, `toml`, `clap` or logging crate yet. `deny.toml` allows MIT and Apache-2.0 only, from `crates.io` only, with no wildcards | Nothing yet reads or writes JSON or TOML, and the command line has two flags. Each crate arrives with the code that uses it, pinned exactly, and Dependabot's `cargo` ecosystem is on from this change (D12) |
| D21 | One type for untrusted text | Strings from a report are held as `Untrusted`, which has no `Display`. Output is built only from `Safe`, whose constructors are `text`, `code_block`, `log_line`, `workflow_data`, `workflow_property`, `trusted` (a `&'static str`, so a string built at run time cannot enter), `number`, `concat` and `join`. Every constructor that takes untrusted text takes a size budget | The renderer can then accept only `Safe`, and the compiler rejects a raw string. `Safe` has no `link` yet: it arrives with the renderer, where the trusted URL parts exist |
| D22 | Escaping rules for Markdown text (Q9 stays open) | `Safe::text` writes the five HTML specials as entities, `@` as `&#64;` and every `#` as `&#35;`, backslash-escapes every Markdown special in `` \`*_{}[]()+-.!\|~=$ `` wherever it appears, turns line breaks and tabs into spaces, drops NUL and other controls, spells out bidirectional, zero-width and byte-order characters as `\u{…}`, writes the second of two colons and any colon before a slash as `&#58;`, and writes leading spaces as entities | Escaping everywhere is noisier in the source than escaping where syntax could start, but it is total and needs no context from the caller. Breaking `::` keeps a name from forming a workflow command if it is ever echoed, and breaking `://` with the escaped `.` in `www.` stops autolinks. Rust paths such as `a::b` still read normally once rendered. The `@` entity is the conservative default and is **not verified** to suppress mentions: that sandbox test is still owed. Emoji shortcodes such as `:smile:` are not neutralised, which is cosmetic |
| D23 | Truncation | A budget counts characters after escaping, so a cut never lands inside an escape. A cut ends with `… (N more characters omitted)`, outside the budget, and `N` includes characters already cut at parse time. `Untrusted` remembers how many it lost | Cutting is always visible and the marker is made of digits and fixed words, so it needs no escaping |
| D24 | Parser limits and errors | The input is at most 50 MiB, nesting at most 64 elements, at most 200 000 cases, 256 attributes per element and 100 attempts per case. Names, class names and paths are capped at 1 KiB, messages at 1 KiB, bodies and captured output at 16 KiB, while streaming. A document type, an entity other than the five predefined ones and numeric references, a NUL, an encoding other than UTF-8, a second root and text outside the root are errors. Invalid UTF-8 is replaced. Errors name the problem and never quote the input. A `time` or `line` that is not plain digits counts as zero or absent instead of failing the file | The limits are the specification's. Timing is informational, and JUnit writers from other locales write commas, so failing a whole file for it would hide real results. A message that quotes the input would carry hostile text into logs |
| D25 | Reading JUnit | A failure or error wins over a skip. A case is flaky when it has no failure, error or skip and at least one failed attempt (`flakyFailure`, `flakyError`). `rerunFailure` and `rerunError` are attempts of a case that stays failed. Nested `testsuite` elements flatten into their top-level suite, whose `time` is the suite's time. A suite with no `time` sums its cases. `Report::merge_same_named` merges suites that share a name and flavour | The cheapest reading that never hides a failure, and it matches how nextest, bun and the step contract nest and retry |
| D26 | Locations | A case's file and line come from `file` and `line` attributes (a suite's `file` is inherited), then by flavour: Rust panic text for nextest, the class name or suite name for vitest, and the part of the class name before ` › ` for Playwright. A path must be relative and free of `..`, drive letters, URLs and control characters, or it is dropped. JavaScript stack frames are not read yet (specification item S2, v0.2) | A path from a report can never name a place outside the repository, and a missing location only means no annotation |
| D27 | Fixtures and CI for this code | The fixtures are written by hand from each reporter's documented format, not captured, and the nextest ones were checked against the shape nextest 0.9.151 writes for this repository's own tests. Playwright's and bun's retry output and Playwright's flaky reporting are **not verified** against real files. CI gains Format, Clippy and Tests jobs in the pinned `ci-rust` image, which carries `cargo-nextest`. The `ci` profile retries once, so flaky tests reach the JUnit. Uploading the results and the dependency policy job wait for the pinned upload action and the first release | The jobs reuse the pins the skeleton already named. Retrying once makes the tool's own JUnit exercise the `flakyFailure` path that its parsers read |

## Open questions

Each has the default the specification assumes. See [SPEC.md](../SPEC.md#open-questions).

| # | Question | Default |
|---|---|---|
| Q5 | Which product adopts it first? | A shell-step product, once its step converter lands. A product already on `workflow_run` would be a quicker first proof |
| Q8 | Should pushes to the default branch also get check runs? | Yes, with no comment |
| Q9 | Is rendering `@` as an entity acceptable in test names, or should the tool refuse to post a name containing `@`? | Render as an entity, after the sandbox test |
| Q10 | Should a shell-step product keep its step converter, or should `pr-results` read `CHECK_TIMES` directly? | Keep the converter |
| Q11 | Should the repository turn on `sha_pinning_required` and add a ruleset for `main` (pull request required, status checks, no force pushes)? | Yes for both, once the checks have run green on `main` long enough to name as required |

## To enable before launch

Private vulnerability reporting is disabled while `.github/ISSUE_TEMPLATE/config.yml` and `docs/security.md` point reporters to the repository's `security/advisories/new` page. It is to be enabled in the repository settings (Settings → Code security → Private vulnerability reporting) before launch. Until then that link does not work for outside reporters.

## What the existing options do

Read from each project's own README, licence, releases and `action.yml`, and from GitHub's documentation. This is what the design starts from, not a ranking.

| | `EnricoMi/publish-unit-test-result-action` | `dorny/test-reporter` | `mikepenz/action-junit-report` | GitHub's own (job summaries, workflow-command annotations) |
| --- | --- | --- | --- | --- |
| Licence | Apache-2.0 | MIT | Apache-2.0 | n/a |
| Runtime | Python in a Docker action, with composite variants | TypeScript on Node | TypeScript on Node | built in |
| Inputs | JUnit, NUnit, XUnit, TRX, Dart JSON, Mocha JSON | many formats, including JUnit, jest-junit, Mocha JSON, Go JSON and dotnet TRX | JUnit XML | anything a script writes |
| Pull request comment | One per `check_name`, updated in place, with counts and deltas | None; check runs and job summary only | Optional, updated in place | None |
| Check run | One per `check_name` | One per `name` | Yes, with an annotate-only mode for read-only tokens | None |
| Annotations | Failing tests, with paths relative to the repository root | Capped, default 10 | Yes, with a limit | `::error file=…,line=…::` |
| Size handling | Options for large files | Documents the report cap and shrinks lists to fit | Stack trace truncation | 1 MiB per step summary, at most 20 shown per job |
| Fork and Dependabot | Documented `workflow_run` recipe | Documented `workflow_run` recipe | `workflow_run` with `commit`, `run_id`, `pr_id` | n/a |
| Base-branch comparison | Yes, through the earlier commit's check-run annotations | No | No | No |
| Flaky | Counts reruns | No | A `flaky_summary` table | No |

GitHub's limits for check runs: `output.summary` and `output.text` are each at most 65 535 characters, a request carries at most 50 annotations, and the `GITHUB_TOKEN` acts as the `github-actions` app, which is how these reporters create check runs. A check run created that way attaches to a check suite of GitHub's choosing, so it can appear under an unexpected workflow in the UI.

### What current reporting taught

- **Two patterns, both partial.** Reporting in the test job is simple and fails on forks and Dependabot, where the token is read-only. The `workflow_run` publisher is correct and is copied from repository to repository, so a fix lands in one copy.
- **One comment per suite** adds up quickly: a pull request with four suites gets four comments, and a job-status comment can come on top.
- **Artefact names are already consistent** across the products this tool is for: `rust-test-results`, `frontend-test-results` or `js-test-results`, `language-test-results`, `mcp-test-results`, and an `event-file` artefact holding the event. Defaulting to them means adoption needs no CI change beyond the publisher.
- **JUnit is everywhere except shell-step projects**, which need a converter or a step format of their own.
- **Where a poster builds its own Markdown** the usual gaps are unescaped test names, code fences that a failure message can break, trusting a pull request number from an artefact a fork controls, and finding its own comment by prefix with no author check. Each has a rule in the specification.
- **Pins are usually tags.** Pinning a reporter by a moving tag trusts the tag not to be retargeted, so this repository pins by full SHA and its documentation models it.
- **JUnit files carry the runner's hostname.** It is never rendered.

### What this tool does differently

- One comment per pull request and a check run per suite.
- A house layout: calm, text-first, Canadian English, no emoji wall, collapsed detail, failures first, passing suites in one line each.
- Flexibility per product: custom check names, suites mapped from artefact names, shell-step results as first-class suites, an optional combined check, and links to the run, its artefacts and the job summary.
- Configuration in a small file in each repository, read from the default branch.
- Security by construction: every untrusted string escaped by one type, verified pull request numbers, author-checked markers, strict size budgets, no secrets and minimal permissions.
- Comparison against the base branch that does not depend on check-run annotations, which break when check names change.

## Risks

- **Graduation and pins.** Transferring the repository to `liminal-hq` changes the `uses:` path. Whether Actions follows the redirect for a transferred repository was not verified, so plan to update consumers' pins in the same wave.
- **Check-run placement.** Check runs from `GITHUB_TOKEN` can appear under the publisher workflow in the UI. This is cosmetic and shared by every option.
- **Artefact retention.** Baselines come from base-branch artefacts, which expire (90 days by default, less if the repository sets it). A quiet repository may have no baseline, and the comment then omits the column.
- **Reproducible builds** may not hold across Rust or image updates. The fallback release process exists.
- **Comment edit history** keeps every previous version visible to anyone, including content that was later escaped better, so escaping must be right from the first release.
- **Rate limits** on large monorepos with many annotations, mitigated by batching and the per-suite cap.
- **Scope creep** towards a dashboard. The non-goals hold the line.
