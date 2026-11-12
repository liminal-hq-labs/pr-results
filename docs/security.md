# Security

How `pr-results` handles untrusted input, and why it can run on fork pull requests. The rules themselves are normative in [SPEC.md](../SPEC.md#the-workflow_run-flow-and-security); this page explains the model and tells you what to check.

## The threat

A pull request from a fork runs your tests with a read-only token, so it cannot post results. To post them, a second workflow has to run with write access after the tests finish. That second workflow reads files the first one produced, and the author of the pull request controls those files. The risks that follow:

- **Injection.** A test name, message or file path that holds Markdown or HTML changes how a comment renders: an image, a link, a mention, a closed `<details>` block or a broken code fence. A comment keeps its edit history, so content that was later escaped better stays visible.
- **Spoofing.** A forged event file names someone else's pull request, or a user posts a comment carrying the tool's hidden marker so the tool edits or trusts the wrong one.
- **Resource attacks.** A zip with an entity-bomb XML file, a symlink or `..` entry, a huge inflated entry, or a hundred thousand test cases.
- **Leaking.** Runner hostnames and environment values inside report files.
- **Privilege.** Running or checking out code from the pull request in a workflow that holds a write token.

## The model

- **Two workflows.** The CI workflow is untrusted and read-only. The publisher is a `workflow_run` workflow that runs from the default branch with only the permissions listed in the specification: `actions: read`, `checks: write`, `pull-requests: write` and `contents: read`, with `permissions: {}` at workflow level.
- **No pull request code in the publisher.** It never checks out the pull request head. Its only checkout is a sparse checkout of `.github/` from the default branch, with `persist-credentials: false`. Nothing from an artefact is run, sourced or used as a path outside the temporary directory.
- **One escaping type.** Every string from an input reaches the output through `Safe`. The renderer has no function that accepts a plain string, so a missing escape is a type error rather than a review finding. Links are built only from parts the tool controls.
- **Verified pull request.** The pull request is found from the trusted event and the API, never from the uploaded event file alone.
- **Author-checked marker.** The sticky comment is found by its marker and its author, and a user's comment with a forged marker is ignored.
- **Bounded input.** DTDs are rejected, nesting is capped at 64, and archive entries are checked for absolute paths, `..` and symlinks. Total and per-entry sizes, file counts and case counts are capped by `limits`.
- **Quiet output.** Hostnames, timestamps, properties and environment values are never rendered.
- **Few secrets.** The token is `GITHUB_TOKEN` and nothing else. It goes only to the GitHub API host and the artefact download redirect, is masked defensively, and is never logged or written to disk.

## Pinning

Every action this repository uses is pinned by full commit SHA with its version in a comment, and container images are pinned by digest. A consumer that pins the composite action by SHA also pins the exact binary, because `action.yml` at that commit carries the binary's SHA-256. The publisher of this repository's own results uses the latest release pinned by SHA, never the pull request's build.

## What to check when you adopt it

- The publisher workflow in your repository lists only the four permissions above.
- It triggers on `workflow_run` and does not check out `github.event.workflow_run.head_sha` or any pull request code.
- The `uses:` line is a full commit SHA with the version in a comment.
- Your configuration file lives on the default branch.

## Reporting a problem

Report a vulnerability privately through the repository's [security advisories](https://github.com/liminal-hq-labs/pr-results/security/advisories/new), not in a public issue.

## Not yet verified

Whether HTML-entity `@` and `#` reliably suppress mentions and issue links in GitHub comments is to be tested in a sandbox before the first release. The hostile-input fixtures that exercise each risk above are part of the [test plan](../SPEC.md#fork-safety-and-hostile-input-tests).
