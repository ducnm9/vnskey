// SPDX-License-Identifier: GPL-3.0-or-later
//
// IM framework driver trait + dispatcher (BEN-10, BEN-40).
// Spec ref: `spec/03-phase3-test-suite.md` §B.3.

use async_trait::async_trait;

use crate::model::InputMode;
use crate::session::SessionHandle;

pub mod fcitx5;
pub mod ibus;

pub use fcitx5::Fcitx5Driver;
pub use ibus::IbusDriver;

#[async_trait]
pub trait ImDriver: Send + Sync + std::fmt::Debug {
    fn id(&self) -> &'static str;
    async fn start(&mut self, session: &SessionHandle) -> Result<(), ImDriverError>;
    async fn stop(&mut self) -> Result<(), ImDriverError>;
    async fn activate_engine(&self, engine_name: &str) -> Result<(), ImDriverError>;
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

/// Resolve an engine slug (e.g. "ibus-bamboo") into an IM driver and engine name.
///
/// Returns `(driver, engine_name)` — the engine name is the part the IM
/// framework uses to activate (e.g. "Bamboo" for IBus, "bamboo" for Fcitx5).
#[must_use]
pub fn resolve_im_driver(engine_slug: &str) -> Option<(Box<dyn ImDriver>, String)> {
    let slug = engine_slug.to_ascii_lowercase();
    if slug.starts_with("ibus-") {
        let engine = slug.strip_prefix("ibus-").unwrap_or("bamboo");
        let engine_name = match engine {
            "bamboo" => "Bamboo",
            "unikey" => "Unikey",
            other => return Some((Box::new(IbusDriver::new()), other.to_owned())),
        };
        Some((Box::new(IbusDriver::new()), engine_name.to_owned()))
    } else if slug.starts_with("fcitx5-") {
        let engine = slug.strip_prefix("fcitx5-").unwrap_or("bamboo");
        Some((Box::new(Fcitx5Driver::new()), engine.to_owned()))
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn resolve_ibus_bamboo() {
        let (driver, name) = resolve_im_driver("ibus-bamboo").unwrap();
        assert_eq!(driver.id(), "ibus");
        assert_eq!(name, "Bamboo");
    }

    #[test]
    fn resolve_fcitx5_bamboo() {
        let (driver, name) = resolve_im_driver("fcitx5-bamboo").unwrap();
        assert_eq!(driver.id(), "fcitx5");
        assert_eq!(name, "bamboo");
    }

    #[test]
    fn resolve_unknown() {
        assert!(resolve_im_driver("scim-anthy").is_none());
    }
}
