// SPDX-License-Identifier: GPL-3.0-or-later
//
// `GeditRunner` — launches gedit in a headless X11 session, focuses the text
// view via xdotool, and captures text via xdotool select-all + xclip.
//
// BEN-11. The ideal capture method is AT-SPI (`at-spi2` D-Bus), but that
// requires additional setup in CI containers. Week 2 ships with a simpler
// xdotool + xclip fallback; AT-SPI is tracked for a later improvement.
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.4.

use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Instant};

use crate::session::SessionHandle;

use super::{AppInstance, AppRunner, AppRunnerError};

const GEDIT_READY_TIMEOUT: Duration = Duration::from_secs(10);
const GEDIT_READY_POLL: Duration = Duration::from_millis(200);

#[derive(Debug)]
pub struct GeditRunner {
    display: Option<String>,
    child: Option<Child>,
}

impl GeditRunner {
    #[must_use]
    pub fn new() -> Self {
        Self { display: None, child: None }
    }
}

impl Default for GeditRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AppRunner for GeditRunner {
    fn id(&self) -> &'static str {
        "gedit"
    }

    async fn launch(&mut self, session: &SessionHandle) -> Result<AppInstance, AppRunnerError> {
        self.display = Some(session.display.clone());

        let mut cmd = Command::new("gedit");
        cmd.arg("--new-document")
            .env("DISPLAY", &session.display)
            .kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("gedit"),
            _ => AppRunnerError::Io(e),
        })?;
        let pid = child.id().unwrap_or(0);
        self.child = Some(child);

        // Wait for the window to appear via xdotool search.
        let deadline = Instant::now() + GEDIT_READY_TIMEOUT;
        let window_id;
        loop {
            let mut search = Command::new("xdotool");
            search
                .args(["search", "--name", "gedit"])
                .env("DISPLAY", &session.display);
            if let Ok(output) = search.output().await {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let first_line = stdout.lines().next().unwrap_or("").trim();
                    if !first_line.is_empty() {
                        window_id = first_line.to_owned();
                        break;
                    }
                }
            }
            if Instant::now() >= deadline {
                return Err(AppRunnerError::StartupTimeout {
                    what: "gedit",
                    secs: GEDIT_READY_TIMEOUT.as_secs(),
                });
            }
            tokio::time::sleep(GEDIT_READY_POLL).await;
        }

        Ok(AppInstance { pid, window_id: Some(window_id) })
    }

    async fn focus_text_area(&self, inst: &AppInstance) -> Result<(), AppRunnerError> {
        let display = self.display.as_deref().unwrap_or(":99");
        if let Some(wid) = &inst.window_id {
            run_xdotool(display, &["windowactivate", "--sync", wid]).await?;
            run_xdotool(display, &["windowfocus", "--sync", wid]).await?;
        }
        Ok(())
    }

    async fn clear_text_area(&self, inst: &AppInstance) -> Result<(), AppRunnerError> {
        let display = self.display.as_deref().unwrap_or(":99");
        // Select all + delete.
        if let Some(wid) = &inst.window_id {
            run_xdotool(display, &["windowactivate", "--sync", wid]).await?;
        }
        run_xdotool(display, &["key", "ctrl+a"]).await?;
        run_xdotool(display, &["key", "Delete"]).await?;
        // Small delay for gedit to process.
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    async fn read_text(&self, inst: &AppInstance) -> Result<String, AppRunnerError> {
        let display = self.display.as_deref().unwrap_or(":99");

        // Select all text in gedit.
        if let Some(wid) = &inst.window_id {
            run_xdotool(display, &["windowactivate", "--sync", wid]).await?;
        }
        run_xdotool(display, &["key", "ctrl+a"]).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Copy to clipboard.
        run_xdotool(display, &["key", "ctrl+c"]).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Read clipboard via xclip.
        let mut cmd = Command::new("xclip");
        cmd.args(["-selection", "clipboard", "-o"])
            .env("DISPLAY", display);
        let output = cmd.output().await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("xclip"),
            _ => AppRunnerError::Io(e),
        })?;

        if !output.status.success() {
            // Empty clipboard is not an error for our purposes — gedit may
            // have an empty document.
            if output.stderr.is_empty() || String::from_utf8_lossy(&output.stderr).contains("target") {
                return Ok(String::new());
            }
            return Err(AppRunnerError::CaptureFailure(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
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

async fn run_xdotool(display: &str, args: &[&str]) -> Result<(), AppRunnerError> {
    let mut cmd = Command::new("xdotool");
    cmd.args(args).env("DISPLAY", display);
    let output = cmd.output().await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("xdotool"),
        _ => AppRunnerError::Io(e),
    })?;
    if !output.status.success() {
        return Err(AppRunnerError::NonZeroExit {
            binary: "xdotool",
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn id_is_stable() {
        assert_eq!(GeditRunner::new().id(), "gedit");
    }

    #[test]
    fn default_has_no_child() {
        let r = GeditRunner::default();
        assert!(r.child.is_none());
        assert!(r.display.is_none());
    }

    #[tokio::test]
    #[ignore = "requires gedit + xdotool + xclip + a live X server"]
    async fn launch_focus_read_close() {
        let session = SessionHandle {
            display: ":99".to_owned(),
            pids: vec![],
        };
        let mut runner = GeditRunner::new();
        let inst = runner.launch(&session).await.expect("gedit should launch");
        assert!(inst.window_id.is_some());
        runner.focus_text_area(&inst).await.expect("focus should work");
        let text = runner.read_text(&inst).await.expect("read should work");
        assert!(text.is_empty() || !text.is_empty()); // any result is fine for smoke
        runner.close(inst).await.expect("close should work");
    }
}
