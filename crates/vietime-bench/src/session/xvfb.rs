// SPDX-License-Identifier: GPL-3.0-or-later
//
// `XvfbDriver` — X11 session driver backed by the Xvfb virtual framebuffer
// plus the `openbox` window manager (needed so GTK apps don't bail out of a
// windowless display).
//
// Week 1 (BEN-02) lands the type + env plumbing. The actual spawn + teardown
// flow is wired up but its integration test is `#[ignore]` because it needs
// real binaries on the host. Unit tests cover the pure pieces
// (`choose_display_number` via an injected listing fn, `env_vars()`).
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.3 + §B.6 (run flow).

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Child;
use tokio::time::{timeout, Instant};

use vietime_core::SessionType;

use super::{SessionDriver, SessionError, SessionHandle};

/// Standard Xvfb screen geometry — matches what Fedora/Ubuntu CI pipelines use
/// for GUI smoke tests. 1920×1080×24 is plenty for gedit/kate/firefox.
const DEFAULT_GEOMETRY: &str = "1920x1080x24";

/// How long we wait for the `/tmp/.X11-unix/X<n>` socket to appear after
/// spawning `Xvfb`. Generous on purpose — 2s covers cold CI boxes.
const XVFB_READY_TIMEOUT: Duration = Duration::from_secs(2);

/// How often we poll for the socket while inside the readiness window.
const XVFB_READY_POLL: Duration = Duration::from_millis(50);

/// Range we search when auto-picking a display number. `:0`–`:2` are almost
/// always the user's real session; we start well above that to avoid stomping
/// on it even if the caller accidentally runs the bench on a desktop login.
const DISPLAY_SEARCH_RANGE: std::ops::Range<u32> = 99..256;

/// The X11 Unix socket directory. The exact path is hard-coded by the X
/// server — no knob to change it — so it's safe to rely on.
const X11_UNIX_DIR: &str = "/tmp/.X11-unix";

/// Headless X11 driver. Construct once, call `start()` to bring the server
/// up, `stop()` at the end of the run.
#[derive(Debug)]
pub struct XvfbDriver {
    /// Chosen display number (`99` → `":99"`).
    display_number: u32,
    /// Screen geometry passed to Xvfb's `-screen 0` arg.
    geometry: String,
    /// Running children — `None` between `new()` and `start()`, `Some` while
    /// the session is live. Stored in start-order so we kill openbox before
    /// Xvfb (otherwise openbox gets SIGPIPE and clutters stderr).
    xvfb: Option<Child>,
    openbox: Option<Child>,
}

impl XvfbDriver {
    /// Build a driver with an auto-picked display number. Picks the lowest
    /// free display in the `99..256` range by scanning `/tmp/.X11-unix/X*`.
    /// Falls back to `:99` if scanning fails.
    #[must_use]
    pub fn new() -> Self {
        let display_number = choose_display_number(X11_UNIX_DIR, list_x11_sockets).unwrap_or(99);
        Self::with_display(display_number)
    }

    /// Build a driver for a caller-chosen display number — useful when
    /// nesting inside a container that already reserves a specific display.
    #[must_use]
    pub fn with_display(display_number: u32) -> Self {
        Self { display_number, geometry: DEFAULT_GEOMETRY.to_owned(), xvfb: None, openbox: None }
    }

    /// Colon-prefixed display string: `":99"`.
    #[must_use]
    pub fn display(&self) -> String {
        format!(":{}", self.display_number)
    }
}

impl Default for XvfbDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionDriver for XvfbDriver {
    fn id(&self) -> &'static str {
        "xvfb"
    }

    fn session_type(&self) -> SessionType {
        SessionType::X11
    }

    async fn start(&mut self) -> Result<SessionHandle, SessionError> {
        if self.xvfb.is_some() {
            // Double-start is a programmer bug — easier to spot as an I/O
            // error than a silent resource leak.
            return Err(SessionError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "XvfbDriver::start called twice without stop",
            )));
        }

        let display = self.display();

        // 1. Spawn Xvfb.
        let mut xvfb_cmd = tokio::process::Command::new("Xvfb");
        xvfb_cmd.arg(&display).arg("-screen").arg("0").arg(&self.geometry).kill_on_drop(true);

        let xvfb = xvfb_cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => SessionError::BinaryMissing("Xvfb"),
            _ => SessionError::Io(e),
        })?;
        let xvfb_pid = xvfb.id().unwrap_or(0);
        self.xvfb = Some(xvfb);

        // 2. Wait for the socket to appear — proof Xvfb is ready for clients.
        wait_for_socket(self.display_number, XVFB_READY_TIMEOUT).await?;

        // 3. Spawn openbox so GTK/Qt apps have a window manager to talk to.
        let mut openbox_cmd = tokio::process::Command::new("openbox");
        openbox_cmd.env("DISPLAY", &display).kill_on_drop(true);

        let openbox = openbox_cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => SessionError::BinaryMissing("openbox"),
            _ => SessionError::Io(e),
        })?;
        let openbox_pid = openbox.id().unwrap_or(0);
        self.openbox = Some(openbox);

        Ok(SessionHandle { display, pids: vec![xvfb_pid, openbox_pid] })
    }

    async fn stop(&mut self) -> Result<(), SessionError> {
        // Kill openbox first — it's a leaf process and logs spam if it
        // outlives its X server.
        if let Some(mut ob) = self.openbox.take() {
            let _ = ob.kill().await;
            let _ = timeout(Duration::from_secs(2), ob.wait()).await;
        }
        if let Some(mut x) = self.xvfb.take() {
            let _ = x.kill().await;
            let _ = timeout(Duration::from_secs(2), x.wait()).await;
        }
        Ok(())
    }

    fn env_vars(&self, handle: &SessionHandle) -> Vec<(String, String)> {
        vec![("DISPLAY".to_owned(), handle.display.clone())]
    }
}

