// SPDX-License-Identifier: GPL-3.0-or-later
//
// `WestonDriver` — headless Wayland compositor driver.
// BEN-30. Spec ref: `spec/03-phase3-test-suite.md` §B.2.2.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Child;
use tokio::time::{timeout, Instant};

use vietime_core::SessionType;

use super::{SessionDriver, SessionError, SessionHandle};

const WESTON_READY_TIMEOUT: Duration = Duration::from_secs(5);
const WESTON_READY_POLL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub struct WestonDriver {
    wayland_display: String,
    runtime_dir: String,
    weston: Option<Child>,
}

impl WestonDriver {
    #[must_use]
    pub fn new() -> Self {
        Self::with_display("wayland-bench-0")
    }

    #[must_use]
    pub fn with_display(name: &str) -> Self {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_owned());
        Self { wayland_display: name.to_owned(), runtime_dir, weston: None }
    }

    #[must_use]
    pub fn display(&self) -> &str {
        &self.wayland_display
    }
}

impl Default for WestonDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionDriver for WestonDriver {
    fn id(&self) -> &'static str {
        "weston"
    }

    fn session_type(&self) -> SessionType {
        SessionType::Wayland
    }

    async fn start(&mut self) -> Result<SessionHandle, SessionError> {
        if self.weston.is_some() {
            return Err(SessionError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "WestonDriver::start called twice without stop",
            )));
        }

        let mut cmd = tokio::process::Command::new("weston");
        cmd.args([
            "--backend=headless",
            "--no-config",
            &format!("--socket={}", self.wayland_display),
            "--width=1920",
            "--height=1080",
        ])
        .env("XDG_RUNTIME_DIR", &self.runtime_dir)
        .kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => SessionError::BinaryMissing("weston"),
            _ => SessionError::Io(e),
        })?;
        let pid = child.id().unwrap_or(0);
        self.weston = Some(child);

        // Wait for the Wayland socket to appear.
        let socket_path = format!("{}/{}", self.runtime_dir, self.wayland_display);
        let deadline = Instant::now() + WESTON_READY_TIMEOUT;
        loop {
            if Path::new(&socket_path).exists() {
                break;
            }
            if Instant::now() >= deadline {
                return Err(SessionError::StartupTimeout {
                    what: "weston",
                    secs: WESTON_READY_TIMEOUT.as_secs(),
                });
            }
            tokio::time::sleep(WESTON_READY_POLL).await;
        }

        Ok(SessionHandle { display: self.wayland_display.clone(), pids: vec![pid] })
    }

    async fn stop(&mut self) -> Result<(), SessionError> {
        if let Some(mut w) = self.weston.take() {
            let _ = w.kill().await;
            let _ = timeout(Duration::from_secs(3), w.wait()).await;
        }
        Ok(())
    }

    fn env_vars(&self, handle: &SessionHandle) -> Vec<(String, String)> {
        vec![
            ("WAYLAND_DISPLAY".to_owned(), handle.display.clone()),
            ("XDG_RUNTIME_DIR".to_owned(), self.runtime_dir.clone()),
        ]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn id_and_session_type() {
        let d = WestonDriver::new();
        assert_eq!(d.id(), "weston");
        assert_eq!(d.session_type(), SessionType::Wayland);
    }

    #[test]
    fn display_name() {
        let d = WestonDriver::with_display("test-wayland");
        assert_eq!(d.display(), "test-wayland");
    }

    #[test]
    fn env_vars_project_wayland() {
        let d = WestonDriver::new();
        let handle = SessionHandle { display: "wayland-bench-0".to_owned(), pids: vec![1234] };
        let env = d.env_vars(&handle);
        assert!(env.iter().any(|(k, _)| k == "WAYLAND_DISPLAY"));
        assert!(env.iter().any(|(k, _)| k == "XDG_RUNTIME_DIR"));
    }

    #[tokio::test]
    #[ignore = "requires weston on the host"]
    async fn weston_start_stop_round_trip() {
        let mut driver = WestonDriver::new();
        let handle = driver.start().await.expect("weston should start");
        assert!(!handle.display.is_empty());
        driver.stop().await.expect("weston should stop");
    }
}
