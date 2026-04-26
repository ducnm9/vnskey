// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD002 `ImFrameworkConflict` — Error.
//
// Fires when both ibus-daemon AND fcitx5 are running. GTK/Qt IM module
// routing gets confused when two daemons fight over the same D-Bus
// well-known name ownership — some apps pick IBus, others pick Fcitx5,
// and the user sees Vietnamese input only in half their apps.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD002).

use vietime_core::{ActiveFramework, Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd002;

impl Checker for Vd002 {
    fn id(&self) -> &'static str {
        "VD002"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        if facts.im.active_framework != ActiveFramework::Conflict {
            return vec![];
        }

        let mut evidence = Vec::new();
        if let Some(ibus) = facts.im.ibus.as_ref() {
            evidence.push(format!(
                "ibus-daemon: running{}",
                ibus.daemon_pid.map(|p| format!(" (pid {p})")).unwrap_or_default()
            ));
        }
        if let Some(fcitx5) = facts.im.fcitx5.as_ref() {
            evidence.push(format!(
                "fcitx5: running{}",
                fcitx5.daemon_pid.map(|p| format!(" (pid {p})")).unwrap_or_default()
            ));
        }

        vec![Issue {
            id: "VD002".to_owned(),
            severity: Severity::Error,
            title: "Both IBus and Fcitx5 are running".to_owned(),
            detail: "Two IM daemons are active at once. Applications will \
                 pick whichever framework wins the GTK/Qt module lookup \
                 first, giving inconsistent behaviour across apps. Pick \
                 one framework and stop the other."
                .to_owned(),
            facts_evidence: evidence,
            recommendation: Some("VR002".to_owned()),
        }]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use vietime_core::{Fcitx5Facts, IbusFacts, ImFacts};

    fn ibus(pid: Option<u32>) -> IbusFacts {
        IbusFacts {
            version: None,
            daemon_running: true,
            daemon_pid: pid,
            config_dir: Some(PathBuf::from("/tmp")),
            registered_engines: vec![],
        }
    }
    fn fcitx5(pid: Option<u32>) -> Fcitx5Facts {
        Fcitx5Facts {
            version: None,
            daemon_running: true,
            daemon_pid: pid,
            config_dir: Some(PathBuf::from("/tmp")),
            addons_enabled: vec![],
            input_methods_configured: vec![],
        }
    }

    #[test]
    fn fires_on_conflict() {
        let facts = Facts {
            im: ImFacts {
                active_framework: ActiveFramework::Conflict,
                ibus: Some(ibus(Some(111))),
                fcitx5: Some(fcitx5(Some(222))),
                ..ImFacts::default()
            },
            ..Facts::default()
        };
        let out = Vd002.check(&facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "VD002");
        assert_eq!(out[0].severity, Severity::Error);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR002"));
        // Both pids show up in evidence so the user can `kill` the right one.
        let joined = out[0].facts_evidence.join("\n");
        assert!(joined.contains("pid 111"));
        assert!(joined.contains("pid 222"));
    }

    #[test]
    fn silent_on_single_daemon() {
        for active in [ActiveFramework::Ibus, ActiveFramework::Fcitx5, ActiveFramework::None] {
            let facts = Facts {
                im: ImFacts { active_framework: active, ..ImFacts::default() },
                ..Facts::default()
            };
            assert!(Vd002.check(&facts).is_empty(), "VD002 must not fire for {active:?}");
        }
    }

    #[test]
    fn evidence_survives_missing_pids() {
        // A detector that knows "daemon up" without a pid (pure D-Bus
        // peek) must still produce readable evidence.
        let facts = Facts {
            im: ImFacts {
                active_framework: ActiveFramework::Conflict,
                ibus: Some(ibus(None)),
                fcitx5: Some(fcitx5(None)),
                ..ImFacts::default()
            },
            ..Facts::default()
        };
        let out = Vd002.check(&facts);
        assert_eq!(out.len(), 1);
        let joined = out[0].facts_evidence.join("\n");
        assert!(joined.contains("ibus-daemon: running"));
        assert!(joined.contains("fcitx5: running"));
        assert!(!joined.contains("pid"));
    }
}
