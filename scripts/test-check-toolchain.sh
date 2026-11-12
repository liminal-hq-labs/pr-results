#!/usr/bin/env bash
# Proves check-toolchain.sh fails when it should, with `dpkg` and `dpkg-query` replaced by stubs
# that report the versions in scripts/musl-packages.txt, so it runs anywhere.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
src="$PWD"

work="$(mktemp -d "${TMPDIR:-/tmp}/check-toolchain-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/bin"
cat >"$work/bin/dpkg" <<'STUB'
#!/bin/sh
echo amd64
STUB
cat >"$work/bin/dpkg-query" <<'STUB'
#!/bin/sh
# Reports what scripts/musl-packages.txt expects, or what STUB_PACKAGES says.
if [ -n "${STUB_PACKAGES:-}" ]; then
	printf '%s\n' "$STUB_PACKAGES"
else
	awk '$1 == "amd64" { print $2 " " $3 }' "$STUB_LIST"
fi
STUB
chmod +x "$work/bin/dpkg" "$work/bin/dpkg-query"

failures=0

# run NAME WANT [VAR=VALUE...]: copy the scripts under $work/<n>/repo, add a file or set a variable,
# and check that the script exits 0 (WANT=pass) or not (WANT=fail).
n=0
run() {
	local name="$1" want="$2" got=fail
	shift 2
	n=$((n + 1))
	local top="$work/case$n"
	mkdir -p "$top/home" "$top/repo"
	cp -r "$src/scripts" "$top/repo/"
	if (
		cd "$top/repo"
		case "$name" in
		*'in the tree'*) mkdir -p .cargo && touch .cargo/config.toml ;;
		*'above the tree'*) mkdir -p "$top/.cargo" && touch "$top/.cargo/config" ;;
		*'in the cargo home'*) touch "$top/home/config.toml" ;;
		esac
		env -i PATH="$work/bin:$PATH" HOME="$top/home" CARGO_HOME="$top/home" STUB_LIST="$top/repo/scripts/musl-packages.txt" "$@" \
			bash scripts/check-toolchain.sh
	) >"$work/out" 2>&1; then
		got=pass
	fi
	if [[ "$got" == "$want" ]]; then
		echo "test-check-toolchain: ok: $name ($want)"
	else
		cat "$work/out" >&2
		echo "test-check-toolchain: FAIL: $name was a $got, wanted a $want" >&2
		failures=$((failures + 1))
	fi
}

run 'the expected packages' pass
run 'a drifted musl version' fail STUB_PACKAGES=$'musl 9.9\nmusl-dev 1.2.4-2\nmusl-tools 1.2.4-2'
run 'a missing package' fail STUB_PACKAGES=$'musl 1.2.4-2\nmusl-tools 1.2.4-2'
run 'a cargo configuration in the tree' fail
run 'a cargo configuration above the tree' fail
run 'a cargo configuration in the cargo home' fail
for var in CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_RUSTFLAGS CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS \
	CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER CARGO_PROFILE_RELEASE_OPT_LEVEL RUSTC_WRAPPER \
	RUSTC_WORKSPACE_WRAPPER CARGO_BUILD_RUSTC_WRAPPER CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER; do
	run "$var set" fail "$var=x"
done

if [[ "$failures" != 0 ]]; then
	exit 1
fi
echo "test-check-toolchain: ok"