/// Scan `dir` (typically `/tmp/.X11-unix`) for existing `X<n>` sockets and
/// pick the lowest free number in `DISPLAY_SEARCH_RANGE`. Returns `None` if
/// every candidate is taken — in practice the caller falls back to `99`
/// because even on a pathological box that's better than panicking.
///
/// The listing fn is injected for testability: unit tests pass a closure
/// returning a fixture, production passes `list_x11_sockets`.
#[must_use]
fn choose_display_number(
    dir: &str,
    lister: impl Fn(&str) -> std::io::Result<Vec<u32>>,
) -> Option<u32> {
    let taken: std::collections::HashSet<u32> =
        lister(dir).unwrap_or_default().into_iter().collect();
    DISPLAY_SEARCH_RANGE.clone().find(|n| !taken.contains(n))
}

/// Real filesystem listing — reads `/tmp/.X11-unix/` and pulls the number
/// out of each `X<n>` entry. Returns an empty vector on any error so we can
/// still pick a sane default.
fn list_x11_sockets(dir: &str) -> std::io::Result<Vec<u32>> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(s) = name.to_str() else { continue };
        if let Some(rest) = s.strip_prefix('X') {
            if let Ok(n) = rest.parse::<u32>() {
                out.push(n);
            }
        }
    }
    Ok(out)
}

/// Poll `/tmp/.X11-unix/X<n>` until it exists or `budget` elapses. Xvfb
/// creates the socket the moment it's ready for client connections.
async fn wait_for_socket(display_number: u32, budget: Duration) -> Result<(), SessionError> {
    let path = format!("{X11_UNIX_DIR}/X{display_number}");
    let deadline = Instant::now() + budget;
    loop {
        if Path::new(&path).exists() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(SessionError::StartupTimeout { what: "Xvfb", secs: budget.as_secs() });
        }
        tokio::time::sleep(XVFB_READY_POLL).await;
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn choose_display_picks_99_when_all_free() {
        let picked = choose_display_number("/tmp/.X11-unix", |_| Ok(Vec::new()));
        assert_eq!(picked, Some(99));
    }

    #[test]
    fn choose_display_skips_taken_numbers() {
        let picked = choose_display_number("/tmp/.X11-unix", |_| Ok(vec![99, 100, 101]));
        assert_eq!(picked, Some(102));
    }

    #[test]
    fn choose_display_falls_back_to_none_when_full() {
        // Shouldn't happen in practice, but make sure we don't infinite-loop.
        let taken: Vec<u32> = DISPLAY_SEARCH_RANGE.clone().collect();
        let picked = choose_display_number("/tmp/.X11-unix", move |_| Ok(taken.clone()));
        assert_eq!(picked, None);
    }

    #[test]
    fn env_vars_projects_display_from_handle() {
        let driver = XvfbDriver::with_display(99);
        let handle = SessionHandle { display: ":99".to_owned(), pids: vec![1234, 5678] };
        let env = driver.env_vars(&handle);
        assert_eq!(env, vec![("DISPLAY".to_owned(), ":99".to_owned())]);
    }

    #[test]
    fn display_string_is_colon_prefixed() {
        let driver = XvfbDriver::with_display(42);
        assert_eq!(driver.display(), ":42");
    }

    #[test]
    fn id_and_session_type_are_stable() {
        let driver = XvfbDriver::with_display(99);
        assert_eq!(driver.id(), "xvfb");
        assert_eq!(driver.session_type(), SessionType::X11);
    }

    // Real-spawn integration test — gated because it needs Xvfb + openbox
    // binaries on the host, which macOS dev boxes don't have. CI opts in
    // from Week 8 (BEN-70).
    #[tokio::test]
    #[ignore = "requires Xvfb + openbox on the host"]
    async fn xvfb_start_stop_round_trip() {
        let mut driver = XvfbDriver::new();
        let handle = driver.start().await.expect("xvfb should start");
        assert!(handle.display.starts_with(':'));
        assert_eq!(handle.pids.len(), 2);
        driver.stop().await.expect("xvfb should stop cleanly");
    }
}
