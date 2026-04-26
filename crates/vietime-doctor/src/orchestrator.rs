// SPDX-License-Identifier: GPL-3.0-or-later
//
// Orchestrator — schedules detectors on a `tokio::task::JoinSet`, enforces
// per-detector timeouts, collects partial facts, and records anomalies for
// any detector that failed, timed out, or panicked.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.1, §B.9.

use std::sync::Arc;
use std::time::Duration;

use futures::future::FutureExt;
use tokio::task::JoinSet;
use tokio::time::timeout;

use vietime_core::{
    ActiveFramework, Anomaly, Facts, ImFacts, Recommendation, Report, SystemFacts,
    REPORT_SCHEMA_VERSION,
};

use crate::checker::{run_checkers, Checker};
use crate::checkers::lookup_recommendation;
use crate::detector::{Detector, DetectorContext, DetectorError, PartialFacts};
use crate::TOOL_VERSION;

/// Tunables for the orchestrator. Defaults match spec/01 §B.9 (total 10s
/// budget); production use passes the defaults, tests tighten them.
#[derive(Debug, Clone, Copy)]
pub struct OrchestratorConfig {
    /// Total wall-clock budget for all detectors combined. A detector can
    /// still claim a shorter `timeout()` of its own — the orchestrator
    /// applies whichever expires first.
    pub total_timeout: Duration,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self { total_timeout: Duration::from_secs(10) }
    }
}

/// An `Orchestrator` owns the detector list and the runtime config.
///
/// Held behind `Arc` so `run_all` can clone the detector into the spawned
/// task — the trait object itself isn't cloneable.
pub struct Orchestrator {
    detectors: Vec<Arc<dyn Detector>>,
    checkers: Vec<Arc<dyn Checker>>,
    config: OrchestratorConfig,
}

impl Orchestrator {
    #[must_use]
    pub fn new(config: OrchestratorConfig) -> Self {
        Self { detectors: Vec::new(), checkers: Vec::new(), config }
    }

    /// Register a detector. Duplicate ids are allowed (the caller's problem);
    /// they will both run and the last-to-complete wins on merge.
    pub fn add(&mut self, detector: Arc<dyn Detector>) -> &mut Self {
        self.detectors.push(detector);
        self
    }

    /// Register a Week-5 checker. Order of registration is preserved for
    /// the `list` subcommand but doesn't affect the final report — issues
    /// are re-sorted by `(severity desc, id asc)` in `run_checkers`.
    pub fn add_checker(&mut self, checker: Arc<dyn Checker>) -> &mut Self {
        self.checkers.push(checker);
        self
    }

    pub fn detectors(&self) -> &[Arc<dyn Detector>] {
        &self.detectors
    }

    pub fn checkers(&self) -> &[Arc<dyn Checker>] {
        &self.checkers
    }

    /// Run every registered detector concurrently, enforce timeouts, and
    /// build a `Report`. Always returns a report — even an empty detector
    /// list produces a valid (if featureless) report.
    pub async fn run(&self, ctx: &DetectorContext) -> Report {
        let (mut partial, anomalies) = run_all(&self.detectors, self.config, ctx).await;

        // Week 3: second pass over the already-merged `PartialFacts` to
        // cross-reference `EngineFact` rows against what the framework
        // detectors observed. This is the 2-pass model promised in the
        // `Detector` trait doc comment — detectors still don't call each
        // other, they just leave enough breadcrumbs for the orchestrator
        // to reconcile.
        reconcile_engine_registration(&mut partial);

        // Week 4: collapse the two independent `AppFacts` rows that DOC-31
        // (`GenericAppDetector`) and DOC-32 (`ElectronAppDetector`) emit
        // for the same `--app <X>` target into a single row per app_id.
        reconcile_app_facts(&mut partial);

        let facts = Facts {
            system: SystemFacts {
                distro: partial.distro,
                desktop: partial.desktop,
                session: partial.session,
                kernel: partial.kernel,
                shell: partial.shell,
            },
            im: ImFacts {
                // `active_framework` is derived in a later phase (checker
                // pass); for now we compute a conservative best guess from
                // what the IM detectors managed to observe.
                active_framework: derive_active(partial.ibus.as_ref(), partial.fcitx5.as_ref()),
                ibus: partial.ibus,
                fcitx5: partial.fcitx5,
                engines: partial.engines,
            },
            env: partial.env.unwrap_or_default(),
            apps: partial.apps,
        };

        // Week 5: checker pass — pure `(&Facts) -> Vec<Issue>` fan-out.
        // Runs after the two reconcile passes so every checker sees the
        // same assembled view that the JSON renderer will.
        let issues = run_checkers(&self.checkers, &facts);
        let recommendations = populate_recommendations(&issues);

        Report {
            schema_version: REPORT_SCHEMA_VERSION,
            generated_at: chrono::Utc::now(),
            tool_version: TOOL_VERSION.to_owned(),
            facts,
            issues,
            recommendations,
            anomalies,
        }
    }
}

