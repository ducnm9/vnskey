// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD001 `NoImFrameworkActive` — Critical.
//
// Fires when no IM daemon is running but a Vietnamese engine package is
// installed on disk. This is the flagship symptom of "I installed bamboo
// but VN input still doesn't work" — neither ibus-daemon nor fcitx5 is
// up, so the installed engine is never even given a chance.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD001).

use vietime_core::{ActiveFramework, Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd001;

impl Checker for Vd001 {
    fn id(&self) -> &'static str {
        "VD001"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        // Only fire when no daemon is active AT ALL. `Conflict` is the
        // other direction — VD002 handles it.
        if facts.im.active_framework != ActiveFramework::None {
            return vec![];
        }
        // Require at least one Vietnamese engine on disk; otherwise the
        // user isn't even trying to type Vietnamese and this "Critical"
        // would be noise on a vanilla desktop.
        let Some(vn_engine) = facts.im.engines.iter().find(|e| e.is_vietnamese) else {
            return vec![];
        };

        let mut evidence =
            vec!["ibus-daemon: not running".to_owned(), "fcitx5: not running".to_owned()];
        evidence.push(format!("engine: {} ({})", vn_engine.name, vn_engine.framework.display()));

        vec![Issue {
            id: "VD001".to_owned(),
            severity: Severity::Critical,
            title: "No input-method daemon running".to_owned(),
            detail: format!(
                "A Vietnamese engine ({}) is installed but neither ibus-daemon \
                 nor fcitx5 is running. Vietnamese input will not work until \
                 one of them is started.",
                vn_engine.name
            ),
            facts_evidence: evidence,
            recommendation: Some("VR001".to_owned()),
        }]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use vietime_core::{EngineFact, ImFacts, ImFramework};

    fn vn_engine() -> EngineFact {
        EngineFact {
            name: "bamboo".to_owned(),
            package: Some("ibus-bamboo".to_owned()),
            version: None,
            framework: ImFramework::Ibus,
            is_vietnamese: true,
            is_registered: false,
        }
    }

    #[test]
    fn fires_when_no_daemon_and_vn_engine_installed() {
        let facts = Facts {
            im: ImFacts {
                active_framework: ActiveFramework::None,
                engines: vec![vn_engine()],
                ..ImFacts::default()
            },
            ..Facts::default()
        };
        let out = Vd001.check(&facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "VD001");
        assert_eq!(out[0].severity, Severity::Critical);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR001"));
        assert!(out[0].facts_evidence.iter().any(|e| e.contains("ibus-daemon")));
        assert!(out[0].facts_evidence.iter().any(|e| e.contains("bamboo")));
    }

    #[test]
    fn silent_when_a_daemon_is_up() {
        for active in [ActiveFramework::Ibus, ActiveFramework::Fcitx5, ActiveFramework::Conflict] {
            let facts = Facts {
                im: ImFacts {
                    active_framework: active,
                    engines: vec![vn_engine()],
                    ..ImFacts::default()
                },
                ..Facts::default()
            };
            assert!(Vd001.check(&facts).is_empty(), "VD001 must not fire for {active:?}");
        }
    }

    #[test]
    fn silent_when_no_vn_engine_even_without_daemon() {
        let facts = Facts {
            im: ImFacts {
                active_framework: ActiveFramework::None,
                engines: vec![EngineFact {
                    name: "xkb:us::eng".to_owned(),
                    package: None,
                    version: None,
                    framework: ImFramework::Ibus,
                    is_vietnamese: false,
                    is_registered: true,
                }],
                ..ImFacts::default()
            },
            ..Facts::default()
        };
        assert!(Vd001.check(&facts).is_empty());
    }

    #[test]
    fn evidence_names_engine_framework() {
        let facts = Facts {
            im: ImFacts {
                active_framework: ActiveFramework::None,
                engines: vec![EngineFact { framework: ImFramework::Fcitx5, ..vn_engine() }],
                ..ImFacts::default()
            },
            ..Facts::default()
        };
        let out = Vd001.check(&facts);
        let joined = out[0].facts_evidence.join("\n");
        assert!(joined.contains("Fcitx5"), "evidence should cite framework display name: {joined}");
    }
}
