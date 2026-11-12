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
# - no cargo configuration file (in the tree, in any directory above it or in the cargo home) and no
#   variable can override the flags the build script sets or wrap the compiler and linker
#   (`CARGO_ENCODED_RUSTFLAGS`, `CARGO_BUILD_RUSTFLAGS`, the per-target forms, `CARGO_PROFILE_*`,
#   `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, their `CARGO_BUILD_*` forms and
#   `CARGO_TARGET_*_LINKER`).
#
# It prints the package versions to the step summary when there is one.

cd "$(dirname "${BASH_SOURCE[0]}")/.."

failed=0
bad() {
	echo "::error::toolchain: $*"
	failed=1
}

for tool in rustc cargo; do
	want_version="$(grep "^$tool " scripts/rust-toolchain.txt)"
	found_version="$("$tool" -V)"
	if [[ -z "$want_version" || "$found_version" != "$want_version" ]]; then
		bad "toolchain drift: expected $want_version, found $found_version; rebuild and re-pin (docs/releasing.md)"
	fi
done

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

# Cargo reads `.cargo/config.toml` in every directory above the working directory too.
dir="$PWD"
while :; do
	for name in config config.toml; do
		[[ -e "$dir/.cargo/$name" ]] && bad "cargo configuration found: $dir/.cargo/$name"
	done
	[[ "$dir" == / ]] && break
	dir="$(dirname "$dir")"
done

while IFS= read -r name; do
	[[ -n "$name" ]] && bad "$name is set and can override the build flags or wrap the toolchain"
done < <(env | cut -d= -f1 | grep -E '^(CARGO_ENCODED_RUSTFLAGS|CARGO_BUILD_RUSTFLAGS|CARGO_TARGET_.*_RUSTFLAGS|CARGO_TARGET_.*_LINKER|CARGO_PROFILE_.*|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|CARGO_BUILD_RUSTC_WRAPPER|CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER)$' || true)

if [[ "$failed" != 0 ]]; then
	exit 1
fi
echo "check-toolchain: ok ($arch: $(tr '\n' ' ' <<<"$found"))"
