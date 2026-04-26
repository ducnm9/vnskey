// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD008 `ChromeX11OnWayland` — Warn.
//
// Fires when Chrome (specifically — not every Chromium variant) is
// running under a Wayland session with the X11/XWayland backend instead
// of Ozone/Wayland. Chrome defaults to X11 even on Wayland sessions,
// and under XWayland GTK/Qt IM routing silently drops Vietnamese input
// in the address bar and web forms.
//
// VD008 is the Chrome-specific complement to VD007. Both may fire on
// the same row; they reference distinct VR ids so de-dup at render time
// is trivial.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD008).

use vietime_core::{AppKind, Facts, Issue, SessionType, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd008;

impl Checker for Vd008 {
    fn id(&self) -> &'static str {
        "VD008"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        if facts.system.session != Some(SessionType::Wayland) {
            return vec![];
        }
        facts
            .apps
            .iter()
            .filter(|a| a.app_id == "chrome")
            .filter(|a| matches!(a.kind, AppKind::Chromium | AppKind::Electron))
            .filter(|a| a.uses_wayland == Some(false))
            .map(|a| Issue {
                id: "VD008".to_owned(),
                severity: Severity::Warn,
                title: "Chrome is using X11 on a Wayland session".to_owned(),
                detail: "Chrome is running through XWayland instead of its \
                     Ozone/Wayland backend. Vietnamese input drops through \
                     the X11 shim in that mode. Switch the Ozone platform \
                     to Wayland."
                    .to_owned(),
                facts_evidence: vec![
                    "session: wayland".to_owned(),
                    format!("{}: --ozone-platform=wayland not set", a.app_id),
                ],
                recommendation: Some("VR008".to_owned()),
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use vietime_core::{AppFacts, SystemFacts};

    fn facts_with(session: Option<SessionType>, apps: Vec<AppFacts>) -> Facts {
        Facts {
            system: SystemFacts { session, ..SystemFacts::default() },
            apps,
            ..Facts::default()
        }
    }

    fn app(app_id: &str, kind: AppKind, uses_wayland: Option<bool>) -> AppFacts {
        AppFacts {
            app_id: app_id.to_owned(),
            binary_path: PathBuf::from("/usr/bin").join(app_id),
            version: None,
            kind,
            electron_version: None,
            uses_wayland,
            detector_notes: vec![],
        }
    }

    #[test]
    fn fires_on_chrome_chromium_wayland_not_ozone() {
        let facts = facts_with(
            Some(SessionType::Wayland),
            vec![app("chrome", AppKind::Chromium, Some(false))],
        );
        let out = Vd008.check(&facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR008"));
    }

    #[test]
    fn silent_on_x11_session() {
        let facts =
            facts_with(Some(SessionType::X11), vec![app("chrome", AppKind::Chromium, Some(false))]);
        assert!(Vd008.check(&facts).is_empty());
    }

    #[test]
    fn silent_when_ozone_flag_set() {
        let facts = facts_with(
            Some(SessionType::Wayland),
            vec![app("chrome", AppKind::Chromium, Some(true))],
        );
        assert!(Vd008.check(&facts).is_empty());
    }

    #[test]
    fn silent_for_non_chrome_apps() {
        let facts = facts_with(
            Some(SessionType::Wayland),
            vec![app("vscode", AppKind::Electron, Some(false))],
        );
        assert!(Vd008.check(&facts).is_empty());
    }
}
