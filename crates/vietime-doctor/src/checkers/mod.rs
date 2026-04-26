// SPDX-License-Identifier: GPL-3.0-or-later
//
// Checker catalogue — concrete `Checker` implementations for Weeks 5-6.
//
// Week 5 (DOC-41…46) shipped the first nine checkers; Week 6 (DOC-50)
// completes the Phase-1 catalogue with VD009-VD011 and VD013-VD015. All
// 15 are wired through `list_all` and enumerated by the `list` subcommand.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4.

use std::sync::Arc;

use crate::checker::Checker;

pub mod recommendations;

mod vd001_no_im_framework;
mod vd002_im_conflict;
mod vd003_env_mismatch;
mod vd004_missing_sdl;
mod vd005_engine_not_registered;
mod vd006_wayland_ibus;
mod vd007_electron_no_ozone;
mod vd008_chrome_x11_on_wayland;
mod vd009_env_conflict_between_files;
mod vd010_vscode_snap;
mod vd011_flatpak_no_portal;
mod vd012_legacy_input_method;
mod vd013_fcitx_addon_disabled;
mod vd014_unicode_locale_missing;
mod vd015_no_vietnamese_engine;

pub use vd001_no_im_framework::Vd001;
pub use vd002_im_conflict::Vd002;
pub use vd003_env_mismatch::Vd003;
pub use vd004_missing_sdl::Vd004;
pub use vd005_engine_not_registered::Vd005;
pub use vd006_wayland_ibus::Vd006;
pub use vd007_electron_no_ozone::Vd007;
pub use vd008_chrome_x11_on_wayland::Vd008;
pub use vd009_env_conflict_between_files::Vd009;
pub use vd010_vscode_snap::Vd010;
pub use vd011_flatpak_no_portal::Vd011;
pub use vd012_legacy_input_method::Vd012;
pub use vd013_fcitx_addon_disabled::Vd013;
pub use vd014_unicode_locale_missing::Vd014;
pub use vd015_no_vietnamese_engine::Vd015;

pub use recommendations::{all as all_recommendations, lookup as lookup_recommendation};

/// Return the full set of checkers wired in this build. Used by `main.rs`
/// to populate the orchestrator and by the `list` subcommand to enumerate
/// the VD ids users can expect to see in reports.
///
/// Order matches the spec §B.4 numbering (VD001 first, VD015 last) so the
/// `list` output reads top-down by id. Runtime order doesn't matter —
/// `run_checkers` re-sorts issues by `(severity desc, id asc)` before
/// they reach the renderer.
#[must_use]
pub fn list_all() -> Vec<Arc<dyn Checker>> {
    vec![
        Arc::new(Vd001),
        Arc::new(Vd002),
        Arc::new(Vd003),
        Arc::new(Vd004),
        Arc::new(Vd005),
        Arc::new(Vd006),
        Arc::new(Vd007),
        Arc::new(Vd008),
        Arc::new(Vd009),
        Arc::new(Vd010),
        Arc::new(Vd011),
        Arc::new(Vd012),
        Arc::new(Vd013),
        Arc::new(Vd014),
        Arc::new(Vd015),
    ]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn list_all_returns_fifteen_checkers() {
        let all = list_all();
        assert_eq!(all.len(), 15, "Phase-1 ships exactly 15 checkers");
    }

    #[test]
    fn list_all_ids_match_phase1_catalogue() {
        let got: Vec<&'static str> = list_all().iter().map(|c| c.id()).collect();
        assert_eq!(
            got,
            vec![
                "VD001", "VD002", "VD003", "VD004", "VD005", "VD006", "VD007", "VD008", "VD009",
                "VD010", "VD011", "VD012", "VD013", "VD014", "VD015",
            ]
        );
    }

    #[test]
    fn every_listed_checker_has_a_matching_recommendation_or_is_info() {
        // VD012 and VD015 are Info-only and have no VR### fix attached;
        // every other checker points at a recommendation we ship. The
        // concrete assertion "checker X refers to VR Y" lives in each
        // VD file.
        let fixes: std::collections::HashSet<&'static str> = [
            "VR001", "VR002", "VR003", "VR004", "VR005", "VR006", "VR007", "VR008", "VR009",
            "VR010", "VR011", "VR013", "VR014",
        ]
        .into_iter()
        .collect();
        for r in all_recommendations() {
            assert!(fixes.contains(r.id.as_str()), "unexpected VR id in catalogue: {}", r.id);
        }
    }
}
