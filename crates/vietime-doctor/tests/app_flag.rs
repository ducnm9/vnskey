// SPDX-License-Identifier: GPL-3.0-or-later
//
// End-to-end test for the `--app <X>` flag (DOC-33).
//
// Spawns the real `vietime-doctor` binary with `report --app vscode --json`
// and checks that the rendered report carries an `AppFacts` row tagged
// `vscode`. The host may or may not have VS Code installed — we only
// assert on facts that `GenericAppDetector` emits unconditionally (the
// `app_id`), so the test is hermetic in the sense that matters.
//
// Spec ref: `spec/01-phase1-doctor.md` §A.4 / §B.3 (DOC-33 acceptance).
//
// For the wider app detection contract see `tests/cli_smoke.rs`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_vietime-doctor"))
}

#[test]
fn report_with_app_flag_emits_app_facts_entry() {
    let out = Command::new(binary_path())
        .args(["report", "--app", "vscode", "--json"])
        .output()
        .expect("spawn vietime-doctor report --app vscode --json");

    // Exit code should still be in the normal 0..=2 range regardless of
    // whether VS Code is installed — `--app` doesn't introduce new
    // checkers in Week 4.
    let code = out.status.code().unwrap_or(70);
    assert!(
        (0..=2).contains(&code),
        "report --app vscode --json exit code should be 0..=2, got {code}"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("report --json must emit valid JSON");
    let apps = parsed["facts"]["apps"]
        .as_array()
        .expect("facts.apps must be an array when --app is passed");
    assert!(
        !apps.is_empty(),
        "facts.apps must contain at least one row when --app <X> resolves to a known profile"
    );
    // The `GenericAppDetector` always emits a row for a known app id, even
    // when the binary can't be located. DOC-33 acceptance is that
    // `facts.apps[0].app_id == "vscode"`.
    assert_eq!(
        apps[0]["app_id"], "vscode",
        "expected the first app row to be vscode, got:\n{stdout}"
    );
    // Kind hint survives through the reconcile pass. `AppKind` serializes
    // as an internally-tagged enum: `{"kind": "electron"}` (or similar), so
    // we peek at the inner `kind` string. On hosts where `code` is a shell
    // wrapper, `refine_kind` may demote Electron → Native; both are fine
    // for DOC-33 acceptance.
    let kind_tag = apps[0]["kind"]["kind"].as_str().unwrap_or("");
    assert!(
        matches!(kind_tag, "electron" | "chromium" | "app_image" | "native" | "flatpak" | "snap"),
        "unexpected AppKind tag for vscode row: `{kind_tag}`"
    );
}

#[test]
fn report_without_app_flag_leaves_apps_empty() {
    // The complement of the above: without `--app`, the app detectors
    // aren't registered, so `facts.apps` is an empty array.
    let out = Command::new(binary_path())
        .args(["report", "--json"])
        .output()
        .expect("spawn vietime-doctor report --json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("report --json must emit valid JSON");
    let apps = parsed["facts"]["apps"].as_array().expect("facts.apps must be an array");
    assert!(apps.is_empty(), "facts.apps must be empty without --app, got:\n{stdout}");
}

#[test]
fn unknown_app_id_emits_no_app_row_but_exits_cleanly() {
    // `GenericAppDetector` treats unknown ids as notes-only; no `AppFacts`
    // row is produced, and the exit code stays in the normal range.
    let out = Command::new(binary_path())
        .args(["report", "--app", "not-a-real-app-xyz", "--json"])
        .output()
        .expect("spawn vietime-doctor report --app not-a-real-app-xyz --json");
    let code = out.status.code().unwrap_or(70);
    assert!((0..=2).contains(&code), "unknown --app should still exit 0..=2, got {code}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("report --json must emit valid JSON");
    let apps = parsed["facts"]["apps"].as_array().expect("facts.apps must be an array");
    assert!(apps.is_empty(), "facts.apps must be empty for an unknown app id, got:\n{stdout}");
}
