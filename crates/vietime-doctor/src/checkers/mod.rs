// SPDX-License-Identifier: GPL-3.0-or-later
//
// Checker catalogue — concrete `Checker` implementations for Week 5.
//
// The nine checkers wired here are the ones spec §B.4 ships in Phase 1
// Week 5 (DOC-41…46). The Week-6 batch (VD009-VD011, VD013-VD015) lands
// under DOC-50 and will be added to `list_all` then.
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
mod vd012_legacy_input_method;

pub use vd001_no_im_framework::Vd001;
pub use vd002_im_conflict::Vd002;
pub use vd003_env_mismatch::Vd003;
pub use vd004_missing_sdl::Vd004;
pub use vd005_engine_not_registered::Vd005;
pub use vd006_wayland_ibus::Vd006;
pub use vd007_electron_no_ozone::Vd007;
pub use vd008_chrome_x11_on_wayland::Vd008;
pub use vd012_legacy_input_method::Vd012;

pub use recommendations::{all as all_recommendations, lookup as lookup_recommendation};

/// Return the full set of checkers wired in this build. Used by `main.rs`
/// to populate the orchestrator and by the `list` subcommand to enumerate
/// the VD ids users can expect to see in reports.
///
/// Order matches the spec §B.4 numbering (VD001 first, VD012 last) so the
/// `list` output reads top-down by severity tier. Runtime order doesn't
/// matter — `run_checkers` re-sorts issues by `(severity desc, id asc)`
/// before they reach the renderer.
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
        Arc::new(Vd012),
    ]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn list_all_returns_nine_checkers() {
        let all = list_all();
        assert_eq!(all.len(), 9, "Week 5 ships exactly 9 checkers");
    }

    #[test]
    fn list_all_ids_match_week5_catalogue() {
        let got: Vec<&'static str> = list_all().iter().map(|c| c.id()).collect();
        assert_eq!(
            got,
            vec!["VD001", "VD002", "VD003", "VD004", "VD005", "VD006", "VD007", "VD008", "VD012"]
        );
    }

    #[test]
    fn every_listed_checker_has_a_matching_recommendation_or_is_info() {
        // VD012 is Info-only and has no VR### fix attached; every other
        // Week-5 checker points at a recommendation we ship. The concrete
        // assertion "checker X refers to VR Y" lives in each VD file.
        let fixes: std::collections::HashSet<&'static str> =
            ["VR001", "VR002", "VR003", "VR004", "VR005", "VR006", "VR007", "VR008"]
                .into_iter()
                .collect();
        for r in all_recommendations() {
            assert!(fixes.contains(r.id.as_str()), "unexpected VR id in catalogue: {}", r.id);
        }
    }
}
