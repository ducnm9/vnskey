// SPDX-License-Identifier: GPL-3.0-or-later
//
// Electron-based app runners: Slack, Discord, Obsidian.
// BEN-51, BEN-52. Spec ref: `spec/03-phase3-test-suite.md` §B.4.
//
// These apps share the same xdotool-based launch/focus/capture pattern.
// In a production bench with login requirements, these would need special
// setup (offline mode, test accounts). Documented as caveats.

use std::time::Duration;

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Instant};

use crate::session::SessionHandle;

use super::{AppInstance, AppRunner, AppRunnerError, xdotool_helper};

const ELECTRON_READY_TIMEOUT: Duration = Duration::from_secs(30);
const ELECTRON_READY_POLL: Duration = Duration::from_millis(500);

macro_rules! electron_runner {
    ($name:ident, $id:literal, $binary:literal, $window_title:literal) => {
        #[derive(Debug)]
        pub struct $name {
            display: Option<String>,
            child: Option<Child>,
        }

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self { display: None, child: None }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        #[async_trait]
        impl AppRunner for $name {
            fn id(&self) -> &'static str {
                $id
            }

            async fn launch(
                &mut self,
                session: &SessionHandle,
            ) -> Result<AppInstance, AppRunnerError> {
                self.display = Some(session.display.clone());

                let mut cmd = Command::new($binary);
                cmd.env("DISPLAY", &session.display).kill_on_drop(true);

                let child = cmd.spawn().map_err(|e| match e.kind() {
                    std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing($binary),
                    _ => AppRunnerError::Io(e),
                })?;
                let pid = child.id().unwrap_or(0);
                self.child = Some(child);

                let deadline = Instant::now() + ELECTRON_READY_TIMEOUT;
                let window_id;
                loop {
                    if let Ok(wid) =
                        xdotool_helper::search_window(&session.display, $window_title).await
                    {
                        window_id = wid;
                        break;
                    }
                    if Instant::now() >= deadline {
                        return Err(AppRunnerError::StartupTimeout {
                            what: $id,
                            secs: ELECTRON_READY_TIMEOUT.as_secs(),
                        });
                    }
                    tokio::time::sleep(ELECTRON_READY_POLL).await;
                }

                tokio::time::sleep(Duration::from_secs(2)).await;

                Ok(AppInstance {
                    pid,
                    window_id: Some(window_id),
                })
            }

            async fn focus_text_area(
                &self,
                inst: &AppInstance,
            ) -> Result<(), AppRunnerError> {
                let display = self.display.as_deref().unwrap_or(":99");
                xdotool_helper::focus_window(display, inst).await
            }

            async fn clear_text_area(
                &self,
                inst: &AppInstance,
            ) -> Result<(), AppRunnerError> {
                let display = self.display.as_deref().unwrap_or(":99");
                xdotool_helper::select_all_delete(display, inst).await
            }

            async fn read_text(
                &self,
                inst: &AppInstance,
            ) -> Result<String, AppRunnerError> {
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
    };
}

electron_runner!(SlackRunner, "slack", "slack", "Slack");
electron_runner!(DiscordRunner, "discord", "discord", "Discord");
electron_runner!(ObsidianRunner, "obsidian", "obsidian", "Obsidian");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable() {
        assert_eq!(SlackRunner::new().id(), "slack");
        assert_eq!(DiscordRunner::new().id(), "discord");
        assert_eq!(ObsidianRunner::new().id(), "obsidian");
    }
}
