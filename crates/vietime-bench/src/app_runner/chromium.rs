// SPDX-License-Identifier: GPL-3.0-or-later
//
// `ChromiumRunner` — launches Chromium/Chrome with a textarea page.
// BEN-22. Spec ref: `spec/03-phase3-test-suite.md` §B.4.

use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Instant};

use crate::session::SessionHandle;

use super::{xdotool_helper, AppInstance, AppRunner, AppRunnerError};

const CHROMIUM_READY_TIMEOUT: Duration = Duration::from_secs(20);
const CHROMIUM_READY_POLL: Duration = Duration::from_millis(500);

const TEXTAREA_HTML: &str = r#"data:text/html,<html><body><textarea id="t" rows="20" cols="80" autofocus></textarea></body></html>"#;

/// Chromium binary candidates in order of preference.
const CHROMIUM_BINARIES: &[&str] =
    &["chromium-browser", "chromium", "google-chrome-stable", "google-chrome"];

#[derive(Debug)]
pub struct ChromiumRunner {
    display: Option<String>,
    child: Option<Child>,
    binary: String,
}

impl ChromiumRunner {
    #[must_use]
    pub fn new() -> Self {
        Self { display: None, child: None, binary: find_chromium_binary() }
    }
}

impl Default for ChromiumRunner {
    fn default() -> Self {
        Self::new()
    }
}

fn find_chromium_binary() -> String {
    for bin in CHROMIUM_BINARIES {
        if which_exists(bin) {
            return (*bin).to_owned();
        }
    }
    "chromium-browser".to_owned()
}

fn which_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[async_trait]
impl AppRunner for ChromiumRunner {
    fn id(&self) -> &'static str {
        "chromium"
    }

    async fn launch(&mut self, session: &SessionHandle) -> Result<AppInstance, AppRunnerError> {
        self.display = Some(session.display.clone());

        let mut cmd = Command::new(&self.binary);
        cmd.args([
            "--no-sandbox",
            "--disable-gpu",
            "--no-first-run",
            "--disable-extensions",
            TEXTAREA_HTML,
        ])
        .env("DISPLAY", &session.display)
        .kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("chromium-browser"),
            _ => AppRunnerError::Io(e),
        })?;
        let pid = child.id().unwrap_or(0);
        self.child = Some(child);

        let deadline = Instant::now() + CHROMIUM_READY_TIMEOUT;
        let window_id;
        loop {
            if let Ok(wid) = xdotool_helper::search_window(&session.display, "Chromium").await {
                window_id = wid;
                break;
            }
            // Also try "Google Chrome" window title.
            if let Ok(wid) = xdotool_helper::search_window(&session.display, "Google Chrome").await
            {
                window_id = wid;
                break;
            }
            if Instant::now() >= deadline {
                return Err(AppRunnerError::StartupTimeout {
                    what: "chromium",
                    secs: CHROMIUM_READY_TIMEOUT.as_secs(),
                });
            }
            tokio::time::sleep(CHROMIUM_READY_POLL).await;
        }

        tokio::time::sleep(Duration::from_secs(2)).await;

        Ok(AppInstance { pid, window_id: Some(window_id) })
    }

    async fn focus_text_area(&self, inst: &AppInstance) -> Result<(), AppRunnerError> {
        let display = self.display.as_deref().unwrap_or(":99");
        xdotool_helper::focus_window(display, inst).await?;
        // Click into the textarea area.
        xdotool_helper::run_xdotool(display, &["key", "Tab"]).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    async fn clear_text_area(&self, inst: &AppInstance) -> Result<(), AppRunnerError> {
        let display = self.display.as_deref().unwrap_or(":99");
        xdotool_helper::select_all_delete(display, inst).await
    }

    async fn read_text(&self, inst: &AppInstance) -> Result<String, AppRunnerError> {
        let display = self.display.as_deref().unwrap_or(":99");
        xdotool_helper::copy_and_read_clipboard(display, inst).await
    }

    async fn close(&mut self, _inst: AppInstance) -> Result<(), AppRunnerError> {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill().await;
            let _ = timeout(Duration::from_secs(5), c.wait()).await;
        }
        self.display = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_stable() {
        assert_eq!(ChromiumRunner::new().id(), "chromium");
    }
}
