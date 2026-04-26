// SPDX-License-Identifier: GPL-3.0-or-later
//
// `LibreOfficeRunner` — launches LibreOffice Writer in a headless session.
// BEN-53. Spec ref: `spec/03-phase3-test-suite.md` §B.4.
//
// Caveat: `--headless` disables the IM framework, so we launch with a display
// instead. UNO API capture is ideal but complex; we use xdotool+xclip.

use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Instant};

use crate::session::SessionHandle;

use super::{xdotool_helper, AppInstance, AppRunner, AppRunnerError};

const LO_READY_TIMEOUT: Duration = Duration::from_secs(20);
const LO_READY_POLL: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub struct LibreOfficeRunner {
    display: Option<String>,
    child: Option<Child>,
}

impl LibreOfficeRunner {
    #[must_use]
    pub fn new() -> Self {
        Self { display: None, child: None }
    }
}

impl Default for LibreOfficeRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AppRunner for LibreOfficeRunner {
    fn id(&self) -> &'static str {
        "libreoffice"
    }

    async fn launch(&mut self, session: &SessionHandle) -> Result<AppInstance, AppRunnerError> {
        self.display = Some(session.display.clone());

        let mut cmd = Command::new("soffice");
        cmd.arg("--writer").arg("--norestore").env("DISPLAY", &session.display).kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("soffice"),
            _ => AppRunnerError::Io(e),
        })?;
        let pid = child.id().unwrap_or(0);
        self.child = Some(child);

        let deadline = Instant::now() + LO_READY_TIMEOUT;
        let window_id;
        loop {
            if let Ok(wid) = xdotool_helper::search_window(&session.display, "LibreOffice").await {
                window_id = wid;
                break;
            }
            if Instant::now() >= deadline {
                return Err(AppRunnerError::StartupTimeout {
                    what: "libreoffice",
                    secs: LO_READY_TIMEOUT.as_secs(),
                });
            }
            tokio::time::sleep(LO_READY_POLL).await;
        }

        tokio::time::sleep(Duration::from_secs(2)).await;

        Ok(AppInstance { pid, window_id: Some(window_id) })
    }

    async fn focus_text_area(&self, inst: &AppInstance) -> Result<(), AppRunnerError> {
        let display = self.display.as_deref().unwrap_or(":99");
        xdotool_helper::focus_window(display, inst).await
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
        assert_eq!(LibreOfficeRunner::new().id(), "libreoffice");
    }
}
