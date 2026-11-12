#!/usr/bin/env bash
# Checks the pins a consumer relies on: the release tag and binary digests agree between the
# composite action and the reusable workflow, both verify the binary the same way, and every
# action and image is pinned to what its comment says.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: check-pins.sh [--offline] [--unreleased]
#
# While the workspace version ends in `-dev`, or with `--unreleased`, a digest may still be the word
# PLACEHOLDER and the download fails clearly if it is used. Once the version is a release, every
# digest must be a real SHA-256 and the tag must be that version, so a release pull request fails
# this check until the digests of its reproducible build are written in.
#
# The last check asks GitHub (`gh api`, which needs a token in GH_TOKEN or a logged-in `gh`) what
# commit each `uses: <sha> # vX` comment names, so a comment cannot say v7.0.1 over another tag's
# commit. `--offline` skips it and says so.

cd "$(dirname "${BASH_SOURCE[0]}")/.."

offline=0
unreleased=0
for arg in "$@"; do
	case "$arg" in
	--offline) offline=1 ;;
	--unreleased) unreleased=1 ;;
	*)
		echo "usage: check-pins.sh [--offline] [--unreleased]" >&2
		exit 2
		;;
	esac
done

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

action=action.yml
workflow=.github/workflows/publish.yml

# ---- the tag and the digests ---------------------------------------------------------------

# Every key must be in both files, so deleting a target's pin from both cannot pass.
# v0.1 ships the x86_64 binary only, so there is no aarch64 key.
expected_keys=(PR_RESULTS_TAG PR_RESULTS_SHA256_X86_64)

pins() {
	grep -E '^[[:space:]]*PR_RESULTS_(TAG|SHA256_[A-Z0-9_]+):[[:space:]]' "$1" | sed -E 's/^[[:space:]]+//' | sort || true
}

action_pins="$(pins "$action")"
workflow_pins="$(pins "$workflow")"
for key in "${expected_keys[@]}"; do
	for file in "$action" "$workflow"; do
		if [[ "$(grep -cE "^[[:space:]]*${key}:[[:space:]]" "$file" || true)" != 1 ]]; then
			bad "$file must carry $key exactly once"
		fi
	done
done
if [[ "$action_pins" != "$workflow_pins" ]]; then
	bad "$action and $workflow carry different pins:"
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

# ---- the verification step -----------------------------------------------------------------

# The `run:` body of the step with id $2 in file $1, unindented.
step_body() {
	awk -v want="$2" '
		function indent(s) { match(s, /^ */); return RLENGTH }
		/^[[:space:]]*- (name|uses):/ { id = "" }
		/^[[:space:]]*id:[[:space:]]/ { id = $NF }
		capture {
			if ($0 ~ /^[[:space:]]*$/) { print ""; next }
			if (indent($0) > base) { print substr($0, base + 3); next }
			capture = 0
		}
		/^[[:space:]]*run:[[:space:]]*\|[[:space:]]*$/ && id == want { capture = 1; base = indent($0) }
	' "$1"
}

action_body="$(step_body "$action" release)"
workflow_body="$(step_body "$workflow" install)"
if [[ -z "$action_body" ]]; then bad "$action has no step with id 'release' and a run block"; fi
if [[ -z "$workflow_body" ]]; then bad "$workflow has no step with id 'install' and a run block"; fi

# What a verification step must contain, in both copies.
# These are literal text to find in the script, so no expansion is wanted.
# shellcheck disable=SC2016
required=(
	'set -euo pipefail'
	'curl --proto '\''=https'\'' --tlsv1.2 -fsSL'
	'if ! [[ "$want" =~ ^[0-9a-f]{64}$ ]]; then'
	'carries no SHA-256 for $target yet'
	'| sha256sum -c -'
	'chmod 0755'
	'mktemp -d "$RUNNER_TEMP/pr-results.XXXXXX"'
)
for pair in "$action:$action_body" "$workflow:$workflow_body"; do
	file="${pair%%:*}"
	body="${pair#*:}"
	[[ -z "$body" ]] && continue
	for needle in "${required[@]}"; do
		if ! grep -qF -- "$needle" <<<"$body"; then
			bad "$file: the install step lacks: $needle"
		fi
	done
	# The digest check must run before the file is made executable, and must not be skipped.
	check_line="$(grep -nF -- '| sha256sum -c -' <<<"$body" | head -n 1 | cut -d: -f1 || true)"
	chmod_line="$(grep -nF -- 'chmod 0755' <<<"$body" | head -n 1 | cut -d: -f1 || true)"
	if [[ -n "$check_line" && -n "$chmod_line" && "$check_line" -gt "$chmod_line" ]]; then
		bad "$file: the file is made executable before its digest is checked"
	fi
	if grep -qE '(\|\||\|\|[[:space:]]*true|\bcontinue-on-error\b)' <<<"$body"; then
		bad "$file: the install step must not ignore a failure"
	fi
