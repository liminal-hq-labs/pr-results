#!/usr/bin/env bash
# Checks that a release tag is a plain version, equals the version in Cargo.toml and names a commit
# on the default branch.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Run by `release.yml` at the root of the checkout, with TAG (the tag), DEFAULT_BRANCH and
# GITHUB_WORKSPACE (the checkout, as the runner sets it inside the container) set. The release jobs run in a container whose user is not the
# owner of the checkout, so git refuses the repository ("dubious ownership") until the checkout is
# marked safe; `scripts/test-release-checks.sh` proves that. A comparison that git could not make
# is reported as such and is not mistaken for a commit that is off the default branch. Either way
# the check fails closed.

: "${TAG:?TAG is not set}"
: "${DEFAULT_BRANCH:?DEFAULT_BRANCH is not set}"
: "${GITHUB_WORKSPACE:?GITHUB_WORKSPACE is not set}"

git config --global --add safe.directory "$GITHUB_WORKSPACE"

if ! [[ "$TAG" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
	echo "::error::The tag $TAG is not a plain version such as v0.1.0."
	exit 1
fi
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
if [ "v$version" != "$TAG" ]; then
	echo "::error::The tag is $TAG but Cargo.toml says $version."
	exit 1
fi
# Exit status 1 means "not an ancestor"; anything else (128 for a fatal error) is a failure to compare.
if git merge-base --is-ancestor HEAD "origin/$DEFAULT_BRANCH"; then
	:
else
	rc=$?
	if [ "$rc" = 1 ]; then
		echo "::error::The tagged commit is not on $DEFAULT_BRANCH."
	else
		echo "::error::Could not compare the tagged commit with origin/$DEFAULT_BRANCH (git exited with status $rc)."
	fi
	exit 1
fi
