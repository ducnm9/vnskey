// SPDX-License-Identifier: GPL-3.0-or-later
//
// Integration smoke tests for the `vietime-bench` binary.
//
// Spec ref: `spec/03-phase3-test-suite.md` §A.4 (exit codes + subcommand list).
//
// These cover the CLI surface only; the library-level tests in `src/*.rs`
// exercise the session drivers, injectors, and input-mode parser.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> PathBuf {
    // Cargo exports `CARGO_BIN_EXE_<name>` for integration tests.
    PathBuf::from(env!("CARGO_BIN_EXE_vietime-bench"))
}

#[test]
fn help_lists_every_subcommand_and_global_flag() {
    let out =
        Command::new(binary_path()).arg("--help").output().expect("spawn vietime-bench --help");
    assert!(out.status.success(), "--help should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for expected in ["run", "list", "report", "compare", "validate", "inspect", "version", "hello"]
    {
        assert!(stdout.contains(expected), "--help should list `{expected}`, got:\n{stdout}");
    }
    for flag in ["--profile", "--engine", "--app", "--mode", "--session", "--verbose", "--runs-dir"]
    {
        assert!(stdout.contains(flag), "--help should list `{flag}`, got:\n{stdout}");
    }
}

#[test]
fn top_level_version_prints_cargo_version() {
    let out = Command::new(binary_path())
        .arg("--version")
        .output()
        .expect("spawn vietime-bench --version");
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
    let out =
        Command::new(binary_path()).arg("version").output().expect("spawn vietime-bench version");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn hello_prints_core_banner() {
    let out = Command::new(binary_path()).arg("hello").output().expect("spawn vietime-bench hello");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("vietime-bench"));
    assert!(stdout.contains("vietime-core ready"));
}

#[test]
fn list_prints_mvp_combos_sessions_and_modes() {
    let out = Command::new(binary_path()).arg("list").output().expect("spawn vietime-bench list");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for combo in ["fcitx5-bamboo", "fcitx5-unikey", "ibus-bamboo", "ibus-unikey"] {
        assert!(stdout.contains(combo), "list should mention `{combo}`, got:\n{stdout}");
    }
    for session in ["x11", "wayland"] {
        assert!(stdout.contains(session), "list should mention `{session}`, got:\n{stdout}");
    }
    for mode in ["telex", "vni", "viqr", "simple-telex"] {
        assert!(stdout.contains(mode), "list should mention `{mode}`, got:\n{stdout}");
    }
    for driver in ["xvfb", "weston", "xdotool", "ydotool"] {
        assert!(stdout.contains(driver), "list should mention `{driver}`, got:\n{stdout}");
    }
}

#[test]
fn run_with_unknown_engine_is_a_usage_error() {
    let out = Command::new(binary_path())
        .args(["run", "--engine", "fcitx5-telex"])
        .output()
        .expect("spawn vietime-bench run --engine fcitx5-telex");
    assert!(!out.status.success(), "bogus combo should not exit 0");
    // Explicitly assert our custom 64 (not clap's 2), to keep the spec link
    // from drifting out of sync with the code.
    assert_eq!(out.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("fcitx5-telex"), "stderr mentions the bad slug: {stderr}");
}

#[test]
fn unknown_subcommand_exits_non_zero() {
    let out = Command::new(binary_path())
        .arg("not-a-real-subcommand")
        .output()
        .expect("spawn vietime-bench bogus");
    assert!(!out.status.success());
}
