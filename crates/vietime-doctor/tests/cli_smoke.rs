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
fn list_subcommand_mentions_all_registered_detectors() {
    let out = Command::new(binary_path()).arg("list").output().expect("spawn vietime-doctor list");
    assert!(out.status.success(), "expected `list` to exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for id in [
        "sys.distro",
        "sys.session",
        "sys.locale",
        "sys.desktop",
        "env.process",
        "env.etc_environment",
        "env.home_profile",
        "env.etc_profile_d",
        "env.systemd",
        "im.ibus.daemon",
        "im.ibus.engines",
        "im.fcitx5.daemon",
        "im.fcitx5.config",
        "im.engines.packages",
        "app.generic",
        "app.electron",
    ] {
        assert!(
            stdout.contains(id),
            "expected list output to include detector id `{id}`, got:\n{stdout}"
        );
    }
    // `list` notes that app.* detectors are gated on `--app <X>` so the
    // default `report` / `check` invocation doesn't run them.
    assert!(
        stdout.contains("app.*"),
        "expected list output to mention app.* gating, got:\n{stdout}"
    );
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
fn list_subcommand_enumerates_all_15_checkers() {
    let out = Command::new(binary_path()).arg("list").output().expect("spawn vietime-doctor list");
    assert!(out.status.success(), "expected `list` to exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Phase-1 ships the full VD001-VD015 catalogue. We assert on every
    // id so adding or removing one without updating this test is a red
    // flag.
    for id in [
        "VD001", "VD002", "VD003", "VD004", "VD005", "VD006", "VD007", "VD008", "VD009", "VD010",
        "VD011", "VD012", "VD013", "VD014", "VD015",
    ] {
        assert!(
            stdout.contains(id),
            "expected list output to include checker id `{id}`, got:\n{stdout}"
        );
    }
    // Sanity-check: the heading for the checker section is present so a
    // future refactor that accidentally collapses the two lists still
    // produces a readable `list` output.
    assert!(
        stdout.contains("Checkers:"),
        "expected list output to include Checkers: heading, got:\n{stdout}"
    );
}

#[test]
fn report_without_no_redact_does_not_leak_username() {
    // We can't guarantee the host's username leaks in *every* field, but
    // the `$USER` env var is what the detector sees and what the redactor
    // is meant to scrub. Run `report` (no `--no-redact`) and assert the
    // username does not appear verbatim in the output.
    let user = std::env::var("USER").or_else(|_| std::env::var("LOGNAME")).unwrap_or_default();
    // Skip if the CI environment has a short/empty user — the redactor's
    // own 2-char safety guard would make this test flaky.
    if user.len() < 2 {
        return;
    }
    let out = Command::new(binary_path())
        .args(["report", "--json"])
        .output()
        .expect("spawn vietime-doctor report --json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains(&user), "redacted report still leaks $USER ({user}); got:\n{stdout}");
}

#[test]
fn no_redact_flag_prints_a_warning_to_stderr() {
    let out = Command::new(binary_path())
        .args(["--no-redact", "report", "--json"])
        .output()
        .expect("spawn vietime-doctor --no-redact report");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--no-redact"),
        "expected --no-redact warning on stderr, got:\n{stderr}"
    );
    // Still valid JSON.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _: serde_json::Value =
        serde_json::from_str(&stdout).expect("report --json still emits valid JSON");
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

#[test]
fn check_subcommand_emits_single_line_status() {
    let out =
        Command::new(binary_path()).arg("check").output().expect("spawn vietime-doctor check");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Must be exactly one non-empty line so CI callers can grep it
    // unambiguously.
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 1, "expected exactly one status line, got:\n{stdout}");
    let line = lines[0];
    assert!(
        line.starts_with("vietime-doctor: "),
        "status line must start with `vietime-doctor: `, got: {line}"
    );
    for needle in ["detectors", "checkers", "issues", "anomalies"] {
        assert!(line.contains(needle), "status line must mention `{needle}`, got: {line}");
    }
    // Exit code is always 0/1/2 on a dev box — never the internal-error sentinel.
    let code = out.status.code().unwrap_or(70);
    assert!((0..=2).contains(&code), "check exit code should be 0..=2 on a dev box, got {code}");
}

#[test]
fn check_subcommand_runs_within_reasonable_time() {
    // Spec target is <500ms, but CI runners (cold caches, tokio startup,
    // spawning ~14 detectors) push us closer to a second. We cap at 5s —
    // tight enough to catch a pathological regression but loose enough
    // to survive noisy CI.
    let start = std::time::Instant::now();
    let out =
        Command::new(binary_path()).arg("check").output().expect("spawn vietime-doctor check");
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "check subcommand took {elapsed:?}, want <5s"
    );
    let code = out.status.code().unwrap_or(70);
    assert!((0..=2).contains(&code), "check exit code should be 0..=2 on a dev box, got {code}");
}
