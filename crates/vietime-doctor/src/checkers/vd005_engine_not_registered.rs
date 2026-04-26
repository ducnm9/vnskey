// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD005 `EngineInstalledNotRegistered` — Warn.
//
// Fires once per engine row where:
//   * the engine is Vietnamese (`is_vietnamese`), AND
//   * the engine comes from a real package (`package.is_some()`), AND
//   * the engine is NOT listed by the active framework (`is_registered == false`).
//
// This is the "I apt-installed ibus-bamboo but it doesn't show up in the
// switcher" failure mode. The user has to open `ibus-setup` / `fcitx5-
// configtool` and add the engine to their input-method list.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD005).

use vietime_core::{Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd005;

impl Checker for Vd005 {
    fn id(&self) -> &'static str {
        "VD005"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        facts
            .im
            .engines
            .iter()
            .filter(|e| e.is_vietnamese && e.package.is_some() && !e.is_registered)
            .map(|e| {
                let pkg = e.package.as_deref().unwrap_or("(unknown package)");
                let fw = e.framework.display();
                Issue {
                    id: "VD005".to_owned(),
                    severity: Severity::Warn,
                    title: format!("{} is installed but not registered in {fw}", e.name),
                    detail: format!(
                        "The {pkg} package ships the `{}` engine but {fw} \
                         hasn't been told to use it. Open the {fw} config \
                         tool and add the engine to your input-method list.",
                        e.name
                    ),
                    facts_evidence: vec![format!(
                        "{} ({pkg}): installed but not registered in {fw}",
                        e.name
                    )],
                    recommendation: Some("VR005".to_owned()),
                }
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use vietime_core::{EngineFact, ImFacts, ImFramework};

    fn engine(
        name: &str,
        package: Option<&str>,
        vn: bool,
        registered: bool,
        framework: ImFramework,
    ) -> EngineFact {
        EngineFact {
            name: name.to_owned(),
            package: package.map(ToOwned::to_owned),
            version: None,
            framework,
            is_vietnamese: vn,
            is_registered: registered,
        }
    }

    #[test]
    fn fires_for_installed_unregistered_vn_engine() {
        let facts = Facts {
            im: ImFacts {
                engines: vec![engine(
                    "bamboo",
                    Some("ibus-bamboo"),
                    true,
                    false,
                    ImFramework::Ibus,
                )],
                ..ImFacts::default()
            },
            ..Facts::default()
        };
        let out = Vd005.check(&facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warn);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR005"));
        assert!(out[0].title.contains("bamboo"));
    }

    #[test]
    fn silent_when_registered() {
        let facts = Facts {
            im: ImFacts {
                engines: vec![engine("bamboo", Some("ibus-bamboo"), true, true, ImFramework::Ibus)],
                ..ImFacts::default()
            },
            ..Facts::default()
        };
        assert!(Vd005.check(&facts).is_empty());
    }

    #[test]
    fn silent_for_non_vietnamese_engine() {
        let facts = Facts {
            im: ImFacts {
                engines: vec![engine(
                    "pinyin",
                    Some("ibus-pinyin"),
                    false,
                    false,
                    ImFramework::Ibus,
                )],
                ..ImFacts::default()
            },
            ..Facts::default()
        };
        assert!(Vd005.check(&facts).is_empty());
    }

    #[test]
    fn silent_when_no_package_origin_known() {
        // If we can't prove the engine is installed (maybe a tarball
        // drop) we can't honestly claim "installed but not registered".
        let facts = Facts {
            im: ImFacts {
                engines: vec![engine("bamboo", None, true, false, ImFramework::Ibus)],
                ..ImFacts::default()
            },
            ..Facts::default()
        };
        assert!(Vd005.check(&facts).is_empty());
    }

    #[test]
    fn multiple_engines_produce_multiple_issues() {
        let facts = Facts {
            im: ImFacts {
                engines: vec![
                    engine("bamboo", Some("ibus-bamboo"), true, false, ImFramework::Ibus),
                    engine("unikey", Some("ibus-unikey"), true, false, ImFramework::Ibus),
                ],
                ..ImFacts::default()
            },
            ..Facts::default()
        };
        let out = Vd005.check(&facts);
        assert_eq!(out.len(), 2);
        let titles: Vec<&str> = out.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.iter().any(|t| t.contains("bamboo")));
        assert!(titles.iter().any(|t| t.contains("unikey")));
    }
}
