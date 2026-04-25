// SPDX-License-Identifier: GPL-3.0-or-later
//
// Session driver trait — the abstraction the bench runner uses to bring up
// a headless X11 or Wayland compositor for a single run.
//
// Concrete implementations live in sibling modules (one per display server)
// so the Wayland driver arriving in Week 4 (BEN-30, `WestonDriver`) can slot
// in without reshuffling the existing X11 code.
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.3 (session drivers).

use async_trait::async_trait;

pub use vietime_core::SessionType;

pub mod xvfb;

pub use xvfb::XvfbDriver;

/// Contract every headless display-server driver implements. Kept deliberately
/// narrow: the runner only needs to start a session, read out the env vars
/// child processes should inherit (`DISPLAY=:99`, `WAYLAND_DISPLAY=...`), and
/// tear it down again at the end.
///
/// The trait is `async` because real drivers spawn processes and poll for
/// readiness — `tokio::process::Command` doesn't give a blocking API on
/// startup, and a synchronous trait would leak the tokio runtime choice into
/// every caller.
#[async_trait]
pub trait SessionDriver: Send + Sync + std::fmt::Debug {
    /// Short machine-readable id (`"xvfb"`, `"weston"`, …) — used in logs and
    /// the `list` output.
    fn id(&self) -> &'static str;

    /// Which `SessionType` this driver provides. `XvfbDriver` returns `X11`;
    /// `WestonDriver` (Week 4) will return `Wayland`.
    fn session_type(&self) -> SessionType;

    /// Spin the server up and return a handle describing where it's listening.
    /// Must be idempotent: calling `start()` twice without an intervening
    /// `stop()` should error rather than leak a second child process.
    async fn start(&mut self) -> Result<SessionHandle, SessionError>;

    /// Tear down anything `start()` spawned. Safe to call on a driver that was
    /// never started (no-op in that case).
    async fn stop(&mut self) -> Result<(), SessionError>;

    /// Environment variables the caller should set on every child process
    /// running *inside* this session (IBus daemon, gedit, xdotool, …). Kept
    /// as owned `String`s so the returned vector can be stashed in a runner
    /// struct without borrowing `self`.
    fn env_vars(&self, handle: &SessionHandle) -> Vec<(String, String)>;
}

/// Runtime handle returned by `SessionDriver::start()`. Everything needed to
/// point other processes at the new display server and — eventually — kill
/// them when the run is over.
#[derive(Debug, Clone)]
pub struct SessionHandle {
    /// X11: `":99"`. Wayland: `"wayland-0"`.
    pub display: String,
    /// PIDs of every child the driver spawned. `XvfbDriver` stores two
    /// (Xvfb itself + openbox); a Wayland driver may store just one.
    pub pids: Vec<u32>,
}

/// Everything that can go wrong bringing a headless session up. Variants are
/// deliberately narrow so callers can pattern-match and produce actionable
/// errors (`BinaryMissing` → "please `apt install xvfb`").
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("binary `{0}` not found on PATH")]
    BinaryMissing(&'static str),

    #[error("{what} exited before we could attach: status={status:?}")]
    EarlyExit { what: &'static str, status: Option<i32> },

    #[error("{what} did not become ready within {secs}s")]
    StartupTimeout { what: &'static str, secs: u64 },

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}
