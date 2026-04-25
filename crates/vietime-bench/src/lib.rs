// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime_bench` — library surface for the Phase 3 compatibility matrix
// runner.
//
// The binary in `src/main.rs` is a thin clap dispatcher on top of this
// library. Integration tests consume the library directly; the real
// session-driver and injector implementations live here so they can be
// unit-tested without spawning the binary.
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.

#![doc = "Vietnamese IME bench runner — internal library."]

pub mod cli;
pub mod injector;
pub mod model;
pub mod session;

pub use cli::{Cli, Command};
pub use injector::{InjectorError, KeystrokeInjector, XdotoolInjector};
pub use model::{InputMode, ParseInputModeError};
pub use session::{SessionDriver, SessionError, SessionHandle, SessionType, XvfbDriver};

/// Tool version surfaced by `--version` and written into every run manifest.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
