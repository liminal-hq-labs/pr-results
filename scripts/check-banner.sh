#!/usr/bin/env bash
# Validates the hero banner: well-formed, self-contained, small, accessible, motion-aware, text fits.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: check-banner.sh [--static] [--shots DIR] [FILE.svg]
#
# The static pass runs xmllint and rsvg-convert and greps the banner for forbidden constructs, the
# size limit, the licence header, <title>, <desc>, role="img", viewBox, data-loop, tab indentation,
# font stacks and the prefers-color-scheme and prefers-reduced-motion rules. The render pass loads
# the banner in headless Chromium (scripts/banner-fit.py) and checks text fit, loop seams and
# reduced motion through the whole loop. Without a browser the render pass is skipped with a
# notice, unless BANNER_REQUIRE_RENDER=1. --shots DIR also writes review frames as PNG files.

cd "$(dirname "${BASH_SOURCE[0]}")/.."

banner=assets/hero.svg
static_only=0
shots=()
while [ "$#" -gt 0 ]; do
	case "$1" in
	--static) static_only=1 ;;
	--shots)
		shots=(--shots "$2")
		shift
		;;
	*) banner="$1" ;;
	esac
	shift
done

max_bytes=61440
fail=0

err() {
	echo "check-banner: FAIL: $*" >&2
	fail=1
}

[ -f "$banner" ] || {
	echo "check-banner: $banner not found" >&2
	exit 1
}

xmllint --noout "$banner" || err "not well-formed XML"

if grep -nE '<script|foreignObject|<iframe|<image|<use[^>]+href="[^#]|href="https?:|@import|@font-face|url\(\s*["'"'"']?(https?:|data:|//)' "$banner"; then
	err "forbidden construct (script, foreignObject, image, external reference, @import or web font)"
fi
if grep -nE '\son[a-z]+="' "$banner"; then
	err "event handler attribute"
fi

size="$(wc -c <"$banner")"
[ "$size" -le "$max_bytes" ] || err "size ${size} bytes is over ${max_bytes}"

head -n 12 "$banner" | grep -q 'SPDX-License-Identifier: Apache-2.0 OR MIT' || err "missing the SPDX licence line"
head -n 12 "$banner" | grep -q '(c) Copyright 2026 Liminal HQ, Scott Morris' || err "missing the copyright line"
grep -q '<title' "$banner" || err "missing <title>"
grep -q '<desc' "$banner" || err "missing <desc>"
grep -q 'role="img"' "$banner" || err 'missing role="img"'
grep -qE '<svg[^>]+viewBox="' "$banner" || err "missing a viewBox"
grep -qE '<svg[^>]+data-loop="[0-9.]+"' "$banner" || err "missing data-loop on the root element"
grep -q 'prefers-reduced-motion: reduce' "$banner" || err "missing prefers-reduced-motion handling"
grep -q 'prefers-color-scheme: dark' "$banner" || err "missing prefers-color-scheme handling"
if grep -nE '^ +[^ ]' "$banner"; then
	err "indents with spaces (house style is tabs)"
fi
# Every font stack ends in a generic family, so a missing font never falls back to serif.
if grep -noE 'font-family[:=] *"?[^;"}]*' "$banner" | grep -vE '(sans-serif|monospace) *$'; then
	err "a font stack does not end in sans-serif or monospace"
fi

tmp="$(mktemp --suffix=.png)"
trap 'rm -f "$tmp"' EXIT
rsvg-convert "$banner" -o "$tmp" || err "rsvg-convert could not render the banner"

[ "$fail" -eq 0 ] && echo "check-banner: ok: static checks (${size} bytes)"

if [ "$static_only" -eq 0 ]; then
	browser_found=0
	for b in "${CHROMIUM:-}" chromium chromium-browser google-chrome google-chrome-stable chrome; do
		if [ -n "$b" ] && command -v "$b" >/dev/null 2>&1; then
			browser_found=1
			break
		fi
	done
	if [ "$browser_found" -eq 1 ] && command -v python3 >/dev/null 2>&1; then
		python3 scripts/banner-fit.py "${shots[@]}" "$banner" || fail=1
	elif [ "${BANNER_REQUIRE_RENDER:-0}" = 1 ]; then
		err "the text-fit pass needs python3 and Chromium or Chrome (set CHROMIUM)"
	else
		echo "check-banner: skipped the text-fit pass: no python3 and Chromium here"
	fi
fi

exit "$fail"
