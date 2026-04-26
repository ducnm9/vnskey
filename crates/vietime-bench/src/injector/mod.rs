// SPDX-License-Identifier: GPL-3.0-or-later
//
// Keystroke injector trait + dispatcher (BEN-03, BEN-31).
// Spec ref: `spec/03-phase3-test-suite.md` §B.5.

use async_trait::async_trait;

pub mod xdotool;
pub mod ydotool;

pub use xdotool::XdotoolInjector;
pub use ydotool::YdotoolInjector;

#[async_trait]
pub trait KeystrokeInjector: Send + Sync + std::fmt::Debug {
    fn id(&self) -> &'static str;
    async fn type_raw(&self, keys: &str, ms_per_key: u32) -> Result<(), InjectorError>;
}

#[derive(Debug, thiserror::Error)]
pub enum InjectorError {
    #[error("binary `{0}` not found on PATH")]
    BinaryMissing(&'static str),

    #[error("{binary} exited with status {code:?}: {stderr}")]
    NonZeroExit { binary: &'static str, code: Option<i32>, stderr: String },

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolve a session type string into a boxed injector.
#[must_use]
pub fn resolve_injector(session: &str, display: &str) -> Box<dyn KeystrokeInjector> {
    match session {
        "wayland" => Box::new(YdotoolInjector::new(display)),
        _ => Box::new(XdotoolInjector::new(display)),
    }
}
