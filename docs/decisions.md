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

## Open questions

Each has the default the specification assumes. See [SPEC.md](../SPEC.md#open-questions).

| # | Question | Default |
|---|---|---|
| Q5 | Which product adopts it first? | A shell-step product, once its step converter lands. A product already on `workflow_run` would be a quicker first proof |
| Q6 | Should `can_approve_pull_request_reviews` be off, since the workflows hold write tokens? | Left to the owner. The reference repositories leave it on |
| Q8 | Should pushes to the default branch also get check runs? | Yes, with no comment |
| Q9 | Is rendering `@` as an entity acceptable in test names, or should the tool refuse to post a name containing `@`? | Render as an entity, after the sandbox test |
| Q10 | Should a shell-step product keep its step converter, or should `pr-results` read `CHECK_TIMES` directly? | Keep the converter |

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
