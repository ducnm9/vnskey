// SPDX-License-Identifier: GPL-3.0-or-later
//
// The `Detector` trait — the primitive every fact-gathering task in Doctor
// implements.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.1, §B.3 (trait + rules).
//
// Design rules distilled from spec/01 §B.3:
//
// 1. Detectors are **read-only** — they never write files or call sudo.
// 2. Every detector has a timeout. `Orchestrator` enforces it.
// 3. A detector failure must not crash the run; the orchestrator records an
//    `Anomaly` and keeps going.
// 4. Detectors don't call each other. Cross-detector dependencies run as a
//    second pass over the already-collected `Facts` (2-pass model in §B.1).

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;

use vietime_core::{AppFacts, DesktopEnv, Distro, EnvFacts, Fcitx5Facts, IbusFacts, SessionType};

/// Environment snapshot + config passed to every detector.
///
/// `env` is a *copy* of the relevant subset of `/proc/self/environ` — not a
/// live handle — so tests can supply an empty or arbitrary map without
/// polluting the real process environment.
#[derive(Debug, Clone, Default)]
pub struct DetectorContext {
    pub env: HashMap<String, String>,
    /// When set, detectors that read files should root their paths here
    /// instead of `/`. Keeps tests hermetic.
    pub sysroot: Option<std::path::PathBuf>,
}

impl DetectorContext {
    /// Construct a context populated from the current process environment.
    /// Only used by the real CLI, never tests.
    #[must_use]
    pub fn from_current_process() -> Self {
        let env = std::env::vars().collect();
        Self { env, sysroot: None }
    }
}

/// A partial contribution to `Facts`. Each detector emits one; the
/// orchestrator merges them in id order.
///
/// Every field is `Option<_>` or a `Vec` so that detector composition is
/// a matter of "last non-None wins" / "concat the lists". No detector ever
/// sees the merged `Facts` during its own run (that would violate rule 4
/// above).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartialFacts {
    pub distro: Option<Distro>,
    pub desktop: Option<DesktopEnv>,
    pub session: Option<SessionType>,
    pub kernel: Option<String>,
    pub shell: Option<String>,
    pub ibus: Option<IbusFacts>,
    pub fcitx5: Option<Fcitx5Facts>,
    pub env: Option<EnvFacts>,
    pub apps: Vec<AppFacts>,
}

/// Output of a single detector run: data + free-form notes + timing.
#[derive(Debug, Clone, Default)]
pub struct DetectorOutput {
    pub partial: PartialFacts,
    /// Short, rendered verbatim in `--verbose` mode.
    pub notes: Vec<String>,
}

/// The detector contract.
///
/// Async because several Phase 1 detectors will make D-Bus calls and spawn
/// subprocesses (`ibus list-engine`). Pure-sync detectors like
/// `DistroDetector` simply ignore the `.await`.
#[async_trait]
pub trait Detector: Send + Sync {
    /// Stable identifier (e.g. `"sys.distro"`). Used for anomaly reporting
    /// and to suppress duplicate runs.
    fn id(&self) -> &'static str;

    /// Maximum wall-clock time before the orchestrator gives up.
    /// Default 3s per spec/01 §B.3.
    fn timeout(&self) -> Duration {
        Duration::from_secs(3)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult;
}

/// Success / failure result. We use a dedicated `thiserror` enum instead of
/// `anyhow` so the orchestrator can distinguish timeouts from real errors
/// without string-matching.
#[derive(Debug, thiserror::Error)]
pub enum DetectorError {
    #[error("detector timed out after {0:?}")]
    Timeout(Duration),
    #[error("detector panicked")]
    Panicked,
    #[error("{0}")]
    Other(String),
}

pub type DetectorResult = Result<DetectorOutput, DetectorError>;

impl PartialFacts {
    /// Merge `other` into `self`, with `other` winning on scalar fields and
    /// `apps` being concatenated. Detectors run in a fixed order; the merge
    /// is applied in that order so later detectors can override earlier
    /// ones for the same field.
    ///
    /// We deliberately never replace a `Some` with a `None` — missing data
    /// from a later detector should not erase data a previous detector
    /// successfully found.
    pub fn merge_from(&mut self, other: Self) {
        if other.distro.is_some() {
            self.distro = other.distro;
        }
        if other.desktop.is_some() {
            self.desktop = other.desktop;
        }
        if other.session.is_some() {
            self.session = other.session;
        }
        if other.kernel.is_some() {
            self.kernel = other.kernel;
        }
        if other.shell.is_some() {
            self.shell = other.shell;
        }
        if other.ibus.is_some() {
            self.ibus = other.ibus;
        }
        if other.fcitx5.is_some() {
            self.fcitx5 = other.fcitx5;
        }
        if other.env.is_some() {
            self.env = other.env;
        }
        self.apps.extend(other.apps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vietime_core::DistroFamily;

    #[test]
    fn merge_keeps_earlier_value_when_later_is_none() {
        let mut base =
            PartialFacts { session: Some(SessionType::Wayland), ..PartialFacts::default() };
        let later = PartialFacts::default();
        base.merge_from(later);
        assert_eq!(base.session, Some(SessionType::Wayland));
    }

    #[test]
    fn merge_overwrites_when_later_is_some() {
        let mut base = PartialFacts { session: Some(SessionType::X11), ..PartialFacts::default() };
        let later = PartialFacts { session: Some(SessionType::Wayland), ..PartialFacts::default() };
        base.merge_from(later);
        assert_eq!(base.session, Some(SessionType::Wayland));
    }

    #[test]
    fn merge_concatenates_apps() {
        let mut base = PartialFacts::default();
        base.apps.push(AppFacts {
            app_id: "vscode".to_owned(),
            binary_path: std::path::PathBuf::from("/usr/bin/code"),
            version: None,
            kind: vietime_core::AppKind::Electron,
            electron_version: None,
            uses_wayland: None,
            detector_notes: vec![],
        });
        let later = PartialFacts {
            apps: vec![AppFacts {
                app_id: "chrome".to_owned(),
                binary_path: std::path::PathBuf::from("/usr/bin/google-chrome"),
                version: None,
                kind: vietime_core::AppKind::Chromium,
                electron_version: None,
                uses_wayland: None,
                detector_notes: vec![],
            }],
            ..PartialFacts::default()
        };
        base.merge_from(later);
        assert_eq!(base.apps.len(), 2);
        assert_eq!(base.apps[0].app_id, "vscode");
        assert_eq!(base.apps[1].app_id, "chrome");
    }

    #[test]
    fn detector_context_from_process_populates_env() {
        // Invariant: PATH is always set in any sane CI environment, so
        // `from_current_process()` must at least produce a non-empty map.
        let ctx = DetectorContext::from_current_process();
        assert!(!ctx.env.is_empty(), "env should not be empty");
    }

    #[test]
    fn distro_family_still_usable_from_this_crate() {
        // Sanity check — re-exports from vietime-core are in scope.
        let f = DistroFamily::Debian;
        assert_eq!(f, DistroFamily::Debian);
    }
}
