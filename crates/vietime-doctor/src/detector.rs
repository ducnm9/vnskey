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

use vietime_core::{
    AppFacts, DesktopEnv, Distro, EngineFact, EnvFacts, Fcitx5Facts, IbusFacts, SessionType,
};

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
    /// Raw `--app <X>` argument value when the CLI was invoked with one.
    /// Week-4 app detectors (DOC-31 / DOC-32) early-return when this is
    /// `None`; base-layer (Week 1-3) detectors never read it.
    pub target_app: Option<String>,
}

impl DetectorContext {
    /// Construct a context populated from the current process environment.
    /// Only used by the real CLI, never tests.
    #[must_use]
    pub fn from_current_process() -> Self {
        let env = std::env::vars().collect();
        Self { env, sysroot: None, target_app: None }
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
    /// Effective locale string (e.g. `"en_US.UTF-8"`). Populated by the
    /// Week-6 `LocaleDetector`.
    pub locale: Option<String>,
    pub ibus: Option<IbusFacts>,
    pub fcitx5: Option<Fcitx5Facts>,
    pub env: Option<EnvFacts>,
    pub apps: Vec<AppFacts>,
    /// Engines discovered by the framework-specific list detectors (DOC-21)
    /// *and* the package-enumeration detector (DOC-24). Entries are additive:
    /// the orchestrator concatenates contributions from every detector and
    /// only deduplicates at the checker layer. `is_registered` is then
    /// reconciled against `ibus.registered_engines` and
    /// `fcitx5.input_methods_configured` in the orchestrator — see
    /// `Orchestrator::reconcile_engine_registration`.
    pub engines: Vec<EngineFact>,
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
        // IBus / Fcitx5 facts are built by *two* detectors apiece:
        // DOC-20/22 produce the daemon side (version, pid, daemon_running)
        // and DOC-23 produces the config side (addons_enabled,
        // input_methods_configured). Last-wins would clobber one or the
        // other depending on `JoinSet` completion order, so we merge
        // field-by-field with a "non-empty incoming wins" rule.
        merge_ibus_facts(&mut self.ibus, other.ibus);
        merge_fcitx5_facts(&mut self.fcitx5, other.fcitx5);
        // Env facts merge by per-field source priority — `Process` always
        // wins over `EtcEnvironment` regardless of which detector
        // completed first in the JoinSet.
        match (self.env.as_mut(), other.env) {
            (None, Some(incoming)) => self.env = Some(incoming),
            (Some(current), Some(incoming)) => current.merge_by_priority(&incoming),
            _ => {}
        }
        self.apps.extend(other.apps);
        self.engines.extend(other.engines);
    }
}

/// Element-wise merge: keep non-default / non-empty fields from either side,
/// preferring `incoming` for values only it provides.
fn merge_ibus_facts(current: &mut Option<IbusFacts>, incoming: Option<IbusFacts>) {
    let Some(incoming) = incoming else { return };
    let Some(cur) = current.as_mut() else {
        *current = Some(incoming);
        return;
    };
    if cur.version.is_none() && incoming.version.is_some() {
        cur.version = incoming.version;
    }
    // Daemon-running is monotonic: any detector that saw it running wins.
    if incoming.daemon_running {
        cur.daemon_running = true;
    }
    if cur.daemon_pid.is_none() && incoming.daemon_pid.is_some() {
        cur.daemon_pid = incoming.daemon_pid;
    }
    if cur.config_dir.is_none() && incoming.config_dir.is_some() {
        cur.config_dir = incoming.config_dir;
    }
    if cur.registered_engines.is_empty() && !incoming.registered_engines.is_empty() {
        cur.registered_engines = incoming.registered_engines;
    }
}

