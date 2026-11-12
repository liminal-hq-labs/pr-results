#!/usr/bin/env bash
# Runs the sample projects under tests/sample-projects in pinned containers and writes their real
# JUnit output to tests/fixtures/<flavour>/real-*.xml.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: scripts/gen-js-fixtures.sh [vitest] [bun] [playwright] [jest]
#
# With no argument every flavour is generated. The environment variable PR_RESULTS_CACHE names the
# directory for work copies, the npm cache and the downloaded dependencies (default
# $HOME/.cache/pr-results); the script removes the work copies when it finishes, so nothing it
# downloads stays in the repository or in /tmp. Docker needs network access for the install step
# only: every test run uses --network none, a fixed container host name and the same mount point,
# so the only things normalized afterwards are the run timestamps.
#
# The images are pinned by digest. To move a pin, look the new digest up from the registry (the
# registry's Docker-Content-Digest header for the tag, or `docker buildx imagetools inspect`),
# never guess it, and rerun the script.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
root="$PWD"

# node:22-bookworm, oven/bun:1.4.4-debian and mcr.microsoft.com/playwright:v1.65.0-noble (the
# image tag must match the @playwright/test version in the sample project).
node_image='docker.io/library/node@sha256:e13726ca604184d7a91fcacab1fc09eefd642dc28e79ac34e85540dd93b78cfb'
bun_image='docker.io/oven/bun@sha256:93d5032fe06d163427822dc1598f420363759bb18155c9cc90eb1be05a64a19e'
playwright_image='mcr.microsoft.com/playwright@sha256:9beb546099e11f52d62e505537914870e01a67e0e6b00ae41da4775324e73dbe'

fixed_timestamp='2026-01-01T00:00:00.000Z'
host='sample-host'

cache="${PR_RESULTS_CACHE:-$HOME/.cache/pr-results}"
work="$cache/gen-js-fixtures"
mkdir -p "$work/home"

# run_in IMAGE NETWORK DIR ENV... -- COMMAND...
run_in() {
	local image="$1" network="$2" dir="$3"
	shift 3
	local envs=()
	while [ "$1" != "--" ]; do
		envs+=(-e "$1")
		shift
	done
	shift
	docker run --rm --network "$network" --hostname "$host" \
		--user "$(id -u):$(id -g)" \
		-e HOME=/home/runner -e npm_config_cache=/home/runner/npm \
		-e CI=true -e NO_COLOR=1 -e FORCE_COLOR=0 \
		"${envs[@]}" \
		-v "$work/home:/home/runner" -v "$dir:/work" -w /work \
		"$image" "$@"
}

fresh_copy() {
	local flavour="$1"
	rm -rf "${work:?}/$flavour"
	mkdir -p "$work/$flavour"
	(cd "tests/sample-projects/$flavour" && tar -c --exclude=node_modules --exclude=reports .) |
		tar -x -C "$work/$flavour"
	mkdir -p "$work/$flavour/reports"
}

# finish FLAVOUR VARIANT TOOL-DESCRIPTION: normalizes a produced report and writes the fixture.
finish() {
	local flavour="$1" variant="$2" tool="$3"
	local from="$work/$flavour/reports/$variant.xml"
	local to="$root/tests/fixtures/$flavour/real-$variant.xml"
	if [ ! -s "$from" ]; then
		echo "gen-js-fixtures: $flavour/$variant produced no report" >&2
		return 1
	fi
	mkdir -p "$(dirname "$to")"
	{
		head -n 1 "$from"
		cat <<EOF
<!--
  Real output of $tool, run by scripts/gen-js-fixtures.sh on the sample project
  tests/sample-projects/$flavour (variant: $variant). Only the run timestamps are replaced
  (with $fixed_timestamp); the host name is the container's fixed $host.

  (c) Copyright 2026 Liminal HQ, Scott Morris
  SPDX-License-Identifier: Apache-2.0 OR MIT
-->
EOF
		tail -n +2 "$from" |
			sed -E "s/timestamp=\"[^\"]*\"/timestamp=\"$fixed_timestamp\"/g"
	} >"$to"
	echo "wrote tests/fixtures/$flavour/real-$variant.xml"
}

selected=("$@")
wanted() {
	local f
	[ "${#selected[@]}" -eq 0 ] && return 0
	for f in "${selected[@]}"; do [ "$f" = "$1" ] && return 0; done
	return 1
}

# A failing test makes the runner exit non-zero, which is the point of the samples.
allow_failure() {
	"$@" || true
}

gen_vitest() {
	fresh_copy vitest
	run_in "$node_image" bridge "$work/vitest" -- npm ci --ignore-scripts --no-audit --no-fund
	local ver
	ver="$(run_in "$node_image" none "$work/vitest" -- node -p "'vitest ' + require('vitest/package.json').version + ' on node ' + process.versions.node")"
	local variant
	for variant in default file projects; do
		allow_failure run_in "$node_image" none "$work/vitest" "VARIANT=$variant" -- \
			npx vitest run --no-color
		finish vitest "$variant" "$ver"
	done
}

gen_jest() {
	fresh_copy jest
	run_in "$node_image" bridge "$work/jest" -- npm ci --ignore-scripts --no-audit --no-fund
	local ver
	ver="$(run_in "$node_image" none "$work/jest" -- node -p "'jest ' + require('jest/package.json').version + ' with jest-junit ' + require('jest-junit/package.json').version + ' on node ' + process.versions.node")"
	local variant
	for variant in default file templates; do
		allow_failure run_in "$node_image" none "$work/jest" "VARIANT=$variant" -- \
			npx jest --ci
		finish jest "$variant" "$ver"
	done
}

gen_playwright() {
	fresh_copy playwright
	run_in "$playwright_image" bridge "$work/playwright" -- npm ci --ignore-scripts --no-audit --no-fund
	local ver
	ver="$(run_in "$playwright_image" none "$work/playwright" -- node -p "'Playwright Test ' + require('@playwright/test/package.json').version + ' on node ' + process.versions.node")"
	allow_failure run_in "$playwright_image" none "$work/playwright" VARIANT=default -- \
		npx playwright test
	finish playwright default "$ver"
	local shard
	for shard in 1 2; do
		allow_failure run_in "$playwright_image" none "$work/playwright" \
			"VARIANT=shard-$shard-of-2" -- npx playwright test "--shard=$shard/2"
		finish playwright "shard-$shard-of-2" "$ver"
	done
}

gen_bun() {
	fresh_copy bun
	local ver
	ver="$(run_in "$bun_image" none "$work/bun" -- bun -e "console.log('bun ' + Bun.version)")"
	allow_failure run_in "$bun_image" none "$work/bun" -- \
		bun test --reporter=junit --reporter-outfile=reports/default.xml
	finish bun default "$ver"
	allow_failure run_in "$bun_image" none "$work/bun" -- \
		bun test --retry 2 --reporter=junit --reporter-outfile=reports/retry.xml
	finish bun retry "$ver"
	allow_failure run_in "$bun_image" none "$work/bun" -- \
		bun test --rerun-each 2 --reporter=junit --reporter-outfile=reports/rerun-each.xml
	finish bun rerun-each "$ver"
}

cleanup() {
	[ -n "${PR_RESULTS_KEEP:-}" ] && return 0
	rm -rf "${work:?}/vitest" "${work:?}/jest" "${work:?}/playwright" "${work:?}/bun"
}
trap cleanup EXIT

wanted vitest && gen_vitest
wanted jest && gen_jest
wanted playwright && gen_playwright
wanted bun && gen_bun
echo "gen-js-fixtures: done"
