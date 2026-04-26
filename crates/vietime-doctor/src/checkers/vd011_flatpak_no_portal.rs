// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD011 `FlatpakAppNoImPortal` — Warn.
//
// Fires when a Flatpak'd app has been detected but the IBus/Fcitx5 socket
// is not exposed to the sandbox. The Phase-1 detector stack can't read the
// sandbox metadata directly (that's DOC-82 in Phase 2), so the Phase-1
// heuristic is: if ANY app row reports `AppKind::Flatpak` and there's no
// `im-module`/`ibus`/`fcitx` hint in its detector notes, we warn the user
// that this is the common Flatpak-breaks-VN-input case and point them at
// `flatpak override`.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD011).

use vietime_core::{AppKind, Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd011;

impl Checker for Vd011 {
    fn id(&self) -> &'static str {
        "VD011"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        facts
            .apps
            .iter()
            .filter_map(|a| {
                let AppKind::Flatpak { sandbox_id } = &a.kind else { return None };
                if has_im_portal_hint(&a.detector_notes) {
                    return None;
                }
                Some(Issue {
                    id: "VD011".to_owned(),
                    severity: Severity::Warn,
                    title: format!(
                        "Flatpak app `{}` may not have access to the IM socket",
                        a.app_id
                    ),
                    detail: "Flatpak sandboxes each app into its own filesystem/DBus \
                         namespace. The IBus / Fcitx5 socket must be exposed \
                         via `flatpak override` or a `--socket` manifest entry \
                         for the app to see Vietnamese input. Doctor couldn't \
                         find an IM portal hint in this app's notes, so \
                         Vietnamese input may silently fail inside it."
                        .to_owned(),
                    facts_evidence: vec![
                        format!("{}: Flatpak (sandbox: {sandbox_id})", a.app_id),
                        format!("binary: {}", a.binary_path.display()),
                    ],
                    recommendation: Some("VR011".to_owned()),
                })
            })
            .collect()
    }
}

/// Heuristic used until DOC-82 brings real manifest inspection: look for
/// any note mentioning `ibus`, `fcitx`, or the word `im-module`.
fn has_im_portal_hint(notes: &[String]) -> bool {
    notes.iter().any(|n| {
        let l = n.to_ascii_lowercase();
        l.contains("ibus")
            || l.contains("fcitx")
            || l.contains("im-module")
            || l.contains("im_module")
    })
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

    fn app(app_id: &str, kind: AppKind, notes: Vec<String>) -> AppFacts {
        AppFacts {
            app_id: app_id.to_owned(),
            binary_path: PathBuf::from("/var/lib/flatpak/app").join(app_id),
            version: None,
            kind,
            electron_version: None,
            uses_wayland: None,
            detector_notes: notes,
        }
    }

    #[test]
    fn fires_on_flatpak_without_im_hint() {
        let f = facts(vec![app(
            "firefox",
            AppKind::Flatpak { sandbox_id: "org.mozilla.firefox".to_owned() },
            vec!["wayland scale 1.0".to_owned()],
        )]);
        let out = Vd011.check(&f);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR011"));
    }

    #[test]
    fn silent_when_notes_mention_ibus() {
        let f = facts(vec![app(
            "firefox",
            AppKind::Flatpak { sandbox_id: "org.mozilla.firefox".to_owned() },
            vec!["GTK_IM_MODULE=ibus is exported in sandbox".to_owned()],
        )]);
        assert!(Vd011.check(&f).is_empty());
    }

    #[test]
    fn silent_for_native_apps() {
        let f = facts(vec![app("firefox", AppKind::Native, vec![])]);
        assert!(Vd011.check(&f).is_empty());
    }

    #[test]
    fn silent_when_no_apps() {
        let f = facts(vec![]);
        assert!(Vd011.check(&f).is_empty());
    }
}
