#!/usr/bin/env bash
# Proves the tag check of release.yml works the way the release job runs it: in the pinned
# `ci-rust` container, as a user that does not own the checkout. Needs docker.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
src="$PWD"

# The image the release builds run in.
image="$(sed -n 's/^[[:space:]]*image:[[:space:]]*\(ghcr.io\/liminal-hq\/ci-rust@sha256:[0-9a-f]*\)$/\1/p' .github/workflows/release.yml | head -n 1)"
if [ -z "$image" ]; then
	echo "test-release-checks: no pinned ci-rust image found in release.yml" >&2
	exit 1
fi

vol="release-checks-$$"
docker volume create "$vol" >/dev/null
trap 'docker volume rm -f "$vol" >/dev/null' EXIT

# in_container USER SCRIPT...: run bash SCRIPT in the image with the volume at /w, as USER.
in_container() {
	local user="$1"
	shift
	docker run --rm --user "$user" -e HOME=/tmp -v "$vol:/w" -v "$src/scripts:/scripts:ro" \
		--entrypoint bash "$image" -c "$@"
}

# The checkout is made by root, as the runner's checkout is not owned by the container's user. HEAD is
# a commit on main, tagged v1.2.3, with a second commit on a side branch for the off-main case.
in_container 0 '
	set -euo pipefail
	cd /w
	git init -q -b main .
	git config user.name test
	git config user.email test@example.invalid
	printf "[workspace.package]\nversion = \"1.2.3\"\n" >Cargo.toml
	git add Cargo.toml
	git commit -q -m one
	git update-ref refs/remotes/origin/main HEAD
	git checkout -q -b side
	git commit -q --allow-empty -m two
	git checkout -q main
'

failures=0
out=""

# check NAME WANT_STATUS WANT_TEXT TAG BRANCH REF [SAFE]: run the script as uid 1001 at REF.
check() {
	local name="$1" want="$2" text="$3" tag="$4" branch="$5" ref="$6" status=0
	in_container 0 "git -C /w checkout -q $ref"
	out="$(in_container 1001 "cd /w && TAG=$tag DEFAULT_BRANCH=$branch GITHUB_WORKSPACE=/w bash /scripts/check-release-tag.sh" 2>&1)" || status=$?
	if { [ "$want" = pass ] && [ "$status" = 0 ]; } || { [ "$want" = fail ] && [ "$status" != 0 ] && grep -qF -- "$text" <<<"$out"; }; then
		echo "ok   $name"
	else
		echo "FAIL $name (status $status, wanted $want '$text')" >&2
		printf '     %s\n' "${out//$'\n'/$'\n     '}" >&2
		failures=$((failures + 1))
	fi
}

# The premise: without the safe.directory line, git refuses this checkout for this user.
status=0
out="$(in_container 1001 'cd /w && git merge-base --is-ancestor HEAD origin/main' 2>&1)" || status=$?
if [ "$status" = 128 ] && grep -qF 'dubious ownership' <<<"$out"; then
	echo "ok   git refuses a checkout the user does not own"
else
	echo "FAIL the premise: expected 'dubious ownership' (status $status): $out" >&2
	failures=$((failures + 1))
fi

check "a commit on main passes, despite the owner" pass '' v1.2.3 main main
check "a commit off main is reported as not on main" fail 'is not on main' v1.2.3 main side
check "a branch git cannot compare with is reported as such" fail 'Could not compare' v1.2.3 nonesuch main
check "a tag that is not a plain version fails" fail 'is not a plain version' v01.2.3 main main
check "a tag that differs from Cargo.toml fails" fail 'Cargo.toml says 1.2.3' v1.2.4 main main

if [ "$failures" != 0 ]; then
	echo "test-release-checks: $failures failure(s)" >&2
	exit 1
fi
echo "test-release-checks: all passed"
