// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD004 `MissingSdlImModule` — Warn.
//
// Fires when a framework is active but `SDL_IM_MODULE` is unset. SDL2
// apps (Love2D, emulators, Steam's overlay) can't route Vietnamese
// input without this env var pointing at the active framework. Without
// it SDL falls back to raw keycodes and the user types English.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD004).

use vietime_core::{ActiveFramework, Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd004;

impl Checker for Vd004 {
    fn id(&self) -> &'static str {
        "VD004"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        // Only interesting when a framework is actually running — if no
        // daemon is up there's nothing for SDL to route to, and VD001
        // already owns that story.
        let active = facts.im.active_framework;
        if !matches!(active, ActiveFramework::Ibus | ActiveFramework::Fcitx5) {
            return vec![];
        }
        if facts.env.sdl_im_module.is_some() {
            return vec![];
        }

        vec![Issue {
            id: "VD004".to_owned(),
            severity: Severity::Warn,
            title: "SDL_IM_MODULE is unset".to_owned(),
            detail: format!(
                "SDL2 apps (games, emulators, Steam overlay) need \
                 SDL_IM_MODULE set to `{}` to route Vietnamese input \
                 through {}. Without it those apps see raw keycodes.",
                match active {
                    ActiveFramework::Ibus => "ibus",
                    ActiveFramework::Fcitx5 => "fcitx",
                    _ => unreachable!(),
                },
                active.as_single().display()
            ),
            facts_evidence: vec![
                "SDL_IM_MODULE: unset".to_owned(),
                format!("active: {}", active.as_single().display()),
            ],
            recommendation: Some("VR004".to_owned()),
        }]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vietime_core::{EnvFacts, ImFacts};

    fn env_with_sdl(val: Option<&str>) -> EnvFacts {
        if let Some(v) = val {
            let map: HashMap<String, String> =
                [("SDL_IM_MODULE".to_owned(), v.to_owned())].into_iter().collect();
            EnvFacts::from_env(&map)
        } else {
            EnvFacts::default()
        }
    }

    #[test]
    fn fires_when_framework_active_and_sdl_unset() {
        let facts = Facts {
            im: ImFacts { active_framework: ActiveFramework::Fcitx5, ..ImFacts::default() },
            env: env_with_sdl(None),
            ..Facts::default()
        };
        let out = Vd004.check(&facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warn);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR004"));
    }

    #[test]
    fn silent_when_sdl_set() {
        let facts = Facts {
            im: ImFacts { active_framework: ActiveFramework::Ibus, ..ImFacts::default() },
            env: env_with_sdl(Some("ibus")),
            ..Facts::default()
        };
        assert!(Vd004.check(&facts).is_empty());
    }

    #[test]
    fn silent_when_no_framework_active() {
        let facts = Facts {
            im: ImFacts { active_framework: ActiveFramework::None, ..ImFacts::default() },
            env: env_with_sdl(None),
            ..Facts::default()
        };
        assert!(Vd004.check(&facts).is_empty());
    }

    #[test]
    fn silent_on_conflict() {
        // Conflict is VD002's territory; don't pile on.
        let facts = Facts {
            im: ImFacts { active_framework: ActiveFramework::Conflict, ..ImFacts::default() },
            env: env_with_sdl(None),
            ..Facts::default()
        };
        assert!(Vd004.check(&facts).is_empty());
    }
}
