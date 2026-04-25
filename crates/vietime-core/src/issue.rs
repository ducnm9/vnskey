// SPDX-License-Identifier: GPL-3.0-or-later
//
// Diagnostic issues and recommendations.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.2 (`Issue`, `Recommendation`),
// §B.4 (VD### catalogue).

use serde::{Deserialize, Serialize};

/// Severity ordering: Info < Warn < Error < Critical.
///
/// The ordering is significant: renderers use it to sort the issue list and
/// the CLI derives its exit code from the highest severity in the report
/// (see `spec/01` §A.4):
///
/// * `Info` / `Warn` → exit 1 (non-critical configuration issue).
/// * `Error` / `Critical` → exit 2 (critical — daemon absent, conflict).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
    Critical,
}

impl Severity {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }
}

/// A single finding produced by a Checker.
///
/// Issues reference Recommendations by id (`VR###`) so the renderer can
/// group "all issues that share a fix" rather than duplicating the fix
/// text per issue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Issue {
    /// Stable id like `"VD001"` — external tools (bug reports, CI) pattern-match on it.
    pub id: String,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    /// Human-readable citations back to the Facts that triggered this issue
    /// (e.g. `"XDG_SESSION_TYPE=wayland"`, `"ibus-daemon: not running"`).
    /// Empty when the check is a pure absence-of-signal (e.g. VD015).
    pub facts_evidence: Vec<String>,
    /// Optional `VR###` id — `None` for pure-Info issues that don't need a fix.
    pub recommendation: Option<String>,
}

/// A suggested fix. Each recommendation is addressable by its `id`; one
/// recommendation can be referenced by multiple issues.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Recommendation {
    pub id: String,
    pub title: String,
    pub description: String,
    /// Shell commands the user can copy-paste. Order is meaningful; we
    /// render them as a fenced code block.
    pub commands: Vec<String>,
    /// When `true`, this fix is safe to run in the Installer's unattended
    /// mode. When `false`, it requires user judgement (e.g. choosing between
    /// IBus and Fcitx5) and Installer must stop and ask.
    pub safe_to_run_unattended: bool,
    pub references: Vec<String>,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn severity_orders_info_below_critical() {
        assert!(Severity::Info < Severity::Warn);
        assert!(Severity::Warn < Severity::Error);
        assert!(Severity::Error < Severity::Critical);
    }

    #[test]
    fn severity_serialises_lowercase() {
        let json = serde_json::to_string(&Severity::Critical).expect("serialize severity");
        assert_eq!(json, "\"critical\"");
    }

    #[test]
    fn issue_round_trips_through_json() {
        let issue = Issue {
            id: "VD001".to_owned(),
            severity: Severity::Critical,
            title: "No IM framework active".to_owned(),
            detail: "Neither ibus-daemon nor fcitx5 is running.".to_owned(),
            facts_evidence: vec!["ibus-daemon: not running".to_owned()],
            recommendation: Some("VR001".to_owned()),
        };
        let json = serde_json::to_string(&issue).expect("serialize");
        let back: Issue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(issue, back);
    }
}