/// Collect every unique `VR###` id referenced by at least one issue,
/// resolve it via `checkers::recommendations::lookup`, and return the
/// matching `Recommendation`s sorted by id ascending.
///
/// Unknown VR ids (typo in a checker, or a deferred entry that hasn't
/// shipped yet) are silently dropped — the renderer would produce a
/// broken cross-reference otherwise.
#[must_use]
fn populate_recommendations(issues: &[vietime_core::Issue]) -> Vec<Recommendation> {
    let mut ids: Vec<String> = issues.iter().filter_map(|i| i.recommendation.clone()).collect();
    ids.sort();
    ids.dedup();
    ids.into_iter().filter_map(|id| lookup_recommendation(&id)).collect()
}

impl std::fmt::Debug for Orchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let detector_ids: Vec<&'static str> = self.detectors.iter().map(|d| d.id()).collect();
        let checker_ids: Vec<&'static str> = self.checkers.iter().map(|c| c.id()).collect();
        f.debug_struct("Orchestrator")
            .field("detectors", &detector_ids)
            .field("checkers", &checker_ids)
            .field("config", &self.config)
            .finish()
    }
}

fn derive_active(
    ibus: Option<&vietime_core::IbusFacts>,
    fcitx5: Option<&vietime_core::Fcitx5Facts>,
) -> ActiveFramework {
    let ibus_up = ibus.is_some_and(|f| f.daemon_running);
    let fcitx5_up = fcitx5.is_some_and(|f| f.daemon_running);
    match (ibus_up, fcitx5_up) {
        (true, true) => ActiveFramework::Conflict,
        (true, false) => ActiveFramework::Ibus,
        (false, true) => ActiveFramework::Fcitx5,
        (false, false) => ActiveFramework::None,
    }
}

/// Second-pass reconciliation: flip `is_registered` on any engine that
/// appears in `ibus.registered_engines` or `fcitx5.input_methods_configured`,
/// and push IBus-side registered engines back into
/// `ibus.registered_engines` (so a package detector row in `engines` that
/// was confirmed registered also shows up on the framework facts).
///
/// The loop is intentionally simple — engines are O(dozens) on the most
/// flamboyant desktops, so linear scans are fine and keep the ordering
/// deterministic for snapshots.
fn reconcile_engine_registration(partial: &mut PartialFacts) {
    use vietime_core::ImFramework;

    // Step 1: collect every (framework, name) pair confirmed by a
    // framework-specific detector (DOC-21 / DOC-23) or flagged as
    // registered on an engine row we've already got.
    let mut registered: std::collections::HashSet<(ImFramework, String)> =
        std::collections::HashSet::new();
    if let Some(ibus) = partial.ibus.as_ref() {
        for name in &ibus.registered_engines {
            registered.insert((ImFramework::Ibus, name.clone()));
        }
    }
    if let Some(f) = partial.fcitx5.as_ref() {
        for im in &f.input_methods_configured {
            registered.insert((ImFramework::Fcitx5, im.clone()));
        }
    }
    for e in &partial.engines {
        if e.is_registered {
            registered.insert((e.framework, e.name.clone()));
        }
    }

    // Step 2: flip the flag on any engine row (typically from DOC-24)
    // that matches one of those confirmed pairs but came in with
    // `is_registered = false`.
    for e in &mut partial.engines {
        if !e.is_registered && registered.contains(&(e.framework, e.name.clone())) {
            e.is_registered = true;
        }
    }

    // Step 3: fold IBus-registered engine names back into
    // `ibus.registered_engines` so the framework facts remain the
    // single source of truth for "what did IBus list".
    if let Some(ibus) = partial.ibus.as_mut() {
        for e in &partial.engines {
            if e.framework == ImFramework::Ibus
                && e.is_registered
                && !ibus.registered_engines.contains(&e.name)
            {
                ibus.registered_engines.push(e.name.clone());
            }
        }
    }
}

