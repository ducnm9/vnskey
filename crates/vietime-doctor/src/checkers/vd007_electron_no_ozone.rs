// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD007 `ElectronWaylandNoOzone` — Error.
//
// Fires once per `--app <X>` row (`facts.apps[i]`) where:
//   * kind is Electron or Chromium, AND
//   * `uses_wayland == Some(false)` (the app is *observably* missing
//     the `--ozone-platform=wayland` flag).
//
// `uses_wayland == None` means "we can't tell" — typically because we're
// on an X11 host or the app isn't running. Don't fire then: we'd be
// screaming about a Wayland-specific misconfiguration without knowing
// the session is actually Wayland.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD007).

use vietime_core::{AppKind, Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd007;

impl Checker for Vd007 {
    fn id(&self) -> &'static str {
        "VD007"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        facts
            .apps
            .iter()
            .filter(|a| matches!(a.kind, AppKind::Electron | AppKind::Chromium))
            .filter(|a| a.uses_wayland == Some(false))
            .map(|a| {
                let version = a
                    .electron_version
                    .as_deref()
                    .map(|v| format!(" (Electron {v})"))
                    .unwrap_or_default();
                Issue {
                    id: "VD007".to_owned(),
                    severity: Severity::Error,
                    title: format!("{}{} is missing Wayland Ozone flags", a.app_id, version),
                    detail: format!(
                        "{} is running without `--ozone-platform=wayland`. \
                         Electron/Chromium apps drop Vietnamese input under \
                         Wayland unless they're launched with the Ozone \
                         Wayland backend enabled.",
                        a.app_id
                    ),
                    facts_evidence: vec![format!(
                        "{} running without --ozone-platform=wayland",
                        a.app_id
                    )],
                    recommendation: Some("VR007".to_owned()),
                }
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use vietime_core::AppFacts;

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
    fn fires_when_electron_explicitly_not_ozone() {
        let facts =
            Facts { apps: vec![app("vscode", AppKind::Electron, Some(false))], ..Facts::default() };
        let out = Vd007.check(&facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Error);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR007"));
        assert!(out[0].title.contains("vscode"));
    }

    #[test]
    fn silent_when_uses_wayland_is_true() {
        let facts =
            Facts { apps: vec![app("vscode", AppKind::Electron, Some(true))], ..Facts::default() };
        assert!(Vd007.check(&facts).is_empty());
    }

    #[test]
    fn silent_when_uses_wayland_is_none() {
        // `None` = unknown (e.g. X11 host, process not running). Don't
        // fire a Wayland-specific diagnostic when we have no evidence
        // we're even on Wayland.
        let facts =
            Facts { apps: vec![app("vscode", AppKind::Electron, None)], ..Facts::default() };
        assert!(Vd007.check(&facts).is_empty());
    }

    #[test]
    fn silent_for_native_kind() {
        let facts =
            Facts { apps: vec![app("firefox", AppKind::Native, Some(false))], ..Facts::default() };
        assert!(Vd007.check(&facts).is_empty());
    }

    #[test]
    fn fires_once_per_matching_electron_app() {
        let facts = Facts {
            apps: vec![
                app("vscode", AppKind::Electron, Some(false)),
                app("slack", AppKind::Electron, Some(false)),
            ],
            ..Facts::default()
        };
        let out = Vd007.check(&facts);
        assert_eq!(out.len(), 2);
        let titles: Vec<&str> = out.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.iter().any(|t| t.contains("vscode")));
        assert!(titles.iter().any(|t| t.contains("slack")));
    }
}
