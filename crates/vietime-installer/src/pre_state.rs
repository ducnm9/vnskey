// SPDX-License-Identifier: GPL-3.0-or-later
//
// `PreState` — snapshot of the host system the Planner reasons about.
//
// `PreState` is a pure data struct. It's built either from a Doctor `Report`
// (the real CLI path) or from a fixture (tests). The Planner never reads
// the filesystem; the orchestrator behind `detect_pre_state()` does that
// work once, up-front.
//
// Spec ref: `spec/02-phase2-installer.md` §B.2.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use vietime_core::{
    ActiveFramework, DesktopEnv, Distro, DistroFamily, EnvFacts, Report, SessionType,
};

use vietime_doctor::detector::{Detector, DetectorContext};
use vietime_doctor::detectors::{DesktopDetector, DistroDetector, SessionDetector};
use vietime_doctor::{Orchestrator, OrchestratorConfig};

/// Everything the Planner needs to know about the host before it builds a
/// Plan. Each field is `Option<_>` where the Doctor detector can legitimately
/// fail to produce a value (e.g. exotic distro with no `/etc/os-release`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreState {
    pub distro: Distro,
    #[serde(default)]
    pub desktop: Option<DesktopEnv>,
    pub session: SessionType,
    #[serde(default)]
    pub active_framework: ActiveFramework,
    pub env: EnvFacts,
    /// Populated by `PackageOps::is_installed` once INS-20 lands.
    /// Left empty in Week 1.
    #[serde(default)]
    pub installed_packages: Vec<String>,
}

impl PreState {
    /// Compress a Doctor `Report` down to the fields the Planner needs.
    #[must_use]
    pub fn from_report(report: &Report) -> Self {
        Self {
            distro: report.facts.system.distro.clone().unwrap_or_else(Distro::unknown),
            desktop: report.facts.system.desktop.clone(),
            session: report.facts.system.session.unwrap_or(SessionType::Unknown),
            active_framework: report.facts.im.active_framework,
            env: report.facts.env.clone(),
            installed_packages: Vec::new(),
        }
    }

    /// Ubuntu 24.04 Wayland, IBus-running — the canonical fixture the golden
    /// planner tests use. Keeping it as an associated function means tests
    /// across modules don't each have to construct it from scratch.
    #[must_use]
    pub fn fixture_ubuntu_24_04() -> Self {
        Self {
            distro: Distro {
                id: "ubuntu".to_owned(),
                version_id: Some("24.04".to_owned()),
                pretty: Some("Ubuntu 24.04 LTS".to_owned()),
                family: DistroFamily::Debian,
                id_like: vec!["debian".to_owned()],
            },
            desktop: Some(DesktopEnv::Gnome { version: None }),
            session: SessionType::Wayland,
            active_framework: ActiveFramework::Ibus,
            env: EnvFacts::default(),
            installed_packages: Vec::new(),
        }
    }

    /// Fedora 40 GNOME Wayland, no IM active. Used to test "unsupported distro
    /// family" paths in the Planner.
    #[must_use]
    pub fn fixture_fedora_40() -> Self {
        Self {
            distro: Distro {
                id: "fedora".to_owned(),
                version_id: Some("40".to_owned()),
                pretty: Some("Fedora Linux 40 (Workstation Edition)".to_owned()),
                family: DistroFamily::Redhat,
                id_like: Vec::new(),
            },
            desktop: Some(DesktopEnv::Gnome { version: None }),
            session: SessionType::Wayland,
            active_framework: ActiveFramework::None,
            env: EnvFacts::default(),
            installed_packages: Vec::new(),
        }
    }
}

/// Drive the Doctor orchestrator against the current process environment,
/// then compress the resulting `Report` down to a `PreState`.
///
/// Only called from the real CLI; tests use `PreState::from_report` against
/// a hand-built `Report` or one of the `fixture_*` constructors.
pub async fn detect_pre_state() -> PreState {
    let mut orch = Orchestrator::new(OrchestratorConfig::default());
    let detectors: Vec<Arc<dyn Detector>> = vec![
        Arc::new(DistroDetector::new()),
        Arc::new(SessionDetector::new()),
        Arc::new(DesktopDetector::new()),
    ];
    for d in detectors {
        orch.add(d);
    }
    let ctx = DetectorContext::from_current_process();
    let report = orch.run(&ctx).await;
    PreState::from_report(&report)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use vietime_core::{Facts, ImFacts, SystemFacts, REPORT_SCHEMA_VERSION};

    fn report_with(system: SystemFacts, im: ImFacts) -> Report {
        Report {
            schema_version: REPORT_SCHEMA_VERSION,
            generated_at: chrono::Utc::now(),
            tool_version: "test".to_owned(),
            facts: Facts { system, im, env: EnvFacts::default(), apps: Vec::new() },
            issues: Vec::new(),
            recommendations: Vec::new(),
            anomalies: Vec::new(),
        }
    }

    #[test]
    fn from_report_maps_each_field() {
        let distro = Distro {
            id: "ubuntu".to_owned(),
            version_id: Some("24.04".to_owned()),
            pretty: Some("Ubuntu 24.04 LTS".to_owned()),
            family: DistroFamily::Debian,
            id_like: vec!["debian".to_owned()],
        };
        let system = SystemFacts {
            distro: Some(distro.clone()),
            desktop: Some(DesktopEnv::Gnome { version: None }),
            session: Some(SessionType::Wayland),
            kernel: None,
            shell: None,
        };
        let im = ImFacts {
            active_framework: ActiveFramework::Ibus,
            ibus: None,
            fcitx5: None,
            engines: Vec::new(),
        };
        let pre = PreState::from_report(&report_with(system, im));
        assert_eq!(pre.distro, distro);
        assert_eq!(pre.desktop, Some(DesktopEnv::Gnome { version: None }));
        assert_eq!(pre.session, SessionType::Wayland);
        assert_eq!(pre.active_framework, ActiveFramework::Ibus);
        assert!(pre.installed_packages.is_empty());
    }

    #[test]
    fn from_report_falls_back_to_unknown_distro_on_missing_facts() {
        let pre = PreState::from_report(&report_with(SystemFacts::default(), ImFacts::default()));
        assert_eq!(pre.distro.family, DistroFamily::Unknown);
        assert_eq!(pre.session, SessionType::Unknown);
        assert_eq!(pre.active_framework, ActiveFramework::None);
    }

    #[test]
    fn fixture_ubuntu_24_04_is_debian_family() {
        let pre = PreState::fixture_ubuntu_24_04();
        assert!(pre.distro.is_family(DistroFamily::Debian));
        assert_eq!(pre.session, SessionType::Wayland);
    }

    #[test]
    fn fixture_fedora_is_redhat_family() {
        let pre = PreState::fixture_fedora_40();
        assert!(pre.distro.is_family(DistroFamily::Redhat));
    }
}
