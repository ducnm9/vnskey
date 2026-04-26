#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Maintainer dry-run — full pre-release verification gate.
# Runs every check the CI matrix runs, plus the cargo-deny / schema /
# binary-size budgets that the release workflow polices. Use before
# tagging `v0.1.0` (DOC-64) so surprises don't show up on GH Actions
# after the tag push.
#
# Usage:
#     scripts/maintainer-dry-run.sh              # default: everything
#     scripts/maintainer-dry-run.sh --skip-release   # skip release build
#     scripts/maintainer-dry-run.sh --only fmt        # shorthand stage-filter
#
# Exit codes:
#     0   every stage passed
#     >0  at least one stage failed; see `FAIL:` prefix in output

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

COLOR_HEAD='\033[1;34m'
COLOR_OK='\033[32m'
COLOR_ERR='\033[31m'
COLOR_END='\033[0m'

only=''
skip_release=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --only)
            shift
            only="${1:-}"
            ;;
        --skip-release)
            skip_release=true
            ;;
        -h|--help)
            grep -E '^# ' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "error: unknown arg: $1" >&2
            exit 2
            ;;
    esac
    shift
done

fails=()

stage() {
    local name=$1
    shift
    if [[ -n "$only" && "$only" != "$name" ]]; then
        return 0
    fi
    printf '\n%b==> %s%b\n' "$COLOR_HEAD" "$name" "$COLOR_END"
    if "$@"; then
        printf '%bPASS: %s%b\n' "$COLOR_OK" "$name" "$COLOR_END"
    else
        printf '%bFAIL: %s%b\n' "$COLOR_ERR" "$name" "$COLOR_END"
        fails+=("$name")
    fi
}

stage fmt       cargo fmt --all -- --check
stage clippy    cargo clippy --workspace --all-targets --locked -- -D warnings
stage test      cargo test  --workspace --locked
stage deny      cargo deny  check
stage spdx      bash scripts/check-spdx.sh
stage schema    bash scripts/check-report-schema.sh

if ! $skip_release; then
    stage release-build bash -c 'cargo build --release -p vietime-doctor --locked'
    stage binary-size   bash scripts/check-binary-size.sh
fi

echo
if [[ ${#fails[@]} -eq 0 ]]; then
    printf '%ball stages passed.%b\n' "$COLOR_OK" "$COLOR_END"
    exit 0
fi
printf '%b%d stage(s) failed:%b\n' "$COLOR_ERR" "${#fails[@]}" "$COLOR_END"
for f in "${fails[@]}"; do
    printf '  - %s\n' "$f"
done
exit 1
