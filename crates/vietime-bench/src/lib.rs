// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime_bench` — library surface for the Phase 3 compatibility matrix runner.
// Spec ref: `spec/03-phase3-test-suite.md` §B.

#![doc = "Vietnamese IME bench runner — internal library."]

pub mod app_runner;
pub mod cli;
pub mod im_driver;
pub mod injector;
pub mod model;
pub mod profile;
pub mod runner;
pub mod scoring;
pub mod session;
pub mod vector;

pub use cli::{Cli, Command};
pub use injector::{InjectorError, KeystrokeInjector, XdotoolInjector};
pub use model::{InputMode, ParseInputModeError};
pub use session::{SessionDriver, SessionError, SessionHandle, SessionType, XvfbDriver};

pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
