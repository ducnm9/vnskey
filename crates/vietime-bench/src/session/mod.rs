// SPDX-License-Identifier: GPL-3.0-or-later
//
// Session driver trait + dispatcher (BEN-02, BEN-30, BEN-32).
// Spec ref: `spec/03-phase3-test-suite.md` §B.2.

use async_trait::async_trait;

pub use vietime_core::SessionType;

pub mod weston;
pub mod xvfb;

pub use weston::WestonDriver;
pub use xvfb::XvfbDriver;

#[async_trait]
pub trait SessionDriver: Send + Sync + std::fmt::Debug {
    fn id(&self) -> &'static str;
    fn session_type(&self) -> SessionType;
    async fn start(&mut self) -> Result<SessionHandle, SessionError>;
    async fn stop(&mut self) -> Result<(), SessionError>;
    fn env_vars(&self, handle: &SessionHandle) -> Vec<(String, String)>;
}

#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub display: String,
    pub pids: Vec<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("binary `{0}` not found on PATH")]
    BinaryMissing(&'static str),

    #[error("{what} exited before we could attach: status={status:?}")]
    EarlyExit { what: &'static str, status: Option<i32> },

    #[error("{what} did not become ready within {secs}s")]
    StartupTimeout { what: &'static str, secs: u64 },

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolve a session type string into a boxed driver (BEN-32).
#[must_use]
pub fn resolve_session(session: &str) -> Option<Box<dyn SessionDriver>> {
    match session {
        "x11" => Some(Box::new(XvfbDriver::new())),
        "wayland" => Some(Box::new(WestonDriver::new())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_x11() {
        let d = resolve_session("x11");
        assert!(d.is_some());
        assert_eq!(d.as_ref().map(|d| d.id()), Some("xvfb"));
    }

    #[test]
    fn resolve_wayland() {
        let d = resolve_session("wayland");
        assert!(d.is_some());
        assert_eq!(d.as_ref().map(|d| d.id()), Some("weston"));
    }

    #[test]
    fn resolve_unknown() {
        assert!(resolve_session("tty").is_none());
    }
}
