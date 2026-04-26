// SPDX-License-Identifier: GPL-3.0-or-later
//
// App runner trait + registry dispatcher (BEN-11, BEN-23).
// Spec ref: `spec/03-phase3-test-suite.md` §B.4.

use async_trait::async_trait;

use crate::session::SessionHandle;

pub mod xdotool_helper;

pub mod chromium;
pub mod electron;
pub mod firefox;
pub mod gedit;
pub mod kate;
pub mod libreoffice;
pub mod vscode;

pub use chromium::ChromiumRunner;
pub use electron::{DiscordRunner, ObsidianRunner, SlackRunner};
pub use firefox::FirefoxRunner;
pub use gedit::GeditRunner;
pub use kate::KateRunner;
pub use libreoffice::LibreOfficeRunner;
pub use vscode::VscodeRunner;

/// Runtime handle for a launched application instance.
#[derive(Debug)]
pub struct AppInstance {
    pub pid: u32,
    pub window_id: Option<String>,
}

/// Contract every app runner implements.
#[async_trait]
pub trait AppRunner: Send + Sync + std::fmt::Debug {
    fn id(&self) -> &'static str;

    async fn launch(&mut self, session: &SessionHandle) -> Result<AppInstance, AppRunnerError>;
    async fn focus_text_area(&self, inst: &AppInstance) -> Result<(), AppRunnerError>;
    async fn clear_text_area(&self, inst: &AppInstance) -> Result<(), AppRunnerError>;
    async fn read_text(&self, inst: &AppInstance) -> Result<String, AppRunnerError>;
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

/// All known app runner IDs, in display order for `list`.
pub const ALL_APP_IDS: &[&str] = &[
    "gedit",
    "kate",
    "firefox",
    "chromium",
    "vscode",
    "slack",
    "discord",
    "obsidian",
    "libreoffice",
];

/// Resolve an app id string into a boxed runner instance (BEN-23).
///
/// Returns `None` for unknown app ids — the caller decides whether to
/// error or skip.
#[must_use]
pub fn resolve_app(id: &str) -> Option<Box<dyn AppRunner>> {
    match id {
        "gedit" => Some(Box::new(GeditRunner::new())),
        "kate" => Some(Box::new(KateRunner::new())),
        "firefox" => Some(Box::new(FirefoxRunner::new())),
        "chromium" => Some(Box::new(ChromiumRunner::new())),
        "vscode" => Some(Box::new(VscodeRunner::new())),
        "slack" => Some(Box::new(SlackRunner::new())),
        "discord" => Some(Box::new(DiscordRunner::new())),
        "obsidian" => Some(Box::new(ObsidianRunner::new())),
        "libreoffice" => Some(Box::new(LibreOfficeRunner::new())),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn resolve_all_known_ids() {
        for id in ALL_APP_IDS {
            let runner = resolve_app(id);
            assert!(runner.is_some(), "resolve_app({id}) should return Some");
            assert_eq!(runner.as_ref().map(|r| r.id()), Some(*id));
        }
    }

    #[test]
    fn resolve_unknown_returns_none() {
        assert!(resolve_app("notepad").is_none());
        assert!(resolve_app("").is_none());
    }

    #[test]
    fn all_app_ids_count() {
        assert_eq!(ALL_APP_IDS.len(), 9);
    }
}
