// SPDX-License-Identifier: GPL-3.0-or-later
//
// App runner trait (BEN-11).
//
// Each target application (gedit, kate, firefox, …) gets a runner that can
// launch it inside a headless session, focus its text area, inject text, read
// it back, and tear it down.
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.4.

use async_trait::async_trait;

use crate::session::SessionHandle;

pub mod gedit;

pub use gedit::GeditRunner;

/// Runtime handle for a launched application instance.
#[derive(Debug)]
pub struct AppInstance {
    pub pid: u32,
    pub window_id: Option<String>,
}

/// Contract every app runner implements.
#[async_trait]
pub trait AppRunner: Send + Sync + std::fmt::Debug {
    /// Short machine-readable id (`"gedit"`, `"firefox"`, …).
    fn id(&self) -> &'static str;

    /// Launch the app inside the given session. Returns a handle the caller
    /// uses for all subsequent operations.
    async fn launch(&mut self, session: &SessionHandle) -> Result<AppInstance, AppRunnerError>;

    /// Focus the primary text input area so keystrokes go to the right widget.
    async fn focus_text_area(&self, inst: &AppInstance) -> Result<(), AppRunnerError>;

    /// Clear whatever text is currently in the input area.
    async fn clear_text_area(&self, inst: &AppInstance) -> Result<(), AppRunnerError>;

    /// Read back the text currently in the input area. This is the actual
    /// output the scoring engine compares against expected.
    async fn read_text(&self, inst: &AppInstance) -> Result<String, AppRunnerError>;

    /// Close the app, cleaning up any resources.
    async fn close(&mut self, inst: AppInstance) -> Result<(), AppRunnerError>;
}

#[derive(Debug, thiserror::Error)]
pub enum AppRunnerError {
    #[error("binary `{0}` not found on PATH")]
    BinaryMissing(&'static str),

    #[error("{what} did not become ready within {secs}s")]
    StartupTimeout { what: &'static str, secs: u64 },

    #[error("failed to read text from app: {0}")]
    CaptureFailure(String),

    #[error("{binary} exited with status {code:?}: {stderr}")]
    NonZeroExit { binary: &'static str, code: Option<i32>, stderr: String },

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}
