// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime_installer` — library surface for the Phase 2 installer.
//
// Splitting the binary into a library + thin `main.rs` mirrors the layout of
// `vietime-doctor`. Integration tests consume the library directly
// (`plan_golden.rs`) while the CLI smoke tests exercise the binary.
//
// Spec ref: `spec/02-phase2-installer.md` §B.

pub mod model;
pub mod planner;
pub mod pre_state;

pub use model::{
    Combo, Engine, EnvFile, Goal, PackageManager, ParseComboError, Plan, PromptCondition, Step,
    VerifyCheck, PLAN_SCHEMA_VERSION,
};
pub use planner::{plan, validate_plan, PlanError};
pub use pre_state::{detect_pre_state, PreState};

/// Version string stamped into plans and surfaced by `--version`.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
