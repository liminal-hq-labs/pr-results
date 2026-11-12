#!/usr/bin/env bash
# Rewrites a JUnit file so that it keeps its shape and none of its words.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: scripts/anonymize-junit.sh [--salt TEXT] IN.xml OUT.xml
#
# Every word in a name, class name, path, message, stack trace and output becomes a token; the
# elements, attribute names, counts, times, line numbers and punctuation stay. The rules and the
# procedure for sharing a report are in docs/anonymizing-reports.md. Needs a Rust toolchain; the
# document shows the same command in the shared ci-rust image.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

exec cargo run --quiet --locked --release -p pr-results-core --example anonymize_junit -- "$@"
