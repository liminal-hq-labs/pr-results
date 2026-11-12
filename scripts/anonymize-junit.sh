#!/usr/bin/env bash
# Rewrites a JUnit file so that it keeps its shape and none of its words.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: scripts/anonymize-junit.sh (--salt TEXT | --random-salt) IN.xml OUT.xml
#
# Every word in a name, class name, path, message, stack trace and output becomes a token; the
# elements, attribute names, counts, times, line numbers and punctuation stay. The rules and the
# procedure for sharing a report are in docs/anonymizing-reports.md. Needs a Rust toolchain; the
# document shows the same command in the shared ci-rust image.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# A salt is required: the tokens are only as hard to guess as it is.
have_salt=0
for arg in "$@"; do
	case "$arg" in
	--salt | --random-salt) have_salt=1 ;;
	*) ;;
	esac
done
if [ "$have_salt" -eq 0 ]; then
	echo "anonymize-junit: give --salt TEXT (16 bytes or more) or --random-salt" >&2
	exit 2
fi

exec cargo run --quiet --locked --release -p pr-results-core --example anonymize_junit -- "$@"
