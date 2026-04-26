// SPDX-License-Identifier: GPL-3.0-or-later
//
// IM framework driver trait (BEN-10).
//
// Abstracts over IBus and Fcitx5: the bench runner calls `start()` to bring
// the daemon up inside a headless session, `activate_engine("Bamboo")` to
// switch to the Vietnamese engine, `set_mode(Telex)` to pick the typing
// layout, and `stop()` to tear it down.
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.3.

use async_trait::async_trait;

use crate::model::InputMode;
use crate::session::SessionHandle;

pub mod ibus;

pub use ibus::IbusDriver;

/// Contract every IM framework driver implements.
#[async_trait]
pub trait ImDriver: Send + Sync + std::fmt::Debug {
    fn id(&self) -> &'static str;

    /// Start the IM daemon inside the given session. The driver is responsible
    /// for setting framework-specific env vars (GTK_IM_MODULE, etc.) on its
    /// own child processes.
    async fn start(&mut self, session: &SessionHandle) -> Result<(), ImDriverError>;

    /// Shut down the daemon. Safe to call on a driver that was never started.
    async fn stop(&mut self) -> Result<(), ImDriverError>;

    /// Switch the active engine (e.g. `"Bamboo"`, `"Unikey"`).
    async fn activate_engine(&self, engine_name: &str) -> Result<(), ImDriverError>;

    /// Set the typing mode (Telex, VNI, …).
    async fn set_mode(&self, mode: InputMode) -> Result<(), ImDriverError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ImDriverError {
    #[error("binary `{0}` not found on PATH")]
    BinaryMissing(&'static str),

    #[error("{binary} exited with status {code:?}: {stderr}")]
    NonZeroExit { binary: &'static str, code: Option<i32>, stderr: String },

    #[error("{what} did not become ready within {secs}s")]
    StartupTimeout { what: &'static str, secs: u64 },

    #[error("engine `{0}` not found or could not be activated")]
    EngineNotFound(String),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}
