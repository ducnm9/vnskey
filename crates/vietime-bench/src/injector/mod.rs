// SPDX-License-Identifier: GPL-3.0-or-later
//
// Keystroke injector trait — the abstraction the bench runner uses to *type*
// into a live IM-equipped application once a session is up.
//
// X11 → `xdotool` (this week), Wayland → `ydotool` (Week 4, BEN-31). Tests
// live alongside each implementation; the trait itself is just a contract.
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.4 (injectors).

use async_trait::async_trait;

pub mod xdotool;

pub use xdotool::XdotoolInjector;

/// Contract for every keystroke-injection backend. `type_raw` takes the
/// literal keys to feed the input method — the runner is responsible for
/// mapping test-vector inputs (e.g. "Hell0o" Telex) onto the raw string.
#[async_trait]
pub trait KeystrokeInjector: Send + Sync + std::fmt::Debug {
    /// Short machine-readable id (`"xdotool"`, `"ydotool"`).
    fn id(&self) -> &'static str;

    /// Feed `keys` to the focused window, leaving `ms_per_key` ms between
    /// keystrokes so the IM has time to compose diacritics.
    ///
    /// `keys` is treated as *characters*, not keysyms — uppercase letters
    /// imply a Shift modifier, but special names like `Return` or `BackSpace`
    /// are the injector's responsibility to spell out separately once the
    /// richer `type_sequence` API lands in BEN-13.
    async fn type_raw(&self, keys: &str, ms_per_key: u32) -> Result<(), InjectorError>;
}

/// Every failure mode the Week-1 injectors can hit. Distinct from
/// `SessionError` because the two run at different stages of the pipeline
/// — conflating them would make error messages confusing ("xdotool missing"
/// is not a session failure).
#[derive(Debug, thiserror::Error)]
pub enum InjectorError {
    #[error("binary `{0}` not found on PATH")]
    BinaryMissing(&'static str),

    #[error("{binary} exited with status {code:?}: {stderr}")]
    NonZeroExit { binary: &'static str, code: Option<i32>, stderr: String },

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}
