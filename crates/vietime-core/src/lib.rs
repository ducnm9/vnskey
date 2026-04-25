// SPDX-License-Identifier: GPL-3.0-or-later
//
// VietIME Suite — shared core library
//
// Provides shared types (distro detection, session type, IM framework enum,
// environment-file parsing) used by vietime-doctor, vietime-installer, and
// vietime-bench.
//
// # Design
//
// This crate is the foundation of the workspace. It MUST:
//
// * be side-effect-free (no file I/O in the parse/detect functions themselves;
//   the caller passes in already-read strings / hashmaps),
// * have zero optional dependencies that could differ between binaries,
// * keep its public API stable — every other crate reuses it.
//
// Spec ref: `spec/00-vision-and-scope.md` §7, `spec/01-phase1-doctor.md` §B.2.

#![doc = "Shared core library for the VietIME Suite tools."]

pub mod desktop;
pub mod distro;
pub mod engine;
pub mod env;
pub mod im_framework;
pub mod issue;
pub mod report;
pub mod session;

pub use desktop::{detect_desktop_from_env, DesktopEnv};
pub use distro::{detect_from_os_release, Distro, DistroFamily};
pub use engine::{is_vietnamese_engine, AppFacts, AppKind, EngineFact, Fcitx5Facts, IbusFacts};
pub use env::{parse_etc_environment, EnvFacts, EnvSource, IM_ENV_KEYS};
pub use im_framework::ImFramework;
pub use issue::{Issue, Recommendation, Severity};
pub use report::{
    ActiveFramework, Anomaly, Facts, ImFacts, Report, SystemFacts, REPORT_SCHEMA_VERSION,
};
pub use session::{detect_session_from_env, SessionType};

/// Crate version, for including in Doctor reports and CLI `--version` output.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a short greeting. Temporary smoke symbol used by the workspace
/// skeleton (P0-11 acceptance); will be removed once real detectors ship in
/// P0-14. Kept `pub` to allow downstream crates to verify the build graph.
#[must_use]
pub fn hello() -> &'static str {
    "vietime-core ready"
}

#[cfg(test)]
mod smoke_tests {
    use super::*;

    #[test]
    fn hello_returns_expected_greeting() {
        assert_eq!(hello(), "vietime-core ready");
    }

    #[test]
    fn version_matches_cargo_pkg_version() {
        // Compile-time assertion masquerading as a test: if CARGO_PKG_VERSION
        // ever returned an empty string our build would be in worse shape
        // than this test could reveal, but the round-trip keeps CI honest.
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }
}
