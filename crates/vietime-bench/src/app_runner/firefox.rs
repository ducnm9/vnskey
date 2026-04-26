// SPDX-License-Identifier: GPL-3.0-or-later
//
// `FirefoxRunner` — launches Firefox with a textarea page, captures text via
// xdotool+xclip (CDP upgrade tracked separately).
// BEN-21. Spec ref: `spec/03-phase3-test-suite.md` §B.4.

use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Instant};

use crate::session::SessionHandle;

use super::{xdotool_helper, AppInstance, AppRunner, AppRunnerError};

const FIREFOX_READY_TIMEOUT: Duration = Duration::from_secs(20);
const FIREFOX_READY_POLL: Duration = Duration::from_millis(500);

/// Minimal HTML page with a textarea for text capture.
const TEXTAREA_HTML: &str = r#"data:text/html,<html><body><textarea id="t" rows="20" cols="80" autofocus></textarea></body></html>"#;

#[derive(Debug)]
pub struct FirefoxRunner {
    display: Option<String>,
    child: Option<Child>,
}

impl FirefoxRunner {
    #[must_use]
    pub fn new() -> Self {
        Self { display: None, child: None }
    }
}

impl Default for FirefoxRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AppRunner for FirefoxRunner {
    fn id(&self) -> &'static str {
        "firefox"
    }

    async fn launch(&mut self, session: &SessionHandle) -> Result<AppInstance, AppRunnerError> {
        self.display = Some(session.display.clone());

        let mut cmd = Command::new("firefox");
        cmd.arg("--new-window")
            .arg(TEXTAREA_HTML)
            .env("DISPLAY", &session.display)
            .env("MOZ_ENABLE_WAYLAND", "0")
            .kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("firefox"),
            _ => AppRunnerError::Io(e),
        })?;
        let pid = child.id().unwrap_or(0);
        self.child = Some(child);

        let deadline = Instant::now() + FIREFOX_READY_TIMEOUT;
        let window_id;
        loop {
            if let Ok(wid) =
                xdotool_helper::search_window(&session.display, "Mozilla Firefox").await
            {
                window_id = wid;
                break;
            }
            if Instant::now() >= deadline {
                return Err(AppRunnerError::StartupTimeout {
                    what: "firefox",
                    secs: FIREFOX_READY_TIMEOUT.as_secs(),
                });
            }
            tokio::time::sleep(FIREFOX_READY_POLL).await;
        }

        // Give Firefox extra time to render the textarea page.
        tokio::time::sleep(Duration::from_secs(2)).await;

        Ok(AppInstance { pid, window_id: Some(window_id) })
    }

    async fn focus_text_area(&self, inst: &AppInstance) -> Result<(), AppRunnerError> {
        let display = self.display.as_deref().unwrap_or(":99");
        xdotool_helper::focus_window(display, inst).await?;
        // Tab into the textarea (Firefox may focus the address bar first).
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
        assert_eq!(FirefoxRunner::new().id(), "firefox");
    }
}
