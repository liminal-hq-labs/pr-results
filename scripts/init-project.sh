#!/usr/bin/env bash
# Fills in the {{PROJECT_*}} placeholders of a repository created from labs-template.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

usage() {
	cat >&2 <<'EOF'
Usage: scripts/init-project.sh [--title "Display Name"] NAME "One-line description"

  NAME         repository name in liminal-hq-labs, lower case with dashes (for example: mailslot)
  description  one line, no trailing full stop (for example: "A private drop slot for files sent your way")
  --title      display name; defaults to NAME with each dash-separated word capitalized

Replaces {{PROJECT_NAME}}, {{PROJECT_TITLE}} and {{PROJECT_DESCRIPTION}} in every tracked text
file except the licences, this script and docs/new-project.md, and removes the template note
from README.md. Run it once, review the diff, then delete this script.
EOF
	exit 2
}

title=""
args=()
while [ "$#" -gt 0 ]; do
	case "$1" in
	--title)
		[ "$#" -ge 2 ] || usage
		title="$2"
		shift
		;;
	-h | --help) usage ;;
	*) args+=("$1") ;;
	esac
	shift
done
[ "${#args[@]}" -eq 2 ] || usage

name="${args[0]}"
description="${args[1]%.}"

if ! [[ "$name" =~ ^[a-z0-9][a-z0-9-]*$ ]]; then
	echo "init-project: NAME must be lower case letters, digits and dashes: $name" >&2
	exit 2
fi
if [ -z "$title" ]; then
	IFS='-' read -r -a words <<<"$name"
	title="${words[*]^}"
fi
for value in "$title" "$description"; do
	if [ -z "$value" ] || [[ "$value" == *[\"\<\>\&\\\|$'\n']* ]]; then
		echo "init-project: the title and description must be non-empty and free of quotes, <, >, &, \\ and |" >&2
		exit 2
	fi
done

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

if ! grep -q '{{PROJECT_' README.md 2>/dev/null; then
	echo "init-project: README.md has no placeholders left; this repository looks initialized already" >&2
	exit 1
fi

files=()
while IFS= read -r f; do
	case "$f" in
	LICENSE-* | scripts/init-project.sh | docs/new-project.md) continue ;;
	esac
	[ -f "$f" ] || continue
	grep -qI '{{PROJECT_' "$f" && files+=("$f")
done < <(git ls-files --cached --others --exclude-standard)

for f in "${files[@]}"; do
	sed -i \
		-e "s|{{PROJECT_NAME}}|${name}|g" \
		-e "s|{{PROJECT_TITLE}}|${title}|g" \
		-e "s|{{PROJECT_DESCRIPTION}}|${description}|g" \
		"$f"
	echo "init-project: updated $f"
done

# Drop the template note and the blank line after it.
sed -i '/<!-- template-note:start -->/,/<!-- template-note:end -->/d' README.md
sed -i '/^# /{n;/^$/{n;/^$/d}}' README.md

leftover="$(git ls-files --cached --others --exclude-standard | grep -v -e '^scripts/init-project.sh$' -e '^docs/new-project.md$' | xargs grep -lI '{{PROJECT_' 2>/dev/null || true)"
if [ -n "$leftover" ]; then
	echo "init-project: placeholders remain in:" >&2
	printf '  %s\n' "$leftover" >&2
	exit 1
fi

cat <<EOF
init-project: done. ${title} (liminal-hq-labs/${name}): ${description}.
Next: review the diff, delete scripts/init-project.sh, and work through docs/new-project.md.
EOF
