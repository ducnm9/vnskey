#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Enforce the <8 MiB budget on `vietime-doctor` release binaries.
# Spec ref: `spec/04-cross-cutting.md` §5.
#
# Usage: scripts/check-binary-size.sh        # require ./target/release/vietime-doctor
#
# Rebuild first with:  cargo build --release -p vietime-doctor --locked
#
# Exit codes:
#   0  binary exists and fits in the budget
#   1  binary is over budget
#   2  binary does not exist (didn't build?)

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

BINARY=target/release/vietime-doctor
BUDGET_BYTES=$((8 * 1024 * 1024))

if [[ ! -f "$BINARY" ]]; then
    echo "error: $BINARY not found. Run 'cargo build --release -p vietime-doctor --locked' first." >&2
    exit 2
fi

size=$(stat -f%z "$BINARY" 2>/dev/null || stat -c%s "$BINARY")
human=$(awk -v b="$size" 'BEGIN { printf "%.2f MiB", b / 1048576 }')
budget_human=$(awk -v b="$BUDGET_BYTES" 'BEGIN { printf "%.2f MiB", b / 1048576 }')

printf 'vietime-doctor release binary: %s (budget: %s)\n' "$human" "$budget_human"

if (( size > BUDGET_BYTES )); then
    printf 'FAIL: binary exceeds budget by %d bytes\n' "$((size - BUDGET_BYTES))" >&2
    exit 1
fi

printf 'OK: under budget by %d bytes\n' "$((BUDGET_BYTES - size))"
exit 0
