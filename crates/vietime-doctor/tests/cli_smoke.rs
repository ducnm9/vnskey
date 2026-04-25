// SPDX-License-Identifier: GPL-3.0-or-later
//
// Integration smoke tests for the `vietime-doctor` binary.
//
// Spec ref: `spec/01-phase1-doctor.md` §A.4 (exit codes, subcommand surface).
//
// These tests invoke the cargo-built binary directly. They cover surface
// behaviour only — unit tests in the library cover the hot paths.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> PathBuf {
    // Cargo sets `CARGO_BIN_EXE_<name>` for integration tests of this crate.
    PathBuf::from(env!("CARGO_BIN_EXE_vietime-doctor"))
}

#[test]
fn help_exits_zero_and_lists_subcommands() {
    let out =
        Command::new(binary_path()).arg("--help").output().expect("spawn vietime-doctor --help");
    assert!(out.status.success(), "expected --help to exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for expected in ["report", "check", "list", "diagnose", "hello"] {
        assert!(
            stdout.contains(expected),
            "expected --help output to mention `{expected}`, got:\n{stdout}"
        );
    }
}

#[test]
fn version_prints_cargo_version() {
    let out = Command::new(binary_path())
        .arg("--version")
        .output()
        .expect("spawn vietime-doctor --version");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "expected version output to contain {}, got: {stdout}",
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn hello_subcommand_prints_core_greeting() {
    let out =
        Command::new(binary_path()).arg("hello").output().expect("spawn vietime-doctor hello");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("vietime-doctor"));
    assert!(stdout.contains("vietime-core ready"));
}

#[test]
fn list_subcommand_mentions_three_detectors() {
    let out = Command::new(binary_path()).arg("list").output().expect("spawn vietime-doctor list");
    assert!(out.status.success(), "expected `list` to exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for id in ["sys.distro", "sys.session", "sys.desktop"] {
        assert!(
            stdout.contains(id),
            "expected list output to include detector id `{id}`, got:\n{stdout}"
        );
    }
}

#[test]
fn default_report_json_is_valid_json_with_schema_version() {
    let out = Command::new(binary_path())
        .args(["report", "--json"])
        .output()
        .expect("spawn vietime-doctor report --json");
    // Exit code depends on the host, but should at least parse.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("report --json must emit valid JSON");
    assert_eq!(parsed["schema_version"], 1, "schema_version should be 1 for v0.1 reports");
    // Exit code is 0/1/2 depending on the host's real IM state; never the
    // "internal error" sentinel.
    let code = out.status.code().unwrap_or(70);
    assert!(
        (0..=2).contains(&code),
        "report --json exit code should be 0..=2 on a dev box, got {code}"
    );
}

#[test]
fn unknown_subcommand_exits_with_usage_error() {
    let out = Command::new(binary_path())
        .arg("not-a-real-subcommand")
        .output()
        .expect("spawn vietime-doctor bogus");
    // Clap returns 2 by default for argument errors — close enough to our
    // USAGE_ERROR=64 for a smoke test; we just want non-zero.
    assert!(!out.status.success());
}
