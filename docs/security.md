# Security

> **Planned.** `pr-results` posts nothing yet. Only the XML limits, the text rules and the `Safe` type below are built, in `pr-results-core`. This page states the design rules the tool must follow, so a product can prepare its CI now. Nothing here describes shipped behaviour. The rules themselves are normative in [SPEC.md](../SPEC.md#the-workflow_run-flow-and-security).

How `pr-results` must handle untrusted input, and why it is designed to run on fork pull requests. This page explains the model and says what to check.

## The threat

A pull request from a fork runs your tests with a read-only token, so it cannot post results. To post them, a second workflow has to run with write access after the tests finish. That second workflow reads files the first one produced, and the author of the pull request controls those files. The risks that follow:

- **Injection.** A test name, message or file path that holds Markdown or HTML changes how a comment renders: an image, a link, a mention, a closed `<details>` block or a broken code fence. A comment keeps its edit history, so content that was later escaped better stays visible.
- **Spoofing.** A forged event file names someone else's pull request, or a user posts a comment carrying the tool's hidden marker so the tool edits or trusts the wrong one.
- **Resource attacks.** A zip with an entity-bomb XML file, a symlink or `..` entry, a huge inflated entry, or a hundred thousand test cases.
- **Leaking.** Runner hostnames and environment values inside report files.
- **Workflow-command injection.** A test name or message written to a log, or into a `::error file=…::` annotation command, can carry `::` sequences that the runner executes as workflow commands, such as `::add-mask::`, `::set-output` or `::stop-commands::`.
- **Privilege.** Running or checking out code from the pull request in a workflow that holds a write token.

## The model

These are rules the tool must follow, not behaviour that exists today.

- **Two workflows.** The CI workflow is untrusted and read-only. The publisher is a `workflow_run` workflow that must run from the default branch with only the permissions listed in the specification: `actions: read`, `checks: write`, `pull-requests: write` and `contents: read`, with `permissions: {}` at workflow level.
- **No pull request code in the publisher.** It must never check out the pull request head. Its only checkout is a sparse checkout of `.github/` from the default branch, with `persist-credentials: false`. Nothing from an artefact may be run, sourced or used as a path outside the temporary directory.
- **One escaping type.** Every string from an input must reach the output through `Safe`. The renderer will have no function that accepts a plain string, so a missing escape is a type error rather than a review finding. Links are built only from parts the tool controls.
- **Verified pull request.** The pull request must be found from the trusted event and the API, never from the uploaded event file alone.
- **Author-checked marker.** The sticky comment is found by its marker and its author, and a user's comment with a forged marker is ignored.
- **Bounded input.** DTDs are rejected, nesting is capped at 64, and archive entries are checked for absolute paths, `..` and symlinks. Total and per-entry sizes, file counts and case counts are capped by `limits`.
- **Quiet output.** Hostnames, timestamps, properties and environment values must never be rendered.
- **No workflow-command injection.** Untrusted names and messages that reach a log or an annotation (`::error file=…::`) are escaped through the one `Safe` type: `%`, CR and LF always, and `:` and `,` as well inside annotation properties. Any untrusted log output is wrapped in `::stop-commands::<random token>` and resumed with the same token, so a `::` sequence in it is inert.
- **Few secrets.** The token is `GITHUB_TOKEN` and nothing else. It must be sent only to the GitHub API host. The `Authorization` header must never be forwarded across a cross-host redirect: the artefact download redirects to a presigned blob URL that needs no token. The token is masked defensively and is never logged or written to disk.

## Pinning

Every action this repository uses is pinned by full commit SHA with its version in a comment, and container images are pinned by digest. A consumer that pins the composite action by SHA will also pin the exact binary, because `action.yml` at that commit will carry the binary's SHA-256. The publisher of this repository's own results must use the latest release pinned by SHA, never the pull request's build.

## What to check when you adopt it

Once the tool exists, check these in your own repository:

- The publisher workflow in your repository lists only the four permissions above.
- It triggers on `workflow_run` and does not check out `github.event.workflow_run.head_sha` or any pull request code.
- The `uses:` line is a full commit SHA with the version in a comment.
- Your configuration file lives on the default branch.

## Reporting a problem

Report a vulnerability privately through the repository's [security advisories](https://github.com/liminal-hq-labs/pr-results/security/advisories/new), not in a public issue.

## Not yet verified

Whether HTML-entity `@` and `#` reliably suppress mentions and issue links in GitHub comments is to be tested in a sandbox before the first release. The hostile-input fixtures that exercise each risk above are part of the [test plan](../SPEC.md#fork-safety-and-hostile-input-tests).
