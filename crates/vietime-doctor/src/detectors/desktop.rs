// SPDX-License-Identifier: GPL-3.0-or-later
//
// `sys.desktop` — identifies the desktop environment from env vars.
//
// Version extraction (e.g. `gnome-shell --version`) is deferred to a
// follow-up detector so this one stays side-effect-free.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3.

use std::time::Duration;

use async_trait::async_trait;

use vietime_core::detect_desktop_from_env;

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};

#[derive(Debug, Default)]
pub struct DesktopDetector;

impl DesktopDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Detector for DesktopDetector {
    fn id(&self) -> &'static str {
        "sys.desktop"
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(100)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let desktop = detect_desktop_from_env(&ctx.env);
        Ok(DetectorOutput {
            partial: PartialFacts { desktop, ..PartialFacts::default() },
            notes: vec![],
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vietime_core::DesktopEnv;

    fn make_ctx(env: &[(&str, &str)]) -> DetectorContext {
        let env: HashMap<String, String> =
            env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        DetectorContext { env, sysroot: None, target_app: None }
    }

    #[tokio::test]
    async fn detects_gnome_from_xdg_current_desktop() {
        let ctx = make_ctx(&[("XDG_CURRENT_DESKTOP", "ubuntu:GNOME")]);
        let out = DesktopDetector::new().run(&ctx).await.expect("detector ok");
        assert_eq!(out.partial.desktop, Some(DesktopEnv::Gnome { version: None }));
    }

    #[tokio::test]
    async fn no_desktop_when_env_empty() {
        let ctx = DetectorContext::default();
        let out = DesktopDetector::new().run(&ctx).await.expect("detector ok");
        assert_eq!(out.partial.desktop, None);
    }
}