/// Second-pass: collapse multiple `AppFacts` rows with the same `app_id`
/// into one. DOC-31 and DOC-32 both emit a row for the target app (the
/// first via `--version` / `file`, the second via binary-strings /
/// `/proc`); merge them preferring the non-empty / `Some` side on each
/// field.
///
/// Kept deterministic by iterating in input order and only flipping a
/// previously-set value when the incoming one is strictly more specific
/// (a non-empty path beats an empty one; `Electron` / `Chromium` beats
/// `Native` / hint-level kinds).
fn reconcile_app_facts(partial: &mut PartialFacts) {
    use vietime_core::AppKind;

    let incoming = std::mem::take(&mut partial.apps);
    let mut merged: Vec<vietime_core::AppFacts> = Vec::new();
    for item in incoming {
        if let Some(existing) = merged.iter_mut().find(|a| a.app_id == item.app_id) {
            if existing.binary_path.as_os_str().is_empty()
                && !item.binary_path.as_os_str().is_empty()
            {
                existing.binary_path = item.binary_path;
            }
            if existing.version.is_none() && item.version.is_some() {
                existing.version = item.version;
            }
            if existing.electron_version.is_none() && item.electron_version.is_some() {
                existing.electron_version = item.electron_version;
            }
            if existing.uses_wayland.is_none() && item.uses_wayland.is_some() {
                existing.uses_wayland = item.uses_wayland;
            }
            // A more specific Electron/Chromium/AppImage answer beats a
            // generic Native hint; AppImage also beats Electron (an
            // Electron-wrapped AppImage is still an AppImage to us). If
            // both sides already disagree above Native level and neither
            // upgrade rule fires, leave the earlier one alone —
            // deterministic wins over "whichever happened to land second".
            let upgrade = matches!(
                (&existing.kind, &item.kind),
                (AppKind::Native, AppKind::Electron | AppKind::Chromium | AppKind::AppImage)
                    | (AppKind::Electron, AppKind::AppImage)
            );
            if upgrade {
                existing.kind = item.kind;
            }
            existing.detector_notes.extend(item.detector_notes);
        } else {
            merged.push(item);
        }
    }
    partial.apps = merged;
}

/// Lower-level entry point. Runs every detector in parallel, enforces
/// timeouts, catches panics, and returns the merged `PartialFacts` along
/// with one `Anomaly` per failed detector.
///
/// Exposed at module level (not just via `Orchestrator`) so tests can
/// exercise the scheduling loop without the Report-shaped wrapper.
pub async fn run_all(
    detectors: &[Arc<dyn Detector>],
    config: OrchestratorConfig,
    ctx: &DetectorContext,
) -> (PartialFacts, Vec<Anomaly>) {
    if detectors.is_empty() {
        return (PartialFacts::default(), Vec::new());
    }

    let mut set: JoinSet<(String, Result<crate::detector::DetectorOutput, DetectorError>)> =
        JoinSet::new();

    for detector in detectors {
        let detector = Arc::clone(detector);
        let ctx = ctx.clone();
        let per_timeout = detector.timeout();
        let id = detector.id().to_owned();

        set.spawn(async move {
            // `catch_unwind` turns a detector panic into a typed error so
            // the orchestrator can record an anomaly instead of bubbling
            // up and killing the whole run.
            let fut = std::panic::AssertUnwindSafe(detector.run(&ctx)).catch_unwind();
            let res = match timeout(per_timeout, fut).await {
                Err(_) => Err(DetectorError::Timeout(per_timeout)),
                Ok(Err(_panic)) => Err(DetectorError::Panicked),
                Ok(Ok(res)) => res,
            };
            (id, res)
        });
    }

    // Apply the total-budget timeout around the entire join loop so that
    // one slow detector can't starve the others.
    let collected = timeout(config.total_timeout, collect(&mut set)).await;
    // Whatever didn't finish in time is aborted.
    set.abort_all();

    let (merged, mut anomalies) = collected.unwrap_or_else(|_| {
        (
            PartialFacts::default(),
            vec![Anomaly {
                detector: "orchestrator".to_owned(),
                reason: format!("total detector budget of {:?} exceeded", config.total_timeout),
            }],
        )
    });

    // Stable anomaly order for deterministic snapshot testing.
    anomalies.sort_by(|a, b| a.detector.cmp(&b.detector));
    (merged, anomalies)
}

