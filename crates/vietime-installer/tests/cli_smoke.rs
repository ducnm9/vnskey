// SPDX-License-Identifier: GPL-3.0-or-later
//
// Integration smoke tests for the `vietime-installer` binary.
//
// Spec ref: `spec/02-phase2-installer.md` §A.4 (exit codes + subcommand list).
//
// These cover the CLI surface only; the library-level tests in `src/*.rs`
// exercise the model, planner, and pre-state detection.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> PathBuf {
    // Cargo exports `CARGO_BIN_EXE_<name>` for integration tests.
    PathBuf::from(env!("CARGO_BIN_EXE_vietime-installer"))
}

#[test]
fn help_lists_every_subcommand() {
    let out =
        Command::new(binary_path()).arg("--help").output().expect("spawn vietime-installer --help");
    assert!(out.status.success(), "--help should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for expected in [
        "install",
        "uninstall",
        "switch",
        "verify",
        "status",
        "list",
        "rollback",
        "snapshots",
        "doctor",
        "version",
        "hello",
    ] {
        assert!(stdout.contains(expected), "--help should list `{expected}`, got:\n{stdout}");
    }
    // Also verify the global flags show up.
    for flag in ["--dry-run", "--yes", "--verbose", "--log-file"] {
        assert!(stdout.contains(flag), "--help should list `{flag}`, got:\n{stdout}");
    }
}

#[test]
fn top_level_version_prints_cargo_version() {
    let out = Command::new(binary_path())
        .arg("--version")
        .output()
        .expect("spawn vietime-installer --version");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "--version should include {}, got: {stdout}",
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn version_subcommand_prints_cargo_version() {
    let out = Command::new(binary_path())
        .arg("version")
        .output()
        .expect("spawn vietime-installer version");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.trim() == env!("CARGO_PKG_VERSION"));
}

#[test]
fn hello_prints_core_banner() {
    let out =
        Command::new(binary_path()).arg("hello").output().expect("spawn vietime-installer hello");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("vietime-installer"));
    assert!(stdout.contains("vietime-core ready"));
}

#[test]
fn list_prints_the_four_mvp_combos() {
    let out =
        Command::new(binary_path()).arg("list").output().expect("spawn vietime-installer list");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for combo in ["fcitx5-bamboo", "fcitx5-unikey", "ibus-bamboo", "ibus-unikey"] {
        assert!(stdout.contains(combo), "list should mention `{combo}`, got:\n{stdout}");
    }
}

#[test]
fn install_with_unknown_combo_is_a_usage_error() {
    let out = Command::new(binary_path())
        .args(["install", "fcitx5-telex"])
        .output()
        .expect("spawn vietime-installer install fcitx5-telex");
    assert!(!out.status.success(), "bogus combo should not exit 0");
    // Explicitly assert our custom 64 (not clap's 2), to keep the spec link
    // from drifting out of sync with the code.
    assert_eq!(out.status.code(), Some(64));
}

#[test]
fn unknown_subcommand_exits_non_zero() {
    let out = Command::new(binary_path())
        .arg("not-a-real-subcommand")
        .output()
        .expect("spawn vietime-installer bogus");
    assert!(!out.status.success());
}
