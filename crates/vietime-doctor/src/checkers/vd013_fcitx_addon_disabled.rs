// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD013 `FcitxAddonDisabled` — Warn.
//
// Fires when Fcitx5 is the active framework but a critical addon for the
// current session is missing from `addons_enabled`. Specifically:
//
// * Wayland session → expects `wayland-im` or `waylandim`.
// * X11 session     → expects `xim`.
//
// Without the addon for the current display server, keyboard events never
// reach the engine and Vietnamese input silently drops. This is a common
// misconfiguration because Fcitx5 ships the addons as separate packages
// on several distros and the config tool doesn't warn when one is missing.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD013).

use vietime_core::{ActiveFramework, Facts, Issue, SessionType, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd013;

impl Checker for Vd013 {
    fn id(&self) -> &'static str {
        "VD013"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        if facts.im.active_framework != ActiveFramework::Fcitx5 {
            return vec![];
        }
        let Some(f) = facts.im.fcitx5.as_ref() else { return vec![] };
        let Some(session) = facts.system.session else { return vec![] };
        let (required, display_name): (&[&str], &str) = match session {
            SessionType::Wayland => (&["wayland-im", "waylandim"][..], "wayland-im"),
            SessionType::X11 => (&["xim"][..], "xim"),
            _ => return vec![],
        };
        let has_one = required
            .iter()
            .any(|needle| f.addons_enabled.iter().any(|a| a.eq_ignore_ascii_case(needle)));
        if has_one {
            return vec![];
        }
        vec![Issue {
            id: "VD013".to_owned(),
            severity: Severity::Warn,
            title: format!("Fcitx5 is active but the `{display_name}` addon is not enabled"),
            detail: format!(
                "The {session:?} session requires the `{display_name}` addon \
                 for Fcitx5 to receive keyboard events. It is missing from \
                 `addons_enabled`; Vietnamese input will silently fail until \
                 it is enabled. Open fcitx5-configtool → Addons and tick it."
            ),
            facts_evidence: vec![
                format!("session: {}", session.as_str()),
                format!("addons_enabled: [{}]", f.addons_enabled.join(", ")),
            ],
            recommendation: Some("VR013".to_owned()),
        }]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use vietime_core::{Fcitx5Facts, ImFacts, SystemFacts};

    fn facts(session: SessionType, addons: &[&str]) -> Facts {
        Facts {
            system: SystemFacts { session: Some(session), ..SystemFacts::default() },
            im: ImFacts {
                active_framework: ActiveFramework::Fcitx5,
                fcitx5: Some(Fcitx5Facts {
                    version: None,
                    daemon_running: true,
                    daemon_pid: None,
                    config_dir: None,
                    addons_enabled: addons.iter().map(|s| (*s).to_owned()).collect(),
                    input_methods_configured: vec![],
                }),
                ..ImFacts::default()
            },
            ..Facts::default()
        }
    }

    #[test]
    fn fires_on_wayland_without_wayland_im() {
        let f = facts(SessionType::Wayland, &["unicode"]);
        let out = Vd013.check(&f);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR013"));
    }

    #[test]
    fn silent_on_wayland_with_wayland_im() {
        let f = facts(SessionType::Wayland, &["wayland-im", "unicode"]);
        assert!(Vd013.check(&f).is_empty());
    }

    #[test]
    fn accepts_case_insensitive_addon_name() {
        let f = facts(SessionType::Wayland, &["WaylandIM"]);
        assert!(Vd013.check(&f).is_empty());
    }

    #[test]
    fn fires_on_x11_without_xim() {
        let f = facts(SessionType::X11, &["unicode"]);
        assert_eq!(Vd013.check(&f).len(), 1);
    }

    #[test]
    fn silent_when_active_is_ibus() {
        let mut f = facts(SessionType::Wayland, &[]);
        f.im.active_framework = ActiveFramework::Ibus;
        assert!(Vd013.check(&f).is_empty());
    }

    #[test]
    fn silent_on_tty_session() {
        let f = facts(SessionType::Tty, &[]);
        assert!(Vd013.check(&f).is_empty());
    }
}
