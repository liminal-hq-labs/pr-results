#!/usr/bin/env bash
# Proves check-probe-anchors.sh passes a read-back with only the footer's links and fails on a
# leaked issue link, pull request link, short commit link, mention, an empty read-back and a
# missing section.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
work="$(mktemp -d "${TMPDIR:-/tmp}/probe-anchors.XXXXXX")"
trap 'rm -rf "$work"' EXIT

repo="example/app"
sha="3f9c2e1d4b5a69788796a5b4c3d2e1f001122334"
control='<h2 dir="auto"><a id="user-content-raw-control" class="anchor" href="#raw-control"></a>Raw control</h2><p><a class="user-mention" href="https://github.com/example">@example</a> <a class="issue-link" href="https://github.com/example/app/issues/1">#1</a></p>'
headings='<h2 dir="auto"><a class="anchor" href="#text"></a>text</h2><p>x</p><h2><a class="anchor" href="#code_span"></a>code_span</h2><h2><a class="anchor" href="#code_block"></a>code_block</h2><h2><a class="anchor" href="#failure-layout"></a>Failure layout</h2>'
footer="<p>Commit <a href=\"https://github.com/${repo}/commit/${sha}\"><code>3f9c2e1</code></a> · <a href=\"https://github.com/${repo}/actions/runs/1\">workflow run</a> · <a href=\"https://github.com/${repo}/actions/runs/1#artifacts\">artefacts</a></p>"

fail=0
expect() { # expect pass|fail NAME HTML
	local want="$1" name="$2" html="$3" got=pass
	printf '%s' "$html" >"$work/body.html"
	if ! bash scripts/check-probe-anchors.sh "$repo" "$work/body.html" "$work" >/dev/null 2>&1; then
		got=fail
	fi
	if [ "$got" != "$want" ]; then
		echo "test-check-probe-anchors: $name: wanted $want, got $got" >&2
		fail=1
	else
		echo "test-check-probe-anchors: ok: $name"
	fi
}

expect pass "only the footer's anchors" "${control}${headings}${footer}"
expect fail "a leaked issue link" "${control}${headings}<p><a class=\"issue-link\" href=\"https://github.com/${repo}/issues/1\">#1</a></p>${footer}"
expect fail "a leaked pull request link" "${control}${headings}<p><a href=\"https://github.com/${repo}/pull/2\">!2</a></p>${footer}"
expect fail "a leaked short commit link" "${control}${headings}<p><a href=\"https://github.com/${repo}/commit/3f9c2e1\">3f9c2e1</a></p>${footer}"
expect fail "a leaked mention" "${control}${headings}<p><a class=\"user-mention\" href=\"https://github.com/example\">@example</a></p>${footer}"
expect fail "an autolink" "${control}${headings}<p><a href=\"https://example.invalid/probe\">x</a></p>${footer}"
expect fail "a link into another repository" "${control}${headings}<a href=\"https://github.com/other/app/actions/runs/1\">x</a>"
expect fail "an empty read-back" ""
expect fail "a missing section" "${control}<h2><a class=\"anchor\" href=\"#text\"></a>text</h2>${footer}"
[ "$fail" = 0 ] || exit 1
echo "test-check-probe-anchors: ok"
