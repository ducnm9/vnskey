// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD010 `VsCodeSnapDetected` — Warn.
//
// Fires when VSCode is installed via Snap. The Snap confinement strips
// GTK_IM_MODULE / QT_IM_MODULE at launch and there's no clean way to thread
// the user's IM framework into the sandbox without the `desktop` interface
// being connected. The spec §B.4 documents this as a widely-hit issue: a
// user types Vietnamese everywhere except the Snap VSCode window.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD010).

use vietime_core::{AppKind, Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd010;

impl Checker for Vd010 {
    fn id(&self) -> &'static str {
        "VD010"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        facts
            .apps
            .iter()
            .filter(|a| a.app_id == "vscode" || a.app_id == "code")
            .filter(|a| matches!(a.kind, AppKind::Snap { .. }))
            .map(|a| {
                let snap_name = match &a.kind {
                    AppKind::Snap { name } => name.clone(),
                    _ => "code".to_owned(),
                };
                Issue {
                    id: "VD010".to_owned(),
                    severity: Severity::Warn,
                    title: "VSCode is installed as a Snap — Vietnamese input may not work"
                        .to_owned(),
                    detail: "Snap confinement strips IM env vars at launch, so \
                         VSCode running from the Snap package does not see \
                         GTK_IM_MODULE / QT_IM_MODULE. Install the .deb / \
                         tarball build from https://code.visualstudio.com or \
                         the Flatpak instead — both respect IM env routing."
                        .to_owned(),
                    facts_evidence: vec![
                        format!("{}: Snap package `{snap_name}`", a.app_id),
                        format!("binary: {}", a.binary_path.display()),
                    ],
                    recommendation: Some("VR010".to_owned()),
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

    fn facts(apps: Vec<AppFacts>) -> Facts {
        Facts { apps, ..Facts::default() }
    }

    fn app(app_id: &str, kind: AppKind) -> AppFacts {
        AppFacts {
            app_id: app_id.to_owned(),
            binary_path: PathBuf::from("/snap/bin").join(app_id),
            version: None,
            kind,
            electron_version: None,
            uses_wayland: None,
            detector_notes: vec![],
        }
    }

    #[test]
    fn fires_when_vscode_is_snap() {
        let f = facts(vec![app("vscode", AppKind::Snap { name: "code".to_owned() })]);
        let out = Vd010.check(&f);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warn);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR010"));
    }

    #[test]
    fn silent_when_vscode_is_native_deb() {
        let f = facts(vec![app("vscode", AppKind::Electron)]);
        assert!(Vd010.check(&f).is_empty());
    }

    #[test]
    fn silent_when_vscode_is_flatpak() {
        let f = facts(vec![app(
            "vscode",
            AppKind::Flatpak { sandbox_id: "com.visualstudio.code".to_owned() },
        )]);
        assert!(Vd010.check(&f).is_empty());
    }

    #[test]
    fn silent_when_app_is_not_vscode() {
        let f = facts(vec![app("chrome", AppKind::Snap { name: "chromium".to_owned() })]);
        assert!(Vd010.check(&f).is_empty());
    }
}