done

# The two copies are the same script apart from the words of two error messages. Drift fails.
normalise() {
	sed -e 's/ Pin a release commit, or use build: source\./ Pin a release commit./' \
		-e 's/ Use build: source\.//'
}
if [[ -n "$action_body" && -n "$workflow_body" ]]; then
	if ! diff <(normalise <<<"$action_body") <(normalise <<<"$workflow_body") >/dev/null; then
		bad "the install steps of $action and $workflow differ:"
		diff <(normalise <<<"$action_body") <(normalise <<<"$workflow_body") >&2 || true
	fi
fi

# ---- the token reaches only the last step ---------------------------------------------------

# `build: source` installs a toolchain and runs a build in a job that holds a write token, so the
# token must not be in the environment of those steps: it appears once, in the final step.
token_lines="$(grep -nE '^[[:space:]]*GITHUB_TOKEN:' "$action" | cut -d: -f1 || true)"
source_line="$(grep -nE '^[[:space:]]*id: source$' "$action" | cut -d: -f1 || true)"
if [[ "$(wc -w <<<"$token_lines")" != 1 ]]; then
	bad "$action must set GITHUB_TOKEN exactly once, in the step that runs pr-results"
elif [[ -z "$source_line" || "$token_lines" -lt "$source_line" ]]; then
	bad "$action sets GITHUB_TOKEN before the toolchain install and build steps"
fi
if grep -nE '^[[:space:]]*(GITHUB_TOKEN|GH_TOKEN):' .github/workflows/publish-test-results.yml >&2; then
	bad "publish-test-results.yml must not put a token in the environment: only the action's last step gets one"
fi

# ---- every action and image is pinned ------------------------------------------------------

mapfile -t files < <(find .github "$action" -name '*.yml' -not -name dependabot.yml -not -name zizmor.yml -not -name config.yml)
declare -a pinned=()
for f in "${files[@]}"; do
	while IFS= read -r line; do
		ref="$(sed -E 's/^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]+//' <<<"$line")"
		case "$ref" in
		./*) continue ;;
		esac
		if [[ "$ref" =~ ^([A-Za-z0-9._-]+/[A-Za-z0-9._-]+)(/[A-Za-z0-9._/-]*)?@([0-9a-f]{40})[[:space:]]+#[[:space:]]+(v[0-9][0-9A-Za-z.+-]*)$ ]]; then
			pinned+=("${BASH_REMATCH[1]} ${BASH_REMATCH[4]} ${BASH_REMATCH[3]}")
		else
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

# ---- the comment names the real tag --------------------------------------------------------

# The commit a tag names: `git/ref/tags/<tag>`, following an annotated tag to its commit, else
# the `tags` listing.
tag_commit() {
	local repo="$1" tag="$2" out type sha
	if out="$(gh api "repos/$repo/git/ref/tags/$tag" --jq '.object.type + " " + .object.sha' 2>/dev/null)"; then
		type="${out%% *}"
		sha="${out#* }"
		local hops=0
		while [[ "$type" == tag && "$hops" -lt 3 ]]; do
			out="$(gh api "repos/$repo/git/tags/$sha" --jq '.object.type + " " + .object.sha')" || return 1
			type="${out%% *}"
			sha="${out#* }"
			hops=$((hops + 1))
		done
		echo "$sha"
		return 0
	fi
	gh api "repos/$repo/tags" --paginate --jq ".[] | select(.name == \"$tag\") | .commit.sha" 2>/dev/null | head -n 1
}

if [[ "$offline" == 1 ]]; then
	echo "check-pins: offline: not asking GitHub whether each '# vX' comment names the pinned commit"
elif ! command -v gh >/dev/null 2>&1; then
	bad "gh is needed to check the version comments (use --offline to skip that check)"
else
	declare -A seen=()
	checked=0
	for entry in "${pinned[@]}"; do
		[[ -n "${seen[$entry]:-}" ]] && continue
		seen[$entry]=1
		read -r repo tag sha <<<"$entry"
		real="$(tag_commit "$repo" "$tag" || true)"
		if [[ -z "$real" ]]; then
			bad "$repo has no tag $tag (or GitHub could not be reached)"
		elif [[ "$real" != "$sha" ]]; then
			bad "$repo@$sha is commented $tag, but $tag is $real"
		fi
		checked=$((checked + 1))
	done
	echo "check-pins: $checked action pins match their tags on GitHub"
fi

if [[ "$failed" != 0 ]]; then
	exit 1
fi
echo "check-pins: ok (version $version, tag $expected_tag)"
