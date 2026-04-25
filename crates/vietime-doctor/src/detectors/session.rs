// SPDX-License-Identifier: GPL-3.0-or-later
//
// `sys.session` — identifies whether we're on X11, Wayland, or a plain TTY.
//
// All the logic lives in `vietime_core::detect_session_from_env`; this
// detector is a thin adapter that fetches the env from `DetectorContext`.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3.

use std::time::Duration;

use async_trait::async_trait;

use vietime_core::detect_session_from_env;

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};

#[derive(Debug, Default)]
pub struct SessionDetector;

impl SessionDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Detector for SessionDetector {
    fn id(&self) -> &'static str {
        "sys.session"
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(100)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let session = detect_session_from_env(&ctx.env);
        Ok(DetectorOutput {
            partial: PartialFacts { session: Some(session), ..PartialFacts::default() },
            notes: vec![],
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vietime_core::SessionType;

    fn make_ctx(env: &[(&str, &str)]) -> DetectorContext {
        let env: HashMap<String, String> =
            env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        DetectorContext { env, sysroot: None }
    }

    #[tokio::test]
    async fn detects_wayland_when_xdg_type_is_wayland() {
        let ctx = make_ctx(&[("XDG_SESSION_TYPE", "wayland")]);
        let out = SessionDetector::new().run(&ctx).await.expect("detector ok");
        assert_eq!(out.partial.session, Some(SessionType::Wayland));
    }

    #[tokio::test]
    async fn detects_x11_when_only_display_is_set() {
        let ctx = make_ctx(&[("DISPLAY", ":0")]);
        let out = SessionDetector::new().run(&ctx).await.expect("detector ok");
        assert_eq!(out.partial.session, Some(SessionType::X11));
    }

    #[tokio::test]
    async fn unknown_when_env_is_empty() {
        let ctx = DetectorContext::default();
        let out = SessionDetector::new().run(&ctx).await.expect("detector ok");
        assert_eq!(out.partial.session, Some(SessionType::Unknown));
    }
}
