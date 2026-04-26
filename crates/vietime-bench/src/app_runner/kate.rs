// SPDX-License-Identifier: GPL-3.0-or-later
//
// `KateRunner` — launches Kate (Qt text editor) in a headless X11 session.
// BEN-20. Spec ref: `spec/03-phase3-test-suite.md` §B.4.

use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Instant};

use crate::session::SessionHandle;

use super::{xdotool_helper, AppInstance, AppRunner, AppRunnerError};

const KATE_READY_TIMEOUT: Duration = Duration::from_secs(15);
const KATE_READY_POLL: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub struct KateRunner {
    display: Option<String>,
    child: Option<Child>,
}

impl KateRunner {
    #[must_use]
    pub fn new() -> Self {
        Self { display: None, child: None }
    }
}

impl Default for KateRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AppRunner for KateRunner {
    fn id(&self) -> &'static str {
        "kate"
    }

    async fn launch(&mut self, session: &SessionHandle) -> Result<AppInstance, AppRunnerError> {
        self.display = Some(session.display.clone());

        let mut cmd = Command::new("kate");
        cmd.arg("--new").env("DISPLAY", &session.display).kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("kate"),
            _ => AppRunnerError::Io(e),
        })?;
        let pid = child.id().unwrap_or(0);
        self.child = Some(child);

        let deadline = Instant::now() + KATE_READY_TIMEOUT;
        let window_id;
        loop {
            if let Ok(wid) = xdotool_helper::search_window(&session.display, "kate").await {
                window_id = wid;
                break;
            }
            if Instant::now() >= deadline {
                return Err(AppRunnerError::StartupTimeout {
                    what: "kate",
                    secs: KATE_READY_TIMEOUT.as_secs(),
                });
            }
            tokio::time::sleep(KATE_READY_POLL).await;
        }

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
            let _ = timeout(Duration::from_secs(3), c.wait()).await;
        }
        self.display = None;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn id_is_stable() {
        assert_eq!(KateRunner::new().id(), "kate");
    }
}
