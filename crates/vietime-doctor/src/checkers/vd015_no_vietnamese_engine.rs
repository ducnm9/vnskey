// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD015 `NoVietnameseEngineInstalled` — Info.
//
// Fires when no Vietnamese engine is present anywhere in `facts.im.engines`.
// The user may not actually want Vietnamese input — maybe they're running
// Doctor for a multilingual setup check — so this is Info, not Warn. But
// it's the most common "why isn't typing working" reason among users who
// installed Doctor before installing an engine, and worth surfacing
// explicitly so they don't chase a non-issue.
//
// No VR### attached — it's Info-only.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD015).

use vietime_core::{Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd015;

impl Checker for Vd015 {
    fn id(&self) -> &'static str {
        "VD015"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        if facts.im.engines.iter().any(|e| e.is_vietnamese) {
            return vec![];
        }
        vec![Issue {
            id: "VD015".to_owned(),
            severity: Severity::Info,
            title: "No Vietnamese input-method engine is installed".to_owned(),
            detail: "None of the detected IM engines are Vietnamese. If you \
                 want Vietnamese input, install one: `ibus-bamboo`, \
                 `ibus-unikey`, or `fcitx5-bamboo` depending on your \
                 framework. This is flagged as Info in case you're running \
                 Doctor for a different language's setup."
                .to_owned(),
            facts_evidence: vec![format!("engines observed: {}", facts.im.engines.len())],
            recommendation: None,
        }]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use vietime_core::{EngineFact, ImFacts, ImFramework};

    fn facts(engines: Vec<EngineFact>) -> Facts {
        Facts { im: ImFacts { engines, ..ImFacts::default() }, ..Facts::default() }
    }

    fn engine(name: &str, is_vietnamese: bool) -> EngineFact {
        EngineFact {
            name: name.to_owned(),
            package: None,
            version: None,
            framework: ImFramework::Ibus,
            is_vietnamese,
            is_registered: false,
        }
    }

    #[test]
    fn fires_when_no_engines() {
        let out = Vd015.check(&facts(vec![]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Info);
        assert!(out[0].recommendation.is_none());
    }

    #[test]
    fn fires_when_only_non_vietnamese_engines() {
        let out = Vd015.check(&facts(vec![engine("xkb:us::eng", false), engine("mozc-jp", false)]));
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn silent_when_vietnamese_engine_present() {
        assert!(Vd015.check(&facts(vec![engine("bamboo", true)])).is_empty());
    }
}
