// SPDX-License-Identifier: GPL-3.0-or-later
//
// `env.process` — reads the 8 IM-relevant env vars from the context's env
// map (i.e. whatever Doctor itself inherited from its parent shell) and
// tags every populated field with [`EnvSource::Process`].
//
// This is the highest-priority env source: `merge_by_priority` on the
// orchestrator's merged `EnvFacts` will always prefer a Process-sourced
// value over anything the file-scanning detectors (DOC-11..13) find,
// because what the user sees in their own shell is what actually applies
// when they launch a graphical app.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-10).

use std::time::Duration;

use async_trait::async_trait;

use vietime_core::{EnvFacts, EnvSource};

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};

/// Reads IM env vars from the process environment that spawned Doctor.
#[derive(Debug, Default)]
pub struct ProcessEnvDetector;

impl ProcessEnvDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Detector for ProcessEnvDetector {
    fn id(&self) -> &'static str {
        "env.process"
    }

    fn timeout(&self) -> Duration {
        // Pure hashmap read — anything more than a handful of ms is a bug.
        Duration::from_millis(100)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let facts = EnvFacts::from_env_with_source(&ctx.env, EnvSource::Process);
        Ok(DetectorOutput {
            partial: PartialFacts { env: Some(facts), ..PartialFacts::default() },
            notes: vec![],
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_ctx(env: &[(&str, &str)]) -> DetectorContext {
        let env: HashMap<String, String> =
            env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        DetectorContext { env, sysroot: None, target_app: None }
    }

    #[tokio::test]
    async fn populates_all_fields_from_env() {
        let ctx = make_ctx(&[
            ("GTK_IM_MODULE", "ibus"),
            ("QT_IM_MODULE", "ibus"),
            ("QT4_IM_MODULE", "ibus"),
            ("XMODIFIERS", "@im=ibus"),
            ("INPUT_METHOD", "ibus"),
            ("SDL_IM_MODULE", "ibus"),
            ("GLFW_IM_MODULE", "ibus"),
            ("CLUTTER_IM_MODULE", "xim"),
        ]);
        let out = ProcessEnvDetector::new().run(&ctx).await.expect("detector ok");
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.qt_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.qt4_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.xmodifiers.as_deref(), Some("@im=ibus"));
        assert_eq!(facts.input_method.as_deref(), Some("ibus"));
        assert_eq!(facts.sdl_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.glfw_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.clutter_im_module.as_deref(), Some("xim"));
    }

    #[tokio::test]
    async fn tags_every_populated_field_as_process() {
        let ctx = make_ctx(&[
            ("GTK_IM_MODULE", "fcitx"),
            ("QT_IM_MODULE", "fcitx"),
            ("XMODIFIERS", "@im=fcitx"),
        ]);
        let out = ProcessEnvDetector::new().run(&ctx).await.expect("detector ok");
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::Process));
        assert_eq!(facts.sources.get("QT_IM_MODULE"), Some(&EnvSource::Process));
        assert_eq!(facts.sources.get("XMODIFIERS"), Some(&EnvSource::Process));
        // Unset vars don't appear in the sources map — keeps VD009 cites clean.
        assert!(!facts.sources.contains_key("SDL_IM_MODULE"));
    }

    #[tokio::test]
    async fn empty_env_emits_empty_facts() {
        let ctx = DetectorContext::default();
        let out = ProcessEnvDetector::new().run(&ctx).await.expect("detector ok");
        let facts = out.partial.env.expect("env facts present");
        assert!(!facts.has_any());
        assert!(facts.sources.is_empty());
    }

    #[tokio::test]
    async fn qt4_im_module_is_picked_up() {
        // Regression guard: the Week-2 spec bump added QT4_IM_MODULE to
        // `EnvFacts`; make sure the detector doesn't silently skip it.
        let ctx = make_ctx(&[("QT4_IM_MODULE", "fcitx")]);
        let out = ProcessEnvDetector::new().run(&ctx).await.expect("detector ok");
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.qt4_im_module.as_deref(), Some("fcitx"));
        assert_eq!(facts.sources.get("QT4_IM_MODULE"), Some(&EnvSource::Process));
    }

    #[tokio::test]
    async fn id_is_env_process() {
        let d = ProcessEnvDetector::new();
        assert_eq!(d.id(), "env.process");
    }
}
