#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Validate that `vietime-doctor --json` output matches `schemas/report.v1.json`.
#
# This script is the "real" JSON Schema validator for CI. The Rust
# integration test `crates/vietime-doctor/tests/schema_validation.rs`
# asserts structural invariants but does not run a full draft-2020-12
# validator; this script bridges that gap using `check-jsonschema`
# (https://github.com/python-jsonschema/check-jsonschema), which is
# available as a PyPI package and preinstalled on many Linux CI images.
#
# Spec ref: `spec/01-phase1-doctor.md` §B.14.
#
# Usage: scripts/check-report-schema.sh [report.json]
#   If no argument is given, the script runs `cargo run -p vietime-doctor --
#   report --json` in a fresh binary and pipes the output.
#
# Exit codes:
#   0  report matches schema
#   1  validation failed
#   2  invoked incorrectly, or the validator isn't available

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SCHEMA_FILE="$REPO_ROOT/schemas/report.v1.json"

if [[ ! -f "$SCHEMA_FILE" ]]; then
    echo "error: schema file not found: $SCHEMA_FILE" >&2
    exit 2
fi

if ! command -v check-jsonschema >/dev/null 2>&1; then
    cat >&2 <<EOF
error: check-jsonschema is not installed.

Install it with one of:
  pipx install check-jsonschema
  pip install --user check-jsonschema

Then re-run this script. (The Rust test \`schema_validation.rs\` runs
independently and covers the structural invariants.)
EOF
    exit 2
fi

TMP_REPORT=""
cleanup() { [[ -n "$TMP_REPORT" && -f "$TMP_REPORT" ]] && rm -f "$TMP_REPORT"; }
trap cleanup EXIT

if [[ $# -ge 1 ]]; then
    REPORT_FILE="$1"
else
    TMP_REPORT="$(mktemp -t vietime-doctor-report.XXXXXX.json)"
    REPORT_FILE="$TMP_REPORT"
    echo "Generating a fresh report with \`cargo run -p vietime-doctor\`..."
    (cd "$REPO_ROOT" && cargo run --quiet -p vietime-doctor -- report --json) >"$REPORT_FILE"
fi

echo "Validating $REPORT_FILE against $SCHEMA_FILE..."
check-jsonschema --schemafile "$SCHEMA_FILE" "$REPORT_FILE"
echo "OK."
