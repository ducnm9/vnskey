// SPDX-License-Identifier: GPL-3.0-or-later
//
// `IbusDriver` — starts `ibus-daemon` inside a headless session, activates
// a Vietnamese engine, and configures the typing mode.
//
// BEN-10. Spec ref: `spec/03-phase3-test-suite.md` §B.3.

use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::time::timeout;

use crate::model::InputMode;
use crate::session::SessionHandle;

use super::{ImDriver, ImDriverError};

const IBUS_READY_TIMEOUT: Duration = Duration::from_secs(5);
const IBUS_READY_POLL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub struct IbusDriver {
    display: Option<String>,
    daemon: Option<Child>,
}

impl IbusDriver {
    #[must_use]
    pub fn new() -> Self {
        Self { display: None, daemon: None }
    }

    fn env_for(session: &SessionHandle) -> Vec<(&str, &str)> {
        vec![("DISPLAY", session.display.as_str())]
    }
}

impl Default for IbusDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ImDriver for IbusDriver {
    fn id(&self) -> &'static str {
        "ibus"
    }

    async fn start(&mut self, session: &SessionHandle) -> Result<(), ImDriverError> {
        if self.daemon.is_some() {
            return Err(ImDriverError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "IbusDriver::start called twice without stop",
            )));
        }

        self.display = Some(session.display.clone());

        let mut cmd = Command::new("ibus-daemon");
        cmd.arg("--daemonize")
            .arg("--replace")
            .arg("--xim");
        for (k, v) in Self::env_for(session) {
            cmd.env(k, v);
        }
        cmd.kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ImDriverError::BinaryMissing("ibus-daemon"),
            _ => ImDriverError::Io(e),
        })?;
        self.daemon = Some(child);

        // Wait for ibus to become ready by polling `ibus read-cache`.
        let deadline = tokio::time::Instant::now() + IBUS_READY_TIMEOUT;
        loop {
            let mut check = Command::new("ibus");
            check.arg("read-cache");
            if let Some(d) = &self.display {
                check.env("DISPLAY", d);
            }
            if let Ok(output) = check.output().await {
                if output.status.success() {
                    break;
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(ImDriverError::StartupTimeout {
                    what: "ibus-daemon",
                    secs: IBUS_READY_TIMEOUT.as_secs(),
                });
            }
            tokio::time::sleep(IBUS_READY_POLL).await;
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ImDriverError> {
        if let Some(mut d) = self.daemon.take() {
            let _ = d.kill().await;
            let _ = timeout(Duration::from_secs(2), d.wait()).await;
        }
        self.display = None;
        Ok(())
    }

    async fn activate_engine(&self, engine_name: &str) -> Result<(), ImDriverError> {
        let mut cmd = Command::new("ibus");
        cmd.arg("engine").arg(engine_name);
        if let Some(d) = &self.display {
            cmd.env("DISPLAY", d);
        }

        let output = cmd.output().await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ImDriverError::BinaryMissing("ibus"),
            _ => ImDriverError::Io(e),
        })?;

        if !output.status.success() {
            return Err(ImDriverError::EngineNotFound(engine_name.to_owned()));
        }
        Ok(())
    }

    async fn set_mode(&self, mode: InputMode) -> Result<(), ImDriverError> {
        // Bamboo stores its mode in gsettings. The schema path is
        // `org.freedesktop.ibus.engine.bamboo`, key `input-method`.
        let mode_value = match mode {
            InputMode::Telex => "telex",
            InputMode::Vni => "vni",
            InputMode::Viqr => "viqr",
            InputMode::SimpleTelex => "simple-telex",
        };

        let mut cmd = Command::new("gsettings");
        cmd.arg("set")
            .arg("org.freedesktop.ibus.engine.bamboo")
            .arg("input-method")
            .arg(mode_value);
        if let Some(d) = &self.display {
            cmd.env("DISPLAY", d);
        }

        let output = cmd.output().await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ImDriverError::BinaryMissing("gsettings"),
            _ => ImDriverError::Io(e),
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(ImDriverError::NonZeroExit {
                binary: "gsettings",
                code: output.status.code(),
                stderr,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn id_is_stable() {
        assert_eq!(IbusDriver::new().id(), "ibus");
    }

    #[test]
    fn default_has_no_daemon() {
        let d = IbusDriver::default();
        assert!(d.daemon.is_none());
        assert!(d.display.is_none());
    }

    #[tokio::test]
    #[ignore = "requires ibus-daemon on the host"]
    async fn start_stop_round_trip() {
        let mut driver = IbusDriver::new();
        let session = SessionHandle {
            display: ":99".to_owned(),
            pids: vec![],
        };
        driver.start(&session).await.expect("should start");
        driver.stop().await.expect("should stop");
    }
}
