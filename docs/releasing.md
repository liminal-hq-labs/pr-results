# Releasing

How a release of pr-results is cut, how a consumer checks it and how to take one back. The design is in [SPEC.md](../SPEC.md#release) and the reasons in decisions D15, D55 and D56 of [decisions.md](decisions.md).

## How a release is cut

1. **Release pull request.** Bump the version in the workspace `Cargo.toml` and `Cargo.lock`, move `CHANGELOG.md`'s `Unreleased` section under the version, and open the pull request. Its `Release pins` job is red while `action.yml` and `.github/workflows/publish.yml` still say `PLACEHOLDER`, by design.
2. **Digests.** The `Reproducible build (x86_64)` and `Reproducible build (aarch64)` jobs each build their binary twice on a runner of that architecture, the second time from a copy of the tree at another path, and print both SHA-256 digests in the log and step summary. If the two differ the job fails and there is nothing to pin. Otherwise copy each digest into `PR_RESULTS_SHA256_X86_64` and `PR_RESULTS_SHA256_AARCH64` in `action.yml` and in `publish.yml` (the two files must agree, and `PR_RESULTS_TAG` must be `v` plus the version), and push. `bash scripts/check-pins.sh` checks all of it and passes only when every digest is real, and from then on the same job fails on a release pull request (a head branch named `release/*` or `chore/release-*`) and on a tag if its digest is not the pinned one. A release pull request whose head branch is not named `release/*` or `chore/release-*` gets no pin comparison until the tag run, so name it accordingly; the tag run of `release.yml` is the real gate and fails, before anything is published, if a digest is stale. On any other pull request and on `main` the pins are the last release's and differ from a changed tree by design, so the job only proves that the two builds agree, prints the digest and leaves a notice. **The digests are valid only for the exact tree they were built from.** If any file under `crates/`, `Cargo.toml`, `Cargo.lock` or `scripts/build-release.sh` changes before the tag, rebuild, confirm the two builds agree and replace both digests in both files; otherwise the job, and `release.yml` at the tag, fail by design.
3. **Merge** with a merge commit, as every pull request here is.
4. **Tag** the merge commit on the default branch, `v0.1.0`, and push the tag. Tags are made only when the owner asks. The tag must match `vX.Y.Z` with no leading zeros and equal the version in `Cargo.toml`.
5. **`release.yml` runs.** One build job per target rebuilds in the pinned `ci-rust` image on a runner of that architecture and **fails if its digest differs** from `action.yml` or `publish.yml`, and then nothing is published. The release job compares both downloaded files with the pins in `action.yml` and `publish.yml` once more, writes one `SHA256SUMS` for both binaries, attests it, and creates the GitHub release with `pr-results-x86_64-unknown-linux-musl`, `pr-results-aarch64-unknown-linux-musl`, `SHA256SUMS` and generated notes. The attestation is made before the release exists, because `gh release create` needs the files and the attestation needs only their digests, so a release step that fails afterwards leaves an orphan attestation. It is harmless: `gh attestation verify` checks a file's digest, and the attestations of a repository can be listed with `gh attestation`. The jobs run in a container whose user does not own the checkout, so `scripts/check-release-tag.sh` first marks it as a safe directory for git (otherwise git stops with "dubious ownership"); any git command added to a container job needs the same, and `scripts/test-release-checks.sh` runs the tag check as another user to prove it.
6. **Check the release** as a consumer would, below, then pin it: after the tag, a follow-up changes this repository's own publisher from `build: source` to the release, pinned by the commit SHA of the tag.

To build the same binary on a workstation of the same architecture, run the `Reproducible build` steps in the pinned image: install `musl-tools`, then `bash scripts/build-release.sh <target> out`. The result must have the digest in `action.yml`.

### When the toolchain drifts

`scripts/check-toolchain.sh` runs before every release build. It compares `rustc -V` and `cargo -V` with `scripts/rust-toolchain.txt`, and the installed `musl`, `musl-dev` and `musl-tools` with `scripts/musl-packages.txt` and fails with "toolchain drift: expected X, found Y; rebuild and re-pin" when they differ, because a different package could change the C objects of `ring` and so the binary. It also fails if a cargo configuration file exists in the tree or in the cargo home, or if a variable such as `CARGO_ENCODED_RUSTFLAGS` or `CARGO_BUILD_RUSTFLAGS` is set, since they can override the flags the build script sets. On a package drift:

1. Read the new versions from the failed job's step summary.
2. Rebuild on both architectures and check that each pair of builds agrees (the job does this once the file is updated).
3. In one pull request, update `scripts/musl-packages.txt` and both digests in `action.yml` and `publish.yml`, and merge it before the tag. If the drift appears at the tag, nothing was published: fix it by a pull request, and tag again only when the owner asks.

## How to verify a release

The action and the reusable workflow check the digest of what they download before they run it, and that pin in `action.yml` and `publish.yml` at the commit you pin is the anchor: it is checked at run time, whatever the release page says. To check by hand:

```sh
gh release download v0.1.0 --repo liminal-hq-labs/pr-results --pattern 'pr-results-*' --pattern SHA256SUMS
sha256sum -c SHA256SUMS
gh attestation verify pr-results-x86_64-unknown-linux-musl \
  --repo liminal-hq-labs/pr-results \
  --signer-workflow liminal-hq-labs/pr-results/.github/workflows/release.yml \
  --source-ref refs/tags/v0.1.0
```

The first two commands compare the files with the digests the release published, and each digest must also be the one in `action.yml` at the commit you pin. `gh attestation verify` checks the signed build provenance. Keep `--signer-workflow` and `--source-ref`: without them, any workflow in the repository, on any branch, that has `id-token` and `attestations: write` could attest a different file and the check would pass. Attestations are available for public repositories on every current plan, GitHub Free included. Repeat the `gh attestation verify` command, with the same `--signer-workflow` and `--source-ref`, for `pr-results-aarch64-unknown-linux-musl`.

## Repository settings the owner applies

These are settings of the repository on GitHub. **They are not applied by this repository's files or workflows**, and nothing here checks that they are on. They are recommended before the first tag:

- [ ] A tag ruleset for `v*` that restricts who can create, update and delete tags, with no bypass for ordinary contributors.
- [ ] Immutable releases, so a published release's assets and tag cannot be changed.
- [ ] `sha_pinning_required` in the repository's Actions permissions (decision D41).
- [ ] A ruleset on `main` after two green weeks (decision D41): a pull request required, no force push, and the checks `Format`, `Clippy`, `Tests`, `Goldens`, `Dependency policy` and `Workflow audit` required.

## How to roll back

- **A bad release that nobody should use.** Mark the GitHub release as a pre-release or delete it (`gh release delete v0.1.0 --yes`), and delete the tag only if no one has pinned it (`git push origin :refs/tags/v0.1.0`). Pins are by commit SHA, so a deleted tag does not change what a consumer who pinned the SHA runs: their digest check still passes while the release asset exists, and fails clearly once the asset is gone. Say so in the next release's notes.
- **A fix.** Cut the next patch version through the steps above. Do not move a tag: a tag that moves makes the digest in `action.yml` describe a binary that is no longer there.
- **This repository's own publisher.** Change its pin back to `build: source` (decision D17) until a good release exists.
- **If the build does not reproduce** on GitHub's runners, use the two-commit fallback (decision D15): tag, build, then a "Pin the binaries" commit that consumers pin. Record that in `decisions.md`.
