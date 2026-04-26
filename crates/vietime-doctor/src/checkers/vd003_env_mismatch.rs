// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD003 `EnvVarMismatch` — Error.
//
// Fires when GTK_IM_MODULE / QT_IM_MODULE / XMODIFIERS don't all agree
// with each other, OR when they agree but point at a framework different
// from the one that's actually running.
//
// Typical symptom: `fcitx5` is the active daemon but `/etc/environment`
// still has `GTK_IM_MODULE=ibus` left over from the old setup. GTK apps
// silently drop VN input while Qt apps (which read `QT_IM_MODULE=fcitx`)
// work fine — the kind of "works in terminal but not in browser" puzzle
// VD003 exists to untangle.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD003).

use vietime_core::{
    im_framework::parse_im_module_value, ActiveFramework, Facts, ImFramework, Issue, Severity,
    IM_ENV_KEYS,
};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd003;

impl Checker for Vd003 {
    fn id(&self) -> &'static str {
        "VD003"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        // Only GTK_IM_MODULE / QT_IM_MODULE / XMODIFIERS participate in
        // the disagreement check — SDL/GLFW/CLUTTER have their own checkers.
        let tracked: [&'static str; 3] = ["GTK_IM_MODULE", "QT_IM_MODULE", "XMODIFIERS"];
        let set_keys: Vec<(&'static str, ImFramework)> = tracked
            .iter()
            .filter_map(|k| facts.env.get_by_key(k).map(|v| (*k, parse_im_module_value(v))))
            .collect();

        // Skip the "nothing is set" case — a pure-absence finding is Info
        // territory (VD015) and not our job.
        if set_keys.is_empty() {
            return vec![];
        }

        let active = facts.im.active_framework.as_single();
        let disagree = facts.env.has_disagreement();

        // Case 1: the three vars disagree with each other.
        // Case 2: they agree with each other but differ from the active
        //         framework (we skip the comparison when active is None
        //         so we don't nag on "no daemon" installs — VD001 handles
        //         that side of things).
        let agreed = if disagree {
            None
        } else {
            set_keys.iter().map(|(_, f)| *f).find(|f| *f != ImFramework::None)
        };
        let mismatches_active = match (agreed, active) {
            (Some(unified), running) if running != ImFramework::None => unified != running,
            _ => false,
        };
        if !disagree && !mismatches_active {
            return vec![];
        }

        // Cite every *set* IM env var — including ones we didn't use in
        // the disagreement calc, because the fix has to touch them all.
        let mut evidence: Vec<String> = IM_ENV_KEYS
            .iter()
            .filter_map(|k| facts.env.get_by_key(k).map(|v| format!("{k}={v}")))
            .collect();
        if active != ImFramework::None {
            evidence.push(format!("active framework: {}", active.display()));
        } else if facts.im.active_framework == ActiveFramework::Conflict {
            evidence.push("active framework: conflict (both daemons running)".to_owned());
        } else {
            evidence.push("active framework: none".to_owned());
        }

        let title = if disagree {
            "IM environment variables disagree".to_owned()
        } else {
            "IM environment variables don't match the active framework".to_owned()
        };
        let detail = if disagree {
            "GTK_IM_MODULE, QT_IM_MODULE, and XMODIFIERS point at different \
             frameworks. GTK and Qt apps will route input differently, and \
             Vietnamese input will work only for some of them."
                .to_owned()
        } else {
            format!(
                "The environment variables are aligned but point at a \
                 framework different from the one that's running ({}). \
                 Update them to match.",
                active.display()
            )
        };

        vec![Issue {
            id: "VD003".to_owned(),
            severity: Severity::Error,
            title,
            detail,
            facts_evidence: evidence,
            recommendation: Some("VR003".to_owned()),
        }]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vietime_core::{EnvFacts, ImFacts};

    fn env_facts(pairs: &[(&str, &str)]) -> EnvFacts {
        let map: HashMap<String, String> =
            pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        EnvFacts::from_env(&map)
    }

    #[test]
    fn fires_when_gtk_and_qt_disagree() {
        let facts = Facts {
            env: env_facts(&[
                ("GTK_IM_MODULE", "ibus"),
                ("QT_IM_MODULE", "fcitx"),
                ("XMODIFIERS", "@im=ibus"),
            ]),
            ..Facts::default()
        };
        let out = Vd003.check(&facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Error);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR003"));
        let ev = out[0].facts_evidence.join("\n");
        assert!(ev.contains("GTK_IM_MODULE=ibus"));
        assert!(ev.contains("QT_IM_MODULE=fcitx"));
    }

    #[test]
    fn fires_when_env_agrees_but_differs_from_active() {
        let facts = Facts {
            im: ImFacts { active_framework: ActiveFramework::Fcitx5, ..ImFacts::default() },
            env: env_facts(&[
                ("GTK_IM_MODULE", "ibus"),
                ("QT_IM_MODULE", "ibus"),
                ("XMODIFIERS", "@im=ibus"),
            ]),
            ..Facts::default()
        };
        let out = Vd003.check(&facts);
        assert_eq!(out.len(), 1);
        assert!(out[0].detail.contains("Fcitx5"));
    }

    #[test]
    fn silent_when_env_unset() {
        let facts = Facts::default();
        assert!(Vd003.check(&facts).is_empty());
    }

    #[test]
    fn silent_when_env_matches_active() {
        let facts = Facts {
            im: ImFacts { active_framework: ActiveFramework::Ibus, ..ImFacts::default() },
            env: env_facts(&[
                ("GTK_IM_MODULE", "ibus"),
                ("QT_IM_MODULE", "ibus"),
                ("XMODIFIERS", "@im=ibus"),
            ]),
            ..Facts::default()
        };
        assert!(Vd003.check(&facts).is_empty());
    }

    #[test]
    fn silent_when_env_set_but_active_is_none() {
        // User has env vars wired but no daemon running — VD001 will
        // scream about the missing daemon; VD003 should stay quiet so
        // the report doesn't double-count the same root cause.
        let facts = Facts {
            im: ImFacts { active_framework: ActiveFramework::None, ..ImFacts::default() },
            env: env_facts(&[
                ("GTK_IM_MODULE", "fcitx"),
                ("QT_IM_MODULE", "fcitx"),
                ("XMODIFIERS", "@im=fcitx"),
            ]),
            ..Facts::default()
        };
        assert!(Vd003.check(&facts).is_empty());
    }
}
