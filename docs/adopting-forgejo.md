# Publishing on Forgejo

Forgejo has no `workflow_run` trigger. A final job can download earlier jobs' result artefacts into a directory with one subdirectory per artefact and run `pr-results publish --from results --forge forgejo`. Run the job after all test jobs, including when tests fail, and keep it separate from the required CI status. Set `GITHUB_TOKEN` or `FORGEJO_TOKEN` to the job token; add no other secrets. Configure suites in `.forgejo/pr-results.toml`, or pass `--config` explicitly. The action can be pinned to a release commit and receives the same arguments through `args`.

This mode accepts same-repository pull requests and pushes. A fork pull request is skipped, and `pull_request_target` is refused. Forgejo ignores workflow `permissions:`, so review the runner's token access and trust the code in jobs that have it. The publisher verifies the pull request before commenting, and writes commit statuses in place of check runs. A direct CLI probe against a disposable Forgejo instance with a personal-token bot account created and updated one comment and wrote success and failure statuses over HTTPS with a private CA. A Forgejo runner, its automatic job token and the full workflow have not been verified.

Build from source with `cargo install --git https://github.com/liminal-hq-labs/pr-results --locked pr-results` when a release binary does not fit the runner. In-job file collection requires Unix; Windows builds and paths have not been tested.

## A final results job

After a release that includes in-job publishing, pin its action to the release commit. The following steps assume earlier jobs uploaded artefacts and the final job runs even if those jobs failed. Replace `<sha>` with the full release commit SHA. The download action is Forgejo's v4.1.4; use the matching upload action in the test jobs.

```yaml
- name: Download results
  uses: https://code.forgejo.org/forgejo/download-artifact@c850b930e6ba138125429b7e5c93fc707a7f8427 # v4.1.4
  with:
    path: results
- name: Publish results
  uses: https://github.com/liminal-hq-labs/pr-results@<sha>
  with:
    args: --from results --forge forgejo
```

On a private CA, set `ca_file` in `[forge]` to a PEM file available to the job. For a deliberately trusted HTTP-only network, `[forge] allow_http = true` permits a same-authority HTTP API; the token and reports travel without encryption. Keep the publishing job free of other secrets and do not use `pull_request_target`. Actionlint checks GitHub syntax and cannot validate Forgejo's runner or full-URL action resolution.

## Installing the release binary directly

For runners that cannot use the composite action, copy the release tag and the **x86_64** binary's SHA-256 from the release into `PR_RESULTS_TAG` and `PR_RESULTS_SHA256` in the job environment. Verify the release's attestation on a trusted workstation before recording the digest. Pin both values in the workflow; update them together for each release. A mirrored binary must have the same digest. The release must include in-job publishing; v0.1.0 does not.

```yaml
- name: Install pr-results
  shell: bash
  env:
    PR_RESULTS_TAG: '<release tag>'
    PR_RESULTS_SHA256: '<release sha256>'
  run: |
    set -euo pipefail
    dir="$(mktemp -d "${RUNNER_TEMP}/pr-results.XXXXXX")"
    curl --proto '=https' --tlsv1.2 -fsSL --retry 3 \
      -o "$dir/pr-results" \
      "https://github.com/liminal-hq-labs/pr-results/releases/download/${PR_RESULTS_TAG}/pr-results-x86_64-unknown-linux-musl"
    echo "${PR_RESULTS_SHA256}  ${dir}/pr-results" | sha256sum -c -
    chmod 0755 "$dir/pr-results"
    echo "$dir" >> "$GITHUB_PATH"
```

Then run `pr-results publish --from results --forge forgejo` with the job token in its environment. The binary installation example targets Linux x86_64; use the matching asset and digest for Linux aarch64.
