// SPDX-License-Identifier: GPL-3.0-or-later
//
// `Fcitx5Driver` — starts fcitx5 inside a headless session.
// BEN-40. Spec ref: `spec/03-phase3-test-suite.md` §B.3.

use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::time::timeout;

use crate::model::InputMode;
use crate::session::SessionHandle;

use super::{ImDriver, ImDriverError};

const FCITX5_READY_TIMEOUT: Duration = Duration::from_secs(5);
const FCITX5_READY_POLL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub struct Fcitx5Driver {
    display: Option<String>,
    daemon: Option<Child>,
}

impl Fcitx5Driver {
    #[must_use]
    pub fn new() -> Self {
        Self { display: None, daemon: None }
    }
}

impl Default for Fcitx5Driver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ImDriver for Fcitx5Driver {
    fn id(&self) -> &'static str {
        "fcitx5"
    }

    async fn start(&mut self, session: &SessionHandle) -> Result<(), ImDriverError> {
        if self.daemon.is_some() {
            return Err(ImDriverError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Fcitx5Driver::start called twice without stop",
            )));
        }

        self.display = Some(session.display.clone());

        let mut cmd = Command::new("fcitx5");
        cmd.arg("-d").env("DISPLAY", &session.display).kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ImDriverError::BinaryMissing("fcitx5"),
            _ => ImDriverError::Io(e),
        })?;
        self.daemon = Some(child);

        // Wait for fcitx5 to become ready.
        let deadline = tokio::time::Instant::now() + FCITX5_READY_TIMEOUT;
        loop {
            let mut check = Command::new("fcitx5-remote");
            if let Some(d) = &self.display {
                check.env("DISPLAY", d);
            }
            if let Ok(output) = check.output().await {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // fcitx5-remote returns "2" when running.
                    if stdout.trim() == "2" || stdout.trim() == "1" {
                        break;
                    }
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(ImDriverError::StartupTimeout {
                    what: "fcitx5",
                    secs: FCITX5_READY_TIMEOUT.as_secs(),
                });
            }
            tokio::time::sleep(FCITX5_READY_POLL).await;
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
        let mut cmd = Command::new("fcitx5-remote");
        cmd.arg("-s").arg(engine_name);
        if let Some(d) = &self.display {
            cmd.env("DISPLAY", d);
        }

        let output = cmd.output().await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ImDriverError::BinaryMissing("fcitx5-remote"),
            _ => ImDriverError::Io(e),
        })?;

        if !output.status.success() {
            return Err(ImDriverError::EngineNotFound(engine_name.to_owned()));
        }
        Ok(())
    }

    async fn set_mode(&self, mode: InputMode) -> Result<(), ImDriverError> {
        // Fcitx5-bamboo stores mode in its config file.
        let mode_value = match mode {
            InputMode::Telex => "telex",
            InputMode::Vni => "vni",
            InputMode::Viqr => "viqr",
            InputMode::SimpleTelex => "simple-telex",
        };

        let config_dir = dirs_config_fcitx5();
        let config_path = format!("{config_dir}/conf/bamboo.conf");

        // Write the mode config.
        let content = format!("InputMethod={mode_value}\n");
        if let Err(e) = std::fs::create_dir_all(format!("{config_dir}/conf")) {
            return Err(ImDriverError::Io(e));
        }
        std::fs::write(&config_path, content).map_err(ImDriverError::Io)?;

        // Reload fcitx5 config.
        let mut cmd = Command::new("fcitx5-remote");
        cmd.arg("-r");
        if let Some(d) = &self.display {
            cmd.env("DISPLAY", d);
        }
        let _ = cmd.output().await;

        Ok(())
    }
}

fn dirs_config_fcitx5() -> String {
    std::env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
            format!("{home}/.config")
        })
        + "/fcitx5"
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn id_is_stable() {
        assert_eq!(Fcitx5Driver::new().id(), "fcitx5");
    }

    #[test]
    fn default_has_no_daemon() {
        let d = Fcitx5Driver::default();
        assert!(d.daemon.is_none());
    }
}
