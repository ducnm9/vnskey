// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime-doctor` library crate.
//
// The binary in `src/main.rs` is a thin wrapper over the functions exposed
// here. Everything worth unit-testing lives in the library so we can drive
// it from `#[test]` without spawning a subprocess.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.1.

#![doc = "Vietnamese IME diagnostic tool — internal library."]

pub mod detector;
pub mod detectors;
pub mod orchestrator;
pub mod process;
pub mod render;

pub use detector::{Detector, DetectorContext, DetectorOutput, PartialFacts};
pub use orchestrator::{run_all, Orchestrator, OrchestratorConfig};
pub use process::{
    CommandRunner, DbusProbe, SharedDbus, SharedRunner, TokioCommandRunner, ZbusProbe,
};
pub use render::{render, render_json, RenderError, RenderOptions};

/// Tool version used in every `Report` emitted by this binary.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
