#!/usr/bin/env bash
# Runs actionlint over the workflows and steps that the documents show, so the YAML a reader
# copies is as good as the YAML in this repository.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# A ```yaml block holding a workflow (a top-level `on:`) is linted as it is. A block that starts
# with `- name:` or `- uses:` is a list of steps, and is put inside a one-job workflow. Any other
# block (a `permissions:` fragment, a snippet of settings) is skipped. `<full commit sha>` and
# `<sha>` are replaced with forty zeros, because the documents cannot show a real one.

cd "$(dirname "${BASH_SOURCE[0]}")/.."

work="$(mktemp -d "${TMPDIR:-/tmp}/doc-yaml.XXXXXX")"
trap 'rm -rf "$work"' EXIT
mkdir "$work/in"

zeros="0000000000000000000000000000000000000000"
count=0

while IFS= read -r doc; do
	n=0
	block=""
	inside=0
	while IFS= read -r line || [[ -n "$line" ]]; do
		if [[ "$inside" == 0 ]]; then
			if [[ "$line" =~ ^\`\`\`(yaml|yml)[[:space:]]*$ ]]; then
				inside=1
				block=""
			fi
			continue
		fi
		if [[ "$line" =~ ^\`\`\`[[:space:]]*$ ]]; then
			inside=0
			n=$((n + 1))
			body="${block//<full commit sha>/$zeros}"
			body="${body//<sha>/$zeros}"
			out="$work/in/$(echo "$doc" | tr '/.' '__')-$n.yml"
			first="$(grep -m1 -vE '^[[:space:]]*(#.*)?$' <<<"$body" || true)"
			if grep -qE '^on:' <<<"$body"; then
				printf '%s\n' "$body" >"$out"
			elif [[ "$first" =~ ^-[[:space:]]+(name|uses|run):  ]]; then
				{
					printf 'on: push\npermissions: {}\njobs:\n  snippet:\n    runs-on: ubuntu-24.04\n    steps:\n'
					printf '%s\n' "$body" | sed 's/^/      /'
				} >"$out"
			else
				continue
			fi
			count=$((count + 1))
			continue
		fi
		block+="$line"$'\n'
	done <"$doc"
done < <(find README.md SPEC.md docs -name '*.md' | sort)

if [[ "$count" == 0 ]]; then
	echo "check-doc-yaml: no workflow or step snippets found in the documents" >&2
	exit 1
fi

cd "$work/in"
files=(./*.yml)
if command -v actionlint >/dev/null 2>&1; then
	actionlint -color "${files[@]}"
else
	# actionlint 1.7.12, the same image as the CI job. The files are named here because the
	# image's entry point is not a shell and would not expand a pattern.
	docker run --rm -v "$PWD:/doc" -w /doc \
		rhysd/actionlint@sha256:b1934ee5f1c509618f2508e6eb47ee0d3520686341fec936f3b79331f9315667 \
		-color "${files[@]}"
fi
echo "check-doc-yaml: ok ($count snippets)"
