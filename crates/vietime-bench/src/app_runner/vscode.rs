// SPDX-License-Identifier: GPL-3.0-or-later
//
// `VscodeRunner` — launches VS Code (Electron) in a headless session.
// BEN-50. Spec ref: `spec/03-phase3-test-suite.md` §B.4.

use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Instant};

use crate::session::SessionHandle;

use super::{AppInstance, AppRunner, AppRunnerError, xdotool_helper};

const VSCODE_READY_TIMEOUT: Duration = Duration::from_secs(30);
const VSCODE_READY_POLL: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub struct VscodeRunner {
    display: Option<String>,
    child: Option<Child>,
}

impl VscodeRunner {
    #[must_use]
    pub fn new() -> Self {
        Self { display: None, child: None }
    }
}

impl Default for VscodeRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AppRunner for VscodeRunner {
    fn id(&self) -> &'static str {
        "vscode"
    }

    async fn launch(&mut self, session: &SessionHandle) -> Result<AppInstance, AppRunnerError> {
        self.display = Some(session.display.clone());

        let tmp_dir = std::env::temp_dir().join("vietime-bench-vscode");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let tmp_file = tmp_dir.join("bench.txt");
        let _ = std::fs::write(&tmp_file, "");

        let mut cmd = Command::new("code");
        cmd.args([
            "--no-sandbox",
            "--disable-gpu",
            "--disable-extensions",
            "--new-window",
        ])
        .arg(&tmp_file)
        .env("DISPLAY", &session.display)
        .kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("code"),
            _ => AppRunnerError::Io(e),
        })?;
        let pid = child.id().unwrap_or(0);
        self.child = Some(child);

        let deadline = Instant::now() + VSCODE_READY_TIMEOUT;
        let window_id;
        loop {
            if let Ok(wid) =
                xdotool_helper::search_window(&session.display, "Visual Studio Code").await
            {
                window_id = wid;
                break;
            }
            if Instant::now() >= deadline {
                return Err(AppRunnerError::StartupTimeout {
                    what: "vscode",
                    secs: VSCODE_READY_TIMEOUT.as_secs(),
                });
            }
            tokio::time::sleep(VSCODE_READY_POLL).await;
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
        assert_eq!(VscodeRunner::new().id(), "vscode");
    }
}
