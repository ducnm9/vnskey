// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD012 `LegacyImSettingEmpty` — Info.
//
// Fires when `INPUT_METHOD` is unset. This variable is a legacy signal
// read by a small number of apps (notably some JetBrains IDEs' older
// builds and a handful of GTK2-era tools). Most of the ecosystem has
// moved on to GTK_IM_MODULE/QT_IM_MODULE, so this is purely
// informational — we don't ship a VR### fix for it because the right
// answer is "leave it alone unless one of your apps specifically asks".
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD012).

use vietime_core::{Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd012;

impl Checker for Vd012 {
    fn id(&self) -> &'static str {
        "VD012"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        if facts.env.input_method.is_some() {
            return vec![];
        }
        vec![Issue {
            id: "VD012".to_owned(),
            severity: Severity::Info,
            title: "INPUT_METHOD is not set".to_owned(),
            detail: "The legacy `INPUT_METHOD` variable is unset. Most \
                 modern apps use GTK_IM_MODULE / QT_IM_MODULE instead and \
                 don't need it — this is flagged as Info so you can tell \
                 it apart from an unset GTK/QT var, which would be Error \
                 territory."
                .to_owned(),
            facts_evidence: vec!["INPUT_METHOD: unset".to_owned()],
            recommendation: None,
        }]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vietime_core::EnvFacts;

    #[test]
    fn fires_when_input_method_unset() {
        let facts = Facts::default();
        let out = Vd012.check(&facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Info);
        assert!(out[0].recommendation.is_none());
    }

    #[test]
    fn silent_when_input_method_set() {
        let map: HashMap<String, String> =
            [("INPUT_METHOD".to_owned(), "ibus".to_owned())].into_iter().collect();
        let facts = Facts { env: EnvFacts::from_env(&map), ..Facts::default() };
        assert!(Vd012.check(&facts).is_empty());
    }
}
