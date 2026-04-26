// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD006 `WaylandSessionIbus` — Warn.
//
// Fires when the session is Wayland and the active framework is IBus.
// IBus's Wayland story has known gaps (text-insertion bugs on several
// DEs, missing `zwp_text_input_v3` support in older releases); Fcitx5's
// `wayland-im` addon is the recommended path for VN input under
// Wayland today.
//
// This is an advisory warning, not an error — IBus does work for many
// users on Wayland. VR006 points them at the upgrade path.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD006).

use vietime_core::{ActiveFramework, Facts, Issue, SessionType, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd006;

impl Checker for Vd006 {
    fn id(&self) -> &'static str {
        "VD006"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        if facts.system.session != Some(SessionType::Wayland) {
            return vec![];
        }
        if facts.im.active_framework != ActiveFramework::Ibus {
            return vec![];
        }

        vec![Issue {
            id: "VD006".to_owned(),
            severity: Severity::Warn,
            title: "IBus on Wayland — Fcitx5 recommended".to_owned(),
            detail: "You're running IBus under a Wayland session. IBus has \
                 known text-insertion bugs on several Wayland desktops. \
                 Fcitx5 with the `wayland-im` addon is the recommended \
                 path for Vietnamese input on Wayland today."
                .to_owned(),
            facts_evidence: vec!["session: wayland".to_owned(), "active: ibus".to_owned()],
            recommendation: Some("VR006".to_owned()),
        }]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use vietime_core::{ImFacts, SystemFacts};

    fn system(session: Option<SessionType>) -> SystemFacts {
        SystemFacts { session, ..SystemFacts::default() }
    }

    #[test]
    fn fires_on_wayland_plus_ibus() {
        let facts = Facts {
            system: system(Some(SessionType::Wayland)),
            im: ImFacts { active_framework: ActiveFramework::Ibus, ..ImFacts::default() },
            ..Facts::default()
        };
        let out = Vd006.check(&facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warn);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR006"));
    }

    #[test]
    fn silent_on_wayland_plus_fcitx5() {
        let facts = Facts {
            system: system(Some(SessionType::Wayland)),
            im: ImFacts { active_framework: ActiveFramework::Fcitx5, ..ImFacts::default() },
            ..Facts::default()
        };
        assert!(Vd006.check(&facts).is_empty());
    }

    #[test]
    fn silent_on_x11_plus_ibus() {
        let facts = Facts {
            system: system(Some(SessionType::X11)),
            im: ImFacts { active_framework: ActiveFramework::Ibus, ..ImFacts::default() },
            ..Facts::default()
        };
        assert!(Vd006.check(&facts).is_empty());
    }
}
