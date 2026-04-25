#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Verify that every tracked .rs file starts with an SPDX header.
# Spec ref: spec/04-cross-cutting.md §3.
#
# Usage: scripts/check-spdx.sh           # check all tracked .rs files
#        scripts/check-spdx.sh --fix     # prepend the header to offenders
#
# Exit codes:
#   0  all files have the header
#   1  at least one file is missing the header (or --fix was applied)
#   2  invoked incorrectly

set -euo pipefail

HEADER='// SPDX-License-Identifier: GPL-3.0-or-later'

fix_mode=false
case "${1:-}" in
    '' )        ;;
    --fix )     fix_mode=true ;;
    -h|--help )
        grep -E '^# ' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    * )
        echo "error: unknown argument: $1" >&2
        exit 2
        ;;
esac

if command -v git >/dev/null && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    cd "$(git rev-parse --show-toplevel)"
    file_source="git"
else
    cd "$(dirname "$0")/.."
    file_source="find"
fi

# Use a temp file to collect the list, for portability with bash 3.2 on macOS
# which lacks `mapfile` / `readarray`.
file_list=$(mktemp)
trap 'rm -f "$file_list" "$file_list.missing"' EXIT

if [[ "$file_source" == "git" ]]; then
    git ls-files '*.rs' > "$file_list"
    # If nothing is staged yet (freshly initialised repo) fall back to a
    # filesystem walk so the pre-commit hook still catches unstaged files.
    if [[ ! -s "$file_list" ]]; then
        find crates -type f -name '*.rs' 2>/dev/null | sort > "$file_list"
    fi
else
    find crates -type f -name '*.rs' | sort > "$file_list"
fi

missing_count=0
total_count=0
: > "$file_list.missing"
while IFS= read -r f; do
    [[ -n "$f" ]] || continue
    [[ -f "$f" ]] || continue
    total_count=$((total_count + 1))
    first_line=$(awk 'NF { print; exit }' "$f" || true)
    if [[ "$first_line" != "$HEADER" ]]; then
        printf '%s\n' "$f" >> "$file_list.missing"
        missing_count=$((missing_count + 1))
    fi
done < "$file_list"

if [[ $missing_count -eq 0 ]]; then
    echo "SPDX check: $total_count file(s) ok."
    exit 0
fi

echo "SPDX check: $missing_count file(s) missing header:"
sed 's/^/  /' "$file_list.missing"

if $fix_mode; then
    while IFS= read -r f; do
        [[ -n "$f" ]] || continue
        tmp=$(mktemp)
        { printf '%s\n//\n' "$HEADER"; cat "$f"; } > "$tmp"
        mv "$tmp" "$f"
        echo "fixed: $f"
    done < "$file_list.missing"
    exit 1
fi

echo
echo "Hint: run \`scripts/check-spdx.sh --fix\` to auto-prepend headers."
exit 1