fn merge_fcitx5_facts(current: &mut Option<Fcitx5Facts>, incoming: Option<Fcitx5Facts>) {
    let Some(incoming) = incoming else { return };
    let Some(cur) = current.as_mut() else {
        *current = Some(incoming);
        return;
    };
    if cur.version.is_none() && incoming.version.is_some() {
        cur.version = incoming.version;
    }
    if incoming.daemon_running {
        cur.daemon_running = true;
    }
    if cur.daemon_pid.is_none() && incoming.daemon_pid.is_some() {
        cur.daemon_pid = incoming.daemon_pid;
    }
    if cur.config_dir.is_none() && incoming.config_dir.is_some() {
        cur.config_dir = incoming.config_dir;
    }
    if cur.addons_enabled.is_empty() && !incoming.addons_enabled.is_empty() {
        cur.addons_enabled = incoming.addons_enabled;
    }
    if cur.input_methods_configured.is_empty() && !incoming.input_methods_configured.is_empty() {
        cur.input_methods_configured = incoming.input_methods_configured;
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
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
    fn merge_env_preserves_higher_priority_source() {
        use vietime_core::{EnvFacts, EnvSource};
        let mut process_env = HashMap::new();
        process_env.insert("GTK_IM_MODULE".to_owned(), "ibus".to_owned());
        let high = EnvFacts::from_env_with_source(&process_env, EnvSource::Process);

        let mut etc_env = HashMap::new();
        etc_env.insert("GTK_IM_MODULE".to_owned(), "fcitx".to_owned());
        let low = EnvFacts::from_env_with_source(&etc_env, EnvSource::EtcEnvironment);

        // Process-sourced value lands first...
        let mut base = PartialFacts { env: Some(high), ..PartialFacts::default() };
        // ...and is NOT overwritten by the lower-priority etc/environment value
        // even though `other` is last in merge order.
        let later = PartialFacts { env: Some(low), ..PartialFacts::default() };
        base.merge_from(later);

        let env = base.env.expect("env survived the merge");
        assert_eq!(env.gtk_im_module.as_deref(), Some("ibus"));
        assert_eq!(env.sources.get("GTK_IM_MODULE"), Some(&EnvSource::Process));
    }

    #[test]
    fn merge_env_takes_incoming_when_base_has_none() {
        use vietime_core::{EnvFacts, EnvSource};
        let mut etc_env = HashMap::new();
        etc_env.insert("GTK_IM_MODULE".to_owned(), "fcitx".to_owned());
        let incoming = EnvFacts::from_env_with_source(&etc_env, EnvSource::EtcEnvironment);

        let mut base = PartialFacts::default();
        base.merge_from(PartialFacts { env: Some(incoming), ..PartialFacts::default() });

        let env = base.env.expect("env now set");
        assert_eq!(env.gtk_im_module.as_deref(), Some("fcitx"));
        assert_eq!(env.sources.get("GTK_IM_MODULE"), Some(&EnvSource::EtcEnvironment));
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

    #[test]
    fn merge_ibus_fills_empty_fields_from_incoming() {
        // Base has the daemon facts (DOC-20 output) but no registered engines.
        let base_ibus = IbusFacts {
            version: Some("1.5.29".to_owned()),
            daemon_running: true,
            daemon_pid: Some(2341),
            config_dir: None,
            registered_engines: vec![],
        };
        let mut base = PartialFacts { ibus: Some(base_ibus), ..PartialFacts::default() };
        // Incoming carries engines (DOC-21 output) but no daemon facts.
        let incoming_ibus = IbusFacts {
            version: None,
            daemon_running: false,
            daemon_pid: None,
            config_dir: None,
            registered_engines: vec!["bamboo".to_owned(), "xkb:us::eng".to_owned()],
        };
        base.merge_from(PartialFacts { ibus: Some(incoming_ibus), ..PartialFacts::default() });

        let ibus = base.ibus.expect("ibus merged");
        // Daemon facts preserved from base.
        assert_eq!(ibus.version.as_deref(), Some("1.5.29"));
        assert!(ibus.daemon_running);
        assert_eq!(ibus.daemon_pid, Some(2341));
        // Engines pulled in from incoming.
        assert_eq!(ibus.registered_engines, vec!["bamboo", "xkb:us::eng"]);
    }

    #[test]
    fn merge_fcitx5_keeps_base_lists_when_incoming_is_empty() {
        // DOC-23 seed: lists populated, version/pid absent.
        let base_fcitx5 = Fcitx5Facts {
            version: None,
            daemon_running: false,
            daemon_pid: None,
            config_dir: Some(std::path::PathBuf::from("/home/alice/.config/fcitx5")),
            addons_enabled: vec!["unicode".to_owned()],
            input_methods_configured: vec!["bamboo".to_owned()],
        };
        let mut base = PartialFacts { fcitx5: Some(base_fcitx5), ..PartialFacts::default() };
        // DOC-22 arrives later with daemon facts but no config lists.
        let incoming_fcitx5 = Fcitx5Facts {
            version: Some("5.1.12".to_owned()),
            daemon_running: true,
            daemon_pid: Some(777),
            config_dir: None,
            addons_enabled: vec![],
            input_methods_configured: vec![],
        };
        base.merge_from(PartialFacts { fcitx5: Some(incoming_fcitx5), ..PartialFacts::default() });

        let f = base.fcitx5.expect("fcitx5 merged");
        // Incoming daemon facts took effect where base had none.
        assert_eq!(f.version.as_deref(), Some("5.1.12"));
        assert!(f.daemon_running);
        assert_eq!(f.daemon_pid, Some(777));
        // Base's non-empty lists were preserved.
        assert_eq!(f.addons_enabled, vec!["unicode"]);
        assert_eq!(f.input_methods_configured, vec!["bamboo"]);
        assert_eq!(
            f.config_dir.as_deref(),
            Some(std::path::Path::new("/home/alice/.config/fcitx5"))
        );
    }

    #[test]
    fn merge_ibus_adopts_incoming_when_base_is_none() {
        let mut base = PartialFacts::default();
        let incoming = IbusFacts {
            version: Some("1.5.29".to_owned()),
            daemon_running: true,
            daemon_pid: Some(100),
            config_dir: None,
            registered_engines: vec!["bamboo".to_owned()],
        };
        base.merge_from(PartialFacts { ibus: Some(incoming), ..PartialFacts::default() });
        let ibus = base.ibus.expect("ibus now set");
        assert_eq!(ibus.version.as_deref(), Some("1.5.29"));
        assert!(ibus.daemon_running);
        assert_eq!(ibus.registered_engines, vec!["bamboo"]);
    }

    #[test]
    fn merge_concatenates_engines() {
        use vietime_core::ImFramework;
        let mut base = PartialFacts::default();
        base.engines.push(EngineFact {
            name: "bamboo".to_owned(),
            package: None,
            version: None,
            framework: ImFramework::Ibus,
            is_vietnamese: true,
            is_registered: true,
        });
        let later = PartialFacts {
            engines: vec![EngineFact {
                name: "unikey".to_owned(),
                package: Some("ibus-unikey".to_owned()),
                version: Some("0.6.1".to_owned()),
                framework: ImFramework::Ibus,
                is_vietnamese: true,
                is_registered: false,
            }],
            ..PartialFacts::default()
        };
        base.merge_from(later);
        assert_eq!(base.engines.len(), 2);
        assert_eq!(base.engines[0].name, "bamboo");
        assert_eq!(base.engines[1].name, "unikey");
    }
}
