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
    ActiveFramework, Anomaly, Facts, ImFacts, Report, SystemFacts, REPORT_SCHEMA_VERSION,
};

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
    config: OrchestratorConfig,
}

impl Orchestrator {
    #[must_use]
    pub fn new(config: OrchestratorConfig) -> Self {
        Self { detectors: Vec::new(), config }
    }

    /// Register a detector. Duplicate ids are allowed (the caller's problem);
    /// they will both run and the last-to-complete wins on merge.
    pub fn add(&mut self, detector: Arc<dyn Detector>) -> &mut Self {
        self.detectors.push(detector);
        self
    }

    pub fn detectors(&self) -> &[Arc<dyn Detector>] {
        &self.detectors
    }

    /// Run every registered detector concurrently, enforce timeouts, and
    /// build a `Report`. Always returns a report — even an empty detector
    /// list produces a valid (if featureless) report.
    pub async fn run(&self, ctx: &DetectorContext) -> Report {
        let (partial, anomalies) = run_all(&self.detectors, self.config, ctx).await;

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
                engines: Vec::new(),
            },
            env: partial.env.unwrap_or_default(),
            apps: partial.apps,
        };

        Report {
            schema_version: REPORT_SCHEMA_VERSION,
            generated_at: chrono::Utc::now(),
            tool_version: TOOL_VERSION.to_owned(),
            facts,
            issues: Vec::new(),
            recommendations: Vec::new(),
            anomalies,
        }
    }
}

impl std::fmt::Debug for Orchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<&'static str> = self.detectors.iter().map(|d| d.id()).collect();
        f.debug_struct("Orchestrator")
            .field("detectors", &ids)
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
}
