#!/usr/bin/env bash
# Proves check-pins.sh fails when it should: each mutation is made in a scratch copy of the files
# it reads, and the script must reject it.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# The sed expressions hold literal `$` and `${{ }}`, so no expansion is wanted.
# shellcheck disable=SC2016
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

work="$(mktemp -d "${TMPDIR:-/tmp}/check-pins-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT

failures=0

# The flags for the mutations below. A tree whose digest is still PLACEHOLDER (the release pull
# request before its reproducible build) is judged with --unreleased, so the mutations test what
# they say; a tree with a real digest is judged strictly.
flags=(--offline)
if grep -q 'PR_RESULTS_SHA256_X86_64: PLACEHOLDER' action.yml; then
	flags+=(--unreleased)
fi

fresh() {
	rm -rf "$work/repo"
	mkdir -p "$work/repo"
	cp -r Cargo.toml action.yml scripts .github "$work/repo/"
}

# mutation NAME FILE SED-EXPRESSION...: the script must fail after the edit.
mutation() {
	local name="$1" file="$2"
	shift 2
	fresh
	sed -i "$@" "$work/repo/$file"
	if cmp -s "$file" "$work/repo/$file"; then
		echo "test-check-pins: '$name' changed nothing: the test is stale" >&2
		failures=$((failures + 1))
		return
	fi
	if (cd "$work/repo" && bash scripts/check-pins.sh "${flags[@]}") >"$work/out" 2>&1; then
		echo "test-check-pins: FAIL: '$name' was accepted" >&2
		failures=$((failures + 1))
	else
		echo "test-check-pins: ok: '$name' is rejected"
	fi
}

fresh
if ! (cd "$work/repo" && bash scripts/check-pins.sh "${flags[@]}") >"$work/out" 2>&1; then
	cat "$work/out" >&2
	echo "test-check-pins: the unmodified files are rejected" >&2
	exit 1
fi

mutation 'sha256sum -c replaced by true (action)' action.yml 's/| sha256sum -c -$/| true/'
mutation 'sha256sum -c replaced by true (workflow)' .github/workflows/publish.yml 's/| sha256sum -c -$/| true/'
mutation 'x86_64 pin deleted from the action' action.yml '/PR_RESULTS_SHA256_X86_64:/d'
mutation 'tag pin deleted from the workflow' .github/workflows/publish.yml '/PR_RESULTS_TAG:/d'
mutation 'digest regex check removed' action.yml 's/\^\[0-9a-f\]{64}\$/^.*$/'
mutation 'https-only removed from curl' .github/workflows/publish.yml "s/--proto '=https' //"
mutation 'tls floor removed from curl' action.yml 's/ --tlsv1.2//'
mutation 'placeholder refusal message removed' action.yml 's/carries no SHA-256 for/has nothing for/'
mutation 'drift in one copy only' .github/workflows/publish.yml 's/chmod 0755/chmod 0777/'
mutation 'fixed download directory' action.yml 's|mktemp -d "$RUNNER_TEMP/pr-results.XXXXXX"|echo "$RUNNER_TEMP/pr-results"|'
mutation 'unpinned action' .github/workflows/ci.yml '0,/actions\/checkout@[0-9a-f]\{40\} # v7.0.1/s//actions\/checkout@v7/'
mutation 'digest not hex' action.yml 's/PR_RESULTS_SHA256_X86_64: .*/PR_RESULTS_SHA256_X86_64: nothing/'
mutation 'tag differs between the copies' .github/workflows/publish.yml 's/PR_RESULTS_TAG: v0.1.0/PR_RESULTS_TAG: v0.1.1/'

mutation 'token set before the build step' action.yml 's/^\(        PR_RESULTS_RUST: 1.98.0\)$/\1\n        GITHUB_TOKEN: ${{ inputs.token }}/'
mutation 'token in the dogfood job environment' .github/workflows/publish-test-results.yml 's/^    timeout-minutes: 30$/    timeout-minutes: 30\n    env:\n      GITHUB_TOKEN: ${{ github.token }}/'

mutation 'tag does not match the version' action.yml 's/PR_RESULTS_TAG: v0.1.0/PR_RESULTS_TAG: v0.9.0/'

# The release modes. `verdict NAME WANT FLAGS...` edits both files with $EDIT (a sed expression)
# and checks that the script accepts (WANT=accept) or rejects (WANT=reject) the result.
real=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
verdict() {
	local name="$1" want="$2" edit="$3" got=reject
	shift 3
	fresh
	sed -i "$edit" "$work/repo/action.yml" "$work/repo/.github/workflows/publish.yml"
	if (cd "$work/repo" && bash scripts/check-pins.sh --offline "$@") >"$work/out" 2>&1; then
		got=accept
	fi
	if [[ "$got" == "$want" ]]; then
		echo "test-check-pins: ok: $name is ${want}ed"
	else
		cat "$work/out" >&2
		echo "test-check-pins: FAIL: $name was ${got}ed, not ${want}ed" >&2
		failures=$((failures + 1))
	fi
}
placeholder='s/PR_RESULTS_SHA256_X86_64: .*/PR_RESULTS_SHA256_X86_64: PLACEHOLDER/'
realdigest="s/PR_RESULTS_SHA256_X86_64: .*/PR_RESULTS_SHA256_X86_64: $real/"
badtag='s/PR_RESULTS_TAG: .*/PR_RESULTS_TAG: v9.9.9/'
nothex='s/PR_RESULTS_SHA256_X86_64: .*/PR_RESULTS_SHA256_X86_64: nothing/'
verdict 'a placeholder in a release version' reject "$placeholder"
verdict 'a placeholder with --unreleased' accept "$placeholder" --unreleased
verdict 'a real digest in a release version' accept "$realdigest"
verdict 'a real digest with --unreleased' accept "$realdigest" --unreleased
verdict 'a digest that is not hex with --unreleased' reject "$nothex" --unreleased
verdict 'a tag that is not the version with --unreleased' reject "$badtag" --unreleased

if [[ "$failures" != 0 ]]; then
	exit 1
fi
echo "test-check-pins: ok"
