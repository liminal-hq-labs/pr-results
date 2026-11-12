#!/usr/bin/env bash
# Checks the rendered HTML of the mention probe's comment: after the raw control rows the only
# anchors may be a heading's own and the footer's links to this repository's commit (forty
# hexadecimal digits) and workflow run. Anything else is a mention, issue link, pull request link
# or autolink the renderer should have prevented.
#
# Usage: check-probe-anchors.sh OWNER/REPO BODY_HTML [WORKDIR]
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

if [ "$#" -lt 2 ]; then
	echo "usage: check-probe-anchors.sh OWNER/REPO BODY_HTML [WORKDIR]" >&2
	exit 2
fi
repo="$1"
body="$2"
work="${3:-.}"

if [ ! -s "$body" ]; then
	echo "check-probe-anchors: the rendered comment is empty or missing" >&2
	exit 1
fi
for heading in 'Raw control' 'text' 'code_span' 'code_block' 'Failure layout'; do
	if ! grep -qF ">${heading}</h2>" "$body"; then
		echo "check-probe-anchors: the section '${heading}' is not in the rendered comment" >&2
		exit 1
	fi
done

# From the "text" heading on, whatever the line breaks of the HTML.
awk 'BEGIN { RS = "\001" } { i = index($0, ">text</h2>"); print substr($0, i) }' "$body" >"$work/rendered.html"
test -s "$work/rendered.html"
{ grep -oE '<a [^>]*>' "$work/rendered.html" || true; } | { grep -v 'class="anchor"' || true; } >"$work/anchors.txt"

# shellcheck disable=SC2016
escaped="$(printf '%s' "$repo" | sed 's/[.[\*^$()+?{|]/\\&/g')"
allowed="href=\"https://github\\.com/${escaped}/(commit/[0-9a-f]{40}|actions/runs/[0-9]+(#artifacts)?)\""
{ grep -vE "$allowed" "$work/anchors.txt" || true; } >"$work/unexpected-anchors.txt"

echo "check-probe-anchors: $(wc -l <"$work/anchors.txt") anchors after the controls, $(wc -l <"$work/unexpected-anchors.txt") unexpected"
if [ -s "$work/unexpected-anchors.txt" ]; then
	cat "$work/unexpected-anchors.txt" >&2
	exit 1
fi
