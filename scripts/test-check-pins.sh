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
	if (cd "$work/repo" && bash scripts/check-pins.sh --offline) >"$work/out" 2>&1; then
		echo "test-check-pins: FAIL: '$name' was accepted" >&2
		failures=$((failures + 1))
	else
		echo "test-check-pins: ok: '$name' is rejected"
	fi
}

fresh
if ! (cd "$work/repo" && bash scripts/check-pins.sh --offline) >"$work/out" 2>&1; then
	cat "$work/out" >&2
	echo "test-check-pins: the unmodified files are rejected" >&2
	exit 1
fi

mutation 'sha256sum -c replaced by true (action)' action.yml 's/| sha256sum -c -$/| true/'
mutation 'sha256sum -c replaced by true (workflow)' .github/workflows/publish.yml 's/| sha256sum -c -$/| true/'
mutation 'aarch64 pin deleted from the action' action.yml '/PR_RESULTS_SHA256_AARCH64:/d'
mutation 'tag pin deleted from the workflow' .github/workflows/publish.yml '/PR_RESULTS_TAG:/d'
mutation 'digest regex check removed' action.yml 's/\^\[0-9a-f\]{64}\$/^.*$/'
mutation 'https-only removed from curl' .github/workflows/publish.yml "s/--proto '=https' //"
mutation 'tls floor removed from curl' action.yml 's/ --tlsv1.2//'
mutation 'placeholder refusal message removed' action.yml 's/carries no SHA-256 for/has nothing for/'
mutation 'drift in one copy only' .github/workflows/publish.yml 's/chmod 0755/chmod 0777/'
mutation 'fixed download directory' action.yml 's|mktemp -d "$RUNNER_TEMP/pr-results.XXXXXX"|echo "$RUNNER_TEMP/pr-results"|'
mutation 'unpinned action' .github/workflows/ci.yml '0,/actions\/checkout@[0-9a-f]\{40\} # v7.0.1/s//actions\/checkout@v7/'
mutation 'digest not hex' action.yml '0,/PR_RESULTS_SHA256_X86_64: PLACEHOLDER/s//PR_RESULTS_SHA256_X86_64: nothing/'
mutation 'tag differs between the copies' .github/workflows/publish.yml 's/PR_RESULTS_TAG: v0.1.0/PR_RESULTS_TAG: v0.1.1/'

mutation 'token set before the build step' action.yml 's/^\(        PR_RESULTS_RUST: 1.98.0\)$/\1\n        GITHUB_TOKEN: ${{ inputs.token }}/'
mutation 'token in the dogfood job environment' .github/workflows/publish-test-results.yml 's/^    timeout-minutes: 30$/    timeout-minutes: 30\n    env:\n      GITHUB_TOKEN: ${{ github.token }}/'

if [[ "$failures" != 0 ]]; then
	exit 1
fi
echo "test-check-pins: ok"
