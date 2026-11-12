#!/usr/bin/env bash
# Checks the pins a consumer relies on: the release tag and binary digests agree between the
# composite action and the reusable workflow, and every action and image is pinned.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# While the workspace version ends in `-dev` (or with `--unreleased`), a digest may still be the
# word PLACEHOLDER and the download fails clearly if it is used. Once the version is a release,
# every digest must be a real SHA-256 and the tag must be that version, so the release pull
# request fails this check until the digests of its reproducible build are written in.

unreleased=0
case "${1:-}" in
'') ;;
--unreleased) unreleased=1 ;;
*)
	echo "usage: check-pins.sh [--unreleased]" >&2
	exit 2
	;;
esac

cd "$(dirname "${BASH_SOURCE[0]}")/.."

failed=0
bad() {
	echo "check-pins: $*" >&2
	failed=1
}

version="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
if [[ -z "$version" ]]; then
	echo "check-pins: no version in Cargo.toml" >&2
	exit 1
fi
dev=0
[[ "$version" == *-dev || "$unreleased" == 1 ]] && dev=1
expected_tag="v${version%-dev}"

pins() {
	grep -E '^[[:space:]]*PR_RESULTS_(TAG|SHA256_[A-Z0-9_]+):[[:space:]]' "$1" | sed -E 's/^[[:space:]]+//' | sort
}

action_pins="$(pins action.yml)"
workflow_pins="$(pins .github/workflows/publish.yml)"
if [[ -z "$action_pins" ]]; then
	bad "action.yml carries no PR_RESULTS_TAG or PR_RESULTS_SHA256_* values"
fi
if [[ "$action_pins" != "$workflow_pins" ]]; then
	bad "action.yml and .github/workflows/publish.yml carry different pins:"
	diff <(echo "$action_pins") <(echo "$workflow_pins") >&2 || true
fi

while IFS= read -r line; do
	[[ -z "$line" ]] && continue
	key="${line%%:*}"
	value="${line#*: }"
	if [[ "$key" == PR_RESULTS_TAG ]]; then
		[[ "$value" == "$expected_tag" ]] || bad "PR_RESULTS_TAG is $value, but version $version means $expected_tag"
	elif [[ "$value" =~ ^[0-9a-f]{64}$ ]]; then
		:
	elif [[ "$value" == PLACEHOLDER && "$dev" == 1 ]]; then
		:
	else
		bad "$key is '$value': it must be a SHA-256 (PLACEHOLDER is allowed only while the version ends in -dev, or with --unreleased)"
	fi
done <<<"$action_pins"

# Every `uses:` is a local path or a full commit SHA with the version in a comment.
mapfile -t files < <(find .github action.yml -name '*.yml' -not -name dependabot.yml -not -name zizmor.yml -not -name config.yml)
for f in "${files[@]}"; do
	while IFS= read -r line; do
		ref="$(sed -E 's/^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]+//' <<<"$line")"
		case "$ref" in
		./*) continue ;;
		esac
		if ! [[ "$ref" =~ ^[A-Za-z0-9._/-]+@[0-9a-f]{40}[[:space:]]+#[[:space:]]+v[0-9][0-9A-Za-z.+-]*$ ]]; then
			bad "$f: 'uses: $ref' is not pinned by full commit SHA with a version comment"
		fi
	done < <(grep -E '^[[:space:]]*(-[[:space:]]+)?uses:' "$f" || true)

	# Images are pinned by digest.
	while IFS= read -r line; do
		if ! [[ "$line" =~ @sha256:[0-9a-f]{64} ]]; then
			bad "$f: an image is not pinned by digest: $line"
		fi
	done < <(grep -E '^[[:space:]]*image:|docker run' "$f" | grep -vE '^[[:space:]]*#' || true)
done

if [[ "$failed" != 0 ]]; then
	exit 1
fi
echo "check-pins: ok (version $version, tag $expected_tag)"
