# Releasing

How a release of pr-results is cut, how a consumer checks it and how to take one back. The design is in [SPEC.md](../SPEC.md#release) and the reasons in decisions D15, D55 and D56 of [decisions.md](decisions.md).

## How a release is cut

1. **Release pull request.** Bump the version in the workspace `Cargo.toml` and `Cargo.lock`, move `CHANGELOG.md`'s `Unreleased` section under the version, and open the pull request. Its `Release pins` job is red while `action.yml` and `.github/workflows/publish.yml` still say `PLACEHOLDER`, by design.
2. **Digests.** The `Reproducible build (x86_64)` job builds the binary twice, the second time from a copy of the tree at another path, and prints both SHA-256 digests in its log and step summary. If they differ the job fails and there is nothing to pin. Otherwise copy the digest into `PR_RESULTS_SHA256_X86_64` in `action.yml` and in `publish.yml` (the two must be equal, and `PR_RESULTS_TAG` must be `v` plus the version), and push. `bash scripts/check-pins.sh` checks all of it and passes only when both digests are real. Any later change to a crate, `Cargo.toml`, `Cargo.lock` or `scripts/build-release.sh` changes the binary, so the digest is written last.
3. **Merge** with a merge commit, as every pull request here is.
4. **Tag** the merge commit on the default branch, `v0.1.0`, and push the tag. Tags are made only when the owner asks. The tag must be a plain `vX.Y.Z` equal to the version in `Cargo.toml`.
5. **`release.yml` runs.** The build job rebuilds in the pinned `ci-rust` image and **fails if the digest differs** from `action.yml` or `publish.yml`, and then nothing is published. The release job attests the files and creates the GitHub release with `pr-results-x86_64-unknown-linux-musl`, `SHA256SUMS` and generated notes.
6. **Check the release** as a consumer would, below, then pin it: after the tag, a follow-up changes this repository's own publisher from `build: source` to the release, pinned by the commit SHA of the tag.

To build the same binary on a workstation, run the `Reproducible build` steps in the pinned image: install `musl-tools`, then `bash scripts/build-release.sh x86_64-unknown-linux-musl out`. The result must have the digest in `action.yml`.

## How to verify a release

The action and the reusable workflow check the digest of what they download before they run it. To check by hand:

```sh
gh release download v0.1.0 --repo liminal-hq-labs/pr-results --pattern 'pr-results-x86_64-unknown-linux-musl' --pattern SHA256SUMS
sha256sum -c SHA256SUMS
gh attestation verify pr-results-x86_64-unknown-linux-musl --repo liminal-hq-labs/pr-results
```

The first two commands compare the file with the digest the release published, and the digest must also be the one in `action.yml` at the commit you pin. `gh attestation verify` checks the signed build provenance: that this repository's `release.yml` built the file from the tagged commit. Attestations are available for public repositories on every current plan, GitHub Free included. Add `--signer-workflow liminal-hq-labs/pr-results/.github/workflows/release.yml` to insist on the workflow.

## How to roll back

- **A bad release that nobody should use.** Mark the GitHub release as a pre-release or delete it (`gh release delete v0.1.0 --yes`), and delete the tag only if no one has pinned it (`git push origin :refs/tags/v0.1.0`). Pins are by commit SHA, so a deleted tag does not change what a consumer who pinned the SHA runs: their digest check still passes while the release asset exists, and fails clearly once the asset is gone. Say so in the next release's notes.
- **A fix.** Cut the next patch version through the steps above. Do not move a tag: a tag that moves makes the digest in `action.yml` describe a binary that is no longer there.
- **This repository's own publisher.** Change its pin back to `build: source` (decision D17) until a good release exists.
- **If the build does not reproduce** on GitHub's runners, use the two-commit fallback (decision D15): tag, build, then a "Pin the binaries" commit that consumers pin. Record that in `decisions.md`.