async fn collect(
    set: &mut JoinSet<(String, Result<crate::detector::DetectorOutput, DetectorError>)>,
) -> (PartialFacts, Vec<Anomaly>) {
    let mut merged = PartialFacts::default();
    let mut anomalies = Vec::new();
    while let Some(join_res) = set.join_next().await {
        let (id, res) = match join_res {
            Ok(pair) => pair,
            Err(join_err) => {
                // JoinError only fires on cancel / task panic escaping
                // our catch_unwind (shouldn't happen, but record it).
                anomalies.push(Anomaly {
                    detector: "unknown".to_owned(),
                    reason: format!("join error: {join_err}"),
                });
                continue;
            }
        };
        match res {
            Ok(output) => merged.merge_from(output.partial),
            Err(err) => anomalies.push(Anomaly { detector: id, reason: err.to_string() }),
        }
    }
    (merged, anomalies)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::detector::DetectorOutput;
    use async_trait::async_trait;
    use vietime_core::SessionType;

    /// A trivial detector that returns a preset value after an optional sleep.
    struct FakeDetector {
        id: &'static str,
        value: Option<SessionType>,
        delay: Duration,
        own_timeout: Duration,
    }

    #[async_trait]
    impl Detector for FakeDetector {
        fn id(&self) -> &'static str {
            self.id
        }
        fn timeout(&self) -> Duration {
            self.own_timeout
        }
        async fn run(&self, _ctx: &DetectorContext) -> crate::detector::DetectorResult {
            tokio::time::sleep(self.delay).await;
            Ok(DetectorOutput {
                partial: PartialFacts { session: self.value, ..PartialFacts::default() },
                notes: vec![],
            })
        }
    }

    /// A detector that panics.
    struct PanicDetector;
    #[async_trait]
    impl Detector for PanicDetector {
        fn id(&self) -> &'static str {
            "panic.me"
        }
        fn timeout(&self) -> Duration {
            Duration::from_millis(200)
        }
        async fn run(&self, _ctx: &DetectorContext) -> crate::detector::DetectorResult {
            // `#[allow]` kept local because the workspace lint forbids
            // `panic!` in production but allows test code to use it.
            #[allow(clippy::panic)]
            {
                panic!("boom");
            }
        }
    }

    fn arcd(d: impl Detector + 'static) -> Arc<dyn Detector> {
        Arc::new(d)
    }

    #[tokio::test]
    async fn empty_detector_list_yields_empty_report() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        let report = orch.run(&DetectorContext::default()).await;
        assert!(report.anomalies.is_empty());
        assert_eq!(report.schema_version, REPORT_SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn passing_detector_result_is_merged_into_facts() {
        let mut orch = Orchestrator::new(OrchestratorConfig::default());
        orch.add(arcd(FakeDetector {
            id: "fake.ok",
            value: Some(SessionType::Wayland),
            delay: Duration::from_millis(0),
            own_timeout: Duration::from_secs(1),
        }));
        let report = orch.run(&DetectorContext::default()).await;
        assert_eq!(report.facts.system.session, Some(SessionType::Wayland));
        assert!(report.anomalies.is_empty());
    }

    #[tokio::test]
    async fn panicking_detector_does_not_kill_siblings() {
        let mut orch = Orchestrator::new(OrchestratorConfig::default());
        orch.add(arcd(PanicDetector));
        orch.add(arcd(FakeDetector {
            id: "fake.ok",
            value: Some(SessionType::Wayland),
            delay: Duration::from_millis(0),
            own_timeout: Duration::from_secs(1),
        }));
        let report = orch.run(&DetectorContext::default()).await;
        assert_eq!(report.facts.system.session, Some(SessionType::Wayland));
        assert_eq!(report.anomalies.len(), 1);
        assert_eq!(report.anomalies[0].detector, "panic.me");
        assert!(report.anomalies[0].reason.contains("panicked"));
    }

    #[tokio::test]
    async fn slow_detector_hits_per_detector_timeout() {
        let mut orch =
            Orchestrator::new(OrchestratorConfig { total_timeout: Duration::from_secs(5) });
        orch.add(arcd(FakeDetector {
            id: "fake.slow",
            value: Some(SessionType::Wayland),
            delay: Duration::from_millis(500),
            own_timeout: Duration::from_millis(50),
        }));
        orch.add(arcd(FakeDetector {
            id: "fake.fast",
            value: Some(SessionType::X11),
            delay: Duration::from_millis(0),
            own_timeout: Duration::from_secs(1),
        }));
        let report = orch.run(&DetectorContext::default()).await;
        // Fast detector still wrote a value; slow one shows as anomaly.
        assert_eq!(report.facts.system.session, Some(SessionType::X11));
        assert!(report
            .anomalies
            .iter()
            .any(|a| a.detector == "fake.slow" && a.reason.contains("timed out")));
    }

    #[test]
    fn active_framework_conflict_when_both_daemons_up() {
        use std::path::PathBuf;
        let ibus = vietime_core::IbusFacts {
            version: None,
            daemon_running: true,
            daemon_pid: Some(1),
            config_dir: Some(PathBuf::from("/tmp")),
            registered_engines: vec![],
        };
        let fcitx5 = vietime_core::Fcitx5Facts {
            version: None,
            daemon_running: true,
            daemon_pid: Some(2),
            config_dir: Some(PathBuf::from("/tmp")),
            addons_enabled: vec![],
            input_methods_configured: vec![],
        };
        assert_eq!(derive_active(Some(&ibus), Some(&fcitx5)), ActiveFramework::Conflict);
    }

    #[test]
    fn active_framework_none_when_no_daemons() {
        assert_eq!(derive_active(None, None), ActiveFramework::None);
    }

    #[test]
    fn reconcile_flips_registered_when_package_matches_ibus_listing() {
        use std::path::PathBuf;
        use vietime_core::{EngineFact, ImFramework};

        // DOC-21 says ibus listed bamboo.
        let ibus = vietime_core::IbusFacts {
            version: None,
            daemon_running: true,
            daemon_pid: None,
            config_dir: Some(PathBuf::from("/home/a/.config/ibus")),
            registered_engines: vec!["bamboo".to_owned()],
        };
        // DOC-24 reported two installed packages, neither marked registered.
        let engines = vec![
            EngineFact {
                name: "bamboo".to_owned(),
                package: Some("ibus-bamboo".to_owned()),
                version: Some("0.8.2".to_owned()),
                framework: ImFramework::Ibus,
                is_vietnamese: true,
                is_registered: false,
            },
            EngineFact {
                name: "unikey".to_owned(),
                package: Some("ibus-unikey".to_owned()),
                version: Some("0.6.1".to_owned()),
                framework: ImFramework::Ibus,
                is_vietnamese: true,
                is_registered: false,
            },
        ];

        let mut partial = PartialFacts { ibus: Some(ibus), engines, ..PartialFacts::default() };
        reconcile_engine_registration(&mut partial);

        // bamboo → confirmed registered; unikey stays unregistered (classic VD005 signal).
        assert!(partial.engines[0].is_registered);
        assert!(!partial.engines[1].is_registered);
        // `registered_engines` is unchanged (bamboo was already there).
        assert_eq!(partial.ibus.as_ref().expect("ibus set").registered_engines, vec!["bamboo"]);
    }

    #[test]
    fn reconcile_pushes_new_engines_into_ibus_registered_list() {
        use std::path::PathBuf;
        use vietime_core::{EngineFact, ImFramework};

        // DOC-20 produced a daemon-only IbusFacts (no engines listed).
        let ibus = vietime_core::IbusFacts {
            version: Some("1.5.29".to_owned()),
            daemon_running: true,
            daemon_pid: Some(1),
            config_dir: Some(PathBuf::from("/home/a/.config/ibus")),
            registered_engines: vec![],
        };
        // DOC-21 already marked bamboo as registered via its own EngineFact.
        let engines = vec![EngineFact {
            name: "bamboo".to_owned(),
            package: None,
            version: None,
            framework: ImFramework::Ibus,
            is_vietnamese: true,
            is_registered: true,
        }];

        let mut partial = PartialFacts { ibus: Some(ibus), engines, ..PartialFacts::default() };
        reconcile_engine_registration(&mut partial);

        // After reconciliation the framework facts learn about the engine.
        assert_eq!(partial.ibus.as_ref().expect("ibus set").registered_engines, vec!["bamboo"]);
    }

    #[test]
    fn reconcile_app_facts_merges_two_rows_for_same_app_id() {
        use std::path::PathBuf;
        use vietime_core::{AppFacts, AppKind};

        // DOC-31 wrote first: found the binary, parsed version, kind=Electron.
        let generic = AppFacts {
            app_id: "vscode".to_owned(),
            binary_path: PathBuf::from("/usr/bin/code"),
            version: Some("1.87.2".to_owned()),
            kind: AppKind::Electron,
            electron_version: None,
            uses_wayland: None,
            detector_notes: vec!["generic: ok".to_owned()],
        };
        // DOC-32 wrote second: filled in electron_version and uses_wayland.
        let electron = AppFacts {
            app_id: "vscode".to_owned(),
            binary_path: PathBuf::from("/usr/bin/code"),
            version: None,
            kind: AppKind::Electron,
            electron_version: Some("28.2.4".to_owned()),
            uses_wayland: Some(true),
            detector_notes: vec!["electron: ok".to_owned()],
        };
        let mut partial = PartialFacts { apps: vec![generic, electron], ..PartialFacts::default() };
        reconcile_app_facts(&mut partial);
        assert_eq!(partial.apps.len(), 1);
        let a = &partial.apps[0];
        assert_eq!(a.app_id, "vscode");
        assert_eq!(a.version.as_deref(), Some("1.87.2"));
        assert_eq!(a.electron_version.as_deref(), Some("28.2.4"));
        assert_eq!(a.uses_wayland, Some(true));
        assert_eq!(a.detector_notes.len(), 2);
    }

    #[test]
    fn reconcile_app_facts_leaves_single_row_unchanged() {
        use std::path::PathBuf;
        use vietime_core::{AppFacts, AppKind};

        let only = AppFacts {
            app_id: "firefox".to_owned(),
            binary_path: PathBuf::from("/usr/bin/firefox"),
            version: Some("126.0".to_owned()),
            kind: AppKind::Native,
            electron_version: None,
            uses_wayland: None,
            detector_notes: vec![],
        };
        let mut partial = PartialFacts { apps: vec![only.clone()], ..PartialFacts::default() };
        reconcile_app_facts(&mut partial);
        assert_eq!(partial.apps, vec![only]);
    }

    #[test]
    fn reconcile_uses_fcitx5_profile_to_flip_package_engine_flag() {
        use std::path::PathBuf;
        use vietime_core::{EngineFact, ImFramework};

        let fcitx5 = vietime_core::Fcitx5Facts {
            version: None,
            daemon_running: true,
            daemon_pid: None,
            config_dir: Some(PathBuf::from("/home/a/.config/fcitx5")),
            addons_enabled: vec![],
            input_methods_configured: vec!["bamboo".to_owned(), "keyboard-us".to_owned()],
        };
        let engines = vec![EngineFact {
            name: "bamboo".to_owned(),
            package: Some("fcitx5-bamboo".to_owned()),
            version: None,
            framework: ImFramework::Fcitx5,
            is_vietnamese: true,
            is_registered: false,
        }];
        let mut partial = PartialFacts { fcitx5: Some(fcitx5), engines, ..PartialFacts::default() };
        reconcile_engine_registration(&mut partial);
        assert!(partial.engines[0].is_registered);
    }

    // ──────────────────────────────────────────────────────────────
    // Week 5: end-to-end orchestrator + checker integration.
    // ──────────────────────────────────────────────────────────────

    /// Minimal detector that plants both IBus and Fcitx5 as running —
    /// triggers VD002 `ImFrameworkConflict` through the full orchestrator
    /// pipeline (detector → reconcile → derive_active → checker).
    struct BothDaemonsDetector;
    #[async_trait]
    impl Detector for BothDaemonsDetector {
        fn id(&self) -> &'static str {
            "test.both_daemons"
        }
        fn timeout(&self) -> Duration {
            Duration::from_millis(500)
        }
        async fn run(&self, _ctx: &DetectorContext) -> crate::detector::DetectorResult {
            use std::path::PathBuf;
            Ok(DetectorOutput {
                partial: PartialFacts {
                    ibus: Some(vietime_core::IbusFacts {
                        version: None,
                        daemon_running: true,
                        daemon_pid: Some(1),
                        config_dir: Some(PathBuf::from("/tmp/ibus")),
                        registered_engines: vec![],
                    }),
                    fcitx5: Some(vietime_core::Fcitx5Facts {
                        version: None,
                        daemon_running: true,
                        daemon_pid: Some(2),
                        config_dir: Some(PathBuf::from("/tmp/fcitx5")),
                        addons_enabled: vec![],
                        input_methods_configured: vec![],
                    }),
                    ..PartialFacts::default()
                },
                notes: vec![],
            })
        }
    }

    #[tokio::test]
    async fn orchestrator_runs_checkers_and_populates_issues() {
        use crate::checkers::Vd002;
        let mut orch = Orchestrator::new(OrchestratorConfig::default());
        orch.add(Arc::new(BothDaemonsDetector));
        orch.add_checker(Arc::new(Vd002));
        let report = orch.run(&DetectorContext::default()).await;
        assert!(
            report.issues.iter().any(|i| i.id == "VD002"),
            "expected VD002 to fire, got: {:?}",
            report.issues
        );
        assert!(
            report.recommendations.iter().any(|r| r.id == "VR002"),
            "expected VR002 to be populated, got: {:?}",
            report.recommendations
        );
    }

    #[tokio::test]
    async fn orchestrator_leaves_issues_empty_when_no_checker_fires() {
        use crate::checkers::Vd001;
        // No detectors at all — `facts` is empty; VD001 needs a VN engine,
        // so it stays silent.
        let mut orch = Orchestrator::new(OrchestratorConfig::default());
        orch.add_checker(Arc::new(Vd001));
        let report = orch.run(&DetectorContext::default()).await;
        assert!(report.issues.is_empty());
        assert!(report.recommendations.is_empty());
    }

    #[test]
    fn populate_recommendations_dedupes_and_sorts_vr_ids() {
        use vietime_core::{Issue, Severity};
        let issues = vec![
            Issue {
                id: "VD005".to_owned(),
                severity: Severity::Warn,
                title: "x".to_owned(),
                detail: String::new(),
                facts_evidence: vec![],
                recommendation: Some("VR005".to_owned()),
            },
            Issue {
                id: "VD005".to_owned(),
                severity: Severity::Warn,
                title: "y".to_owned(),
                detail: String::new(),
                facts_evidence: vec![],
                recommendation: Some("VR005".to_owned()),
            },
            Issue {
                id: "VD001".to_owned(),
                severity: Severity::Critical,
                title: "z".to_owned(),
                detail: String::new(),
                facts_evidence: vec![],
                recommendation: Some("VR001".to_owned()),
            },
        ];
        let recs = populate_recommendations(&issues);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].id, "VR001");
        assert_eq!(recs[1].id, "VR005");
    }

    #[test]
    fn populate_recommendations_skips_info_only_issues() {
        use vietime_core::{Issue, Severity};
        let issues = vec![Issue {
            id: "VD012".to_owned(),
            severity: Severity::Info,
            title: "x".to_owned(),
            detail: String::new(),
            facts_evidence: vec![],
            recommendation: None,
        }];
        let recs = populate_recommendations(&issues);
        assert!(recs.is_empty());
    }
}
