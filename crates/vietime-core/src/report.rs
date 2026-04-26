// SPDX-License-Identifier: GPL-3.0-or-later
//
// The top-level Doctor report.
//
// Spec ref: `spec/01-phase1-doctor.md` §A.5 (sample output),
// §B.2 (`Report`, `Facts`, `SystemFacts`, `ImFacts`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::desktop::DesktopEnv;
use crate::distro::Distro;
use crate::engine::{AppFacts, EngineFact, Fcitx5Facts, IbusFacts};
use crate::env::EnvFacts;
use crate::im_framework::ImFramework;
use crate::issue::{Issue, Recommendation, Severity};
use crate::session::SessionType;

/// Current JSON schema version. Bumped on breaking changes to the Report
/// shape; consumers must gate on this (spec/01 §B.14).
pub const REPORT_SCHEMA_VERSION: u32 = 1;

/// A single detector's failure — surfaced as `anomalies` in the Report so
/// consumers can tell the difference between "daemon not running" (a real
/// finding) and "we couldn't check the daemon because the detector timed
/// out" (a Doctor bug / env quirk).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Anomaly {
    /// Detector id (e.g. `"sys.distro"`).
    pub detector: String,
    /// Human-readable failure reason — rendered verbatim.
    pub reason: String,
}

/// The top-level report emitted by `vietime-doctor`. The JSON form is the
/// stable wire format for integrations (bug reports, compat matrix, CI).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Report {
    /// Schema version — always written as `REPORT_SCHEMA_VERSION`.
    pub schema_version: u32,
    pub generated_at: DateTime<Utc>,
    /// Version of `vietime-doctor` that produced this report.
    pub tool_version: String,
    pub facts: Facts,
    #[serde(default)]
    pub issues: Vec<Issue>,
    #[serde(default)]
    pub recommendations: Vec<Recommendation>,
    /// Detectors that failed to run. See `Anomaly`.
    #[serde(default)]
    pub anomalies: Vec<Anomaly>,
}

impl Report {
    /// Build an empty report with `schema_version` + `generated_at` pre-filled.
    /// Tests and fixtures can then mutate individual fields.
    #[must_use]
    pub fn new(tool_version: impl Into<String>) -> Self {
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            generated_at: Utc::now(),
            tool_version: tool_version.into(),
            facts: Facts::default(),
            issues: Vec::new(),
            recommendations: Vec::new(),
            anomalies: Vec::new(),
        }
    }

    /// Highest severity across all issues, or `None` if issue-free.
    #[must_use]
    pub fn max_severity(&self) -> Option<Severity> {
        self.issues.iter().map(|i| i.severity).max()
    }

    /// Exit code per `spec/01` §A.4:
    ///
    /// * `0` — report clean.
    /// * `1` — Info / Warn issues present.
    /// * `2` — Error / Critical issues present.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self.max_severity() {
            None | Some(Severity::Info) => 0,
            Some(Severity::Warn) => 1,
            Some(Severity::Error | Severity::Critical) => 2,
        }
    }
}

/// Structured system facts. All fields are `Option` because detectors can
/// fail independently (a timeout or an unknown distro shouldn't sink the
/// whole report).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Facts {
    pub system: SystemFacts,
    pub im: ImFacts,
    pub env: EnvFacts,
    #[serde(default)]
    pub apps: Vec<AppFacts>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemFacts {
    pub distro: Option<Distro>,
    pub desktop: Option<DesktopEnv>,
    pub session: Option<SessionType>,
    pub kernel: Option<String>,
    pub shell: Option<String>,
    /// The effective locale (value of `LC_ALL` if set, else `LC_CTYPE`,
    /// else `LANG`). `None` when none of the three are set.
    /// Populated by the Week-6 `LocaleDetector`.
    #[serde(default)]
    pub locale: Option<String>,
}

/// IM-framework facts. `active_framework` is a derived summary; the per-
/// framework structs carry the raw data each detector contributed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImFacts {
    #[serde(default)]
    pub active_framework: ActiveFramework,
    pub ibus: Option<IbusFacts>,
    pub fcitx5: Option<Fcitx5Facts>,
    #[serde(default)]
    pub engines: Vec<EngineFact>,
}

/// Which framework is "the active one" after running the IM detectors.
///
/// This is separate from `ImFramework` because it has an extra `Conflict`
/// state — both daemons running is a real, diagnosable condition.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActiveFramework {
    #[default]
    None,
    Ibus,
    Fcitx5,
    Conflict,
}

impl ActiveFramework {
    /// Collapse to a plain `ImFramework` when possible; `Conflict` maps to
    /// `None` because the question "which one should env vars point at?"
    /// has no answer when both are running.
    #[must_use]
    pub fn as_single(self) -> ImFramework {
        match self {
            Self::Ibus => ImFramework::Ibus,
            Self::Fcitx5 => ImFramework::Fcitx5,
            Self::None | Self::Conflict => ImFramework::None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::issue::Severity;

    fn issue(id: &str, sev: Severity) -> Issue {
        Issue {
            id: id.to_owned(),
            severity: sev,
            title: id.to_owned(),
            detail: String::new(),
            facts_evidence: vec![],
            recommendation: None,
        }
    }

    #[test]
    fn empty_report_exits_zero() {
        let r = Report::new("0.0.1");
        assert_eq!(r.exit_code(), 0);
        assert_eq!(r.max_severity(), None);
    }

    #[test]
    fn info_only_still_exits_zero() {
        let mut r = Report::new("0.0.1");
        r.issues.push(issue("VD012", Severity::Info));
        assert_eq!(r.exit_code(), 0);
    }

    #[test]
    fn warn_exits_one() {
        let mut r = Report::new("0.0.1");
        r.issues.push(issue("VD004", Severity::Warn));
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn error_exits_two() {
        let mut r = Report::new("0.0.1");
        r.issues.push(issue("VD003", Severity::Error));
        assert_eq!(r.exit_code(), 2);
    }

    #[test]
    fn critical_exits_two() {
        let mut r = Report::new("0.0.1");
        r.issues.push(issue("VD001", Severity::Critical));
        r.issues.push(issue("VD004", Severity::Warn));
        assert_eq!(r.exit_code(), 2);
        assert_eq!(r.max_severity(), Some(Severity::Critical));
    }

    #[test]
    fn schema_version_round_trips() {
        let r = Report::new("0.0.1");
        let json = serde_json::to_string(&r).expect("serialize");
        let back: Report = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.schema_version, REPORT_SCHEMA_VERSION);
    }

    #[test]
    fn active_framework_conflict_collapses_to_none_as_single() {
        assert_eq!(ActiveFramework::Conflict.as_single(), ImFramework::None);
        assert_eq!(ActiveFramework::None.as_single(), ImFramework::None);
        assert_eq!(ActiveFramework::Ibus.as_single(), ImFramework::Ibus);
        assert_eq!(ActiveFramework::Fcitx5.as_single(), ImFramework::Fcitx5);
    }
}
