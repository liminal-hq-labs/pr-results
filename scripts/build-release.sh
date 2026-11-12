#!/usr/bin/env bash
# Builds the release binary for one target with the settings that make it reproducible.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: scripts/build-release.sh <target> <out-dir>
#
# Writes <out-dir>/pr-results-<target> and prints its SHA-256. The CI job `Reproducible build` and
# `release.yml` both call it inside the pinned `ci-rust` image, so a tag rebuilds exactly what the
# release pull request measured. Machine-specific paths (the checkout, the target directory, the
# cargo home and the toolchain) are remapped so the same source gives the same bytes wherever it
# is built, and `[profile.release]` in `Cargo.toml` strips symbols. The musl targets link their C
# (`ring`) with `musl-gcc`, which the image lacks: install the `musl-tools` package first.

if [[ $# -ne 2 ]]; then
	echo "usage: build-release.sh <target> <out-dir>" >&2
	exit 2
fi
target="$1"
out="$2"

case "$target" in
x86_64-unknown-linux-musl | aarch64-unknown-linux-musl) ;;
*)
	echo "build-release: unsupported target $target" >&2
	exit 2
	;;
esac

cd "$(dirname "${BASH_SOURCE[0]}")/.."
root="$PWD"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
sysroot="$(rustc --print sysroot)"

if ! command -v musl-gcc >/dev/null 2>&1; then
	echo "build-release: musl-gcc not found (apt-get install musl-tools)" >&2
	exit 1
fi
rustup target add "$target" >/dev/null

# A constant, not the commit's time: the tag's commit differs from the pull request's, and the
# binary must not.
export SOURCE_DATE_EPOCH=1767225600
tvar="${target//-/_}"
export "CC_${tvar}=musl-gcc"
export "CFLAGS_${tvar}=-ffile-prefix-map=$root=/src -ffile-prefix-map=$cargo_home=/cargo -ffile-prefix-map=$target_dir=/target"
export RUSTFLAGS="--remap-path-prefix=$root=/src --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$sysroot=/rust --remap-path-prefix=$target_dir=/target"

cargo build --locked --release --target "$target" -p pr-results --target-dir "$target_dir"

mkdir -p "$out"
install -m 0755 "$target_dir/$target/release/pr-results" "$out/pr-results-$target"
sha256sum "$out/pr-results-$target"
