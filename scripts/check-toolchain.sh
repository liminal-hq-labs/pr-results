#!/usr/bin/env bash
# Fails if anything outside the source could change the release binary's bytes unnoticed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Run after `apt-get install musl-tools` and before `scripts/build-release.sh`, in the `Reproducible
# build` job and in `release.yml`. It checks that:
#
# - the installed musl packages are the versions in `scripts/musl-packages.txt` for this
#   architecture (a different package could change the C objects of `ring`);
# - no cargo configuration file or `RUSTFLAGS`-like variable can override the flags the build
#   script sets (`CARGO_ENCODED_RUSTFLAGS`, `CARGO_BUILD_RUSTFLAGS`, the per-target forms and the
#   `CARGO_PROFILE_*` overrides).
#
# It prints the package versions to the step summary when there is one.

cd "$(dirname "${BASH_SOURCE[0]}")/.."

failed=0
bad() {
	echo "::error::toolchain: $*"
	failed=1
}

arch="$(dpkg --print-architecture)"
found="$(dpkg-query -W -f='${Package} ${Version}\n' musl musl-dev musl-tools 2>/dev/null | sort || true)"
want="$(awk -v a="$arch" '$1 == a { print $2 " " $3 }' scripts/musl-packages.txt | sort)"
if [[ -z "$want" ]]; then
	bad "scripts/musl-packages.txt has no entry for $arch"
elif [[ "$found" != "$want" ]]; then
	bad "toolchain drift: expected $(tr '\n' ';' <<<"$want"), found $(tr '\n' ';' <<<"$found"); rebuild and re-pin (docs/releasing.md)"
fi
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
	{
		echo "### Toolchain ($arch)"
		echo
		echo '```text'
		echo "$found"
		echo '```'
	} >>"$GITHUB_STEP_SUMMARY"
fi

cargo_home="${CARGO_HOME:-$HOME/.cargo}"
while IFS= read -r file; do
	[[ -n "$file" ]] && bad "cargo configuration found: $file"
done < <(find . -path ./.git -prune -o -path '*/.cargo/*' \( -name config -o -name config.toml \) -print 2>/dev/null
	for f in "$cargo_home/config" "$cargo_home/config.toml"; do [[ -e "$f" ]] && echo "$f"; done)

while IFS= read -r name; do
	[[ -n "$name" ]] && bad "$name is set and can override the build flags"
done < <(env | cut -d= -f1 | grep -E '^(CARGO_ENCODED_RUSTFLAGS|CARGO_BUILD_RUSTFLAGS|CARGO_TARGET_.*_RUSTFLAGS|CARGO_PROFILE_.*)$' || true)

if [[ "$failed" != 0 ]]; then
	exit 1
fi
echo "check-toolchain: ok ($arch: $(tr '\n' ' ' <<<"$found"))"
