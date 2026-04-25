// SPDX-License-Identifier: GPL-3.0-or-later
//
// `env.systemd` — reads the systemd user manager's environment block by
// shelling out to `systemctl --user show-environment` and parsing the
// output with [`vietime_core::parse_etc_environment`] (the output is
// plain `KEY=value\n` — compatible with that parser).
//
// Subprocess invocation goes through the shared
// [`crate::process::CommandRunner`] seam introduced in Week 3, so tests
// inject a fake without spawning a real process. On `Err` the detector
// returns [`DetectorError::Other`] so the orchestrator records a
// run-level anomaly and carries on.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-13b).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use vietime_core::{parse_etc_environment, EnvFacts, EnvSource};

use crate::detector::{
    Detector, DetectorContext, DetectorError, DetectorOutput, DetectorResult, PartialFacts,
};
use crate::process::{CommandRunner, TokioCommandRunner};

/// Queries the systemd `--user` environment.
#[derive(Debug)]
pub struct SystemdEnvDetector {
    runner: Arc<dyn CommandRunner>,
}

impl Default for SystemdEnvDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemdEnvDetector {
    #[must_use]
    pub fn new() -> Self {
        // 2s sub-timeout — the JoinSet-level 3s is the belt, this is the
        // braces.
        Self { runner: Arc::new(TokioCommandRunner::with_timeout(Duration::from_secs(2))) }
    }

    /// Test seam: inject a fake [`CommandRunner`].
    #[must_use]
    pub fn with_runner(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Detector for SystemdEnvDetector {
    fn id(&self) -> &'static str {
        "env.systemd"
    }

    fn timeout(&self) -> Duration {
        // `systemctl --user` on a laggy box can take a second. 3s keeps
        // headroom without letting a wedged manager stall the whole run.
        Duration::from_secs(3)
    }

    async fn run(&self, _ctx: &DetectorContext) -> DetectorResult {
        let raw = match self.runner.run("systemctl", &["--user", "show-environment"]).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // No systemctl on PATH (unusual outside containers but not a
                // programmer bug) — return Other so it surfaces as an anomaly
                // rather than silently missing data the report depends on.
                return Err(DetectorError::Other(format!("systemctl not available: {e}")));
            }
            Err(e) => {
                return Err(DetectorError::Other(format!(
                    "systemctl --user show-environment failed: {e}"
                )));
            }
        };

        let kv = parse_etc_environment(&raw);
        if kv.is_empty() {
            // Manager returned no env at all (early boot, user session
            // never started). Not an error; just nothing to add.
            return Ok(DetectorOutput::default());
        }
        let facts = EnvFacts::from_env_with_source(&kv, EnvSource::SystemdUserEnv);
        Ok(DetectorOutput {
            partial: PartialFacts { env: Some(facts), ..PartialFacts::default() },
            notes: vec!["systemctl --user show-environment".to_owned()],
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::process::tests::FakeCommandRunner;

    fn fake(stdout: &str) -> FakeCommandRunner {
        let mut r = FakeCommandRunner::default();
        r.ok.insert(
            ("systemctl".to_owned(), "--user show-environment".to_owned()),
            stdout.to_owned(),
        );
        r
    }

    fn fake_err(kind: std::io::ErrorKind) -> FakeCommandRunner {
        let mut r = FakeCommandRunner::default();
        r.err.insert(("systemctl".to_owned(), "--user show-environment".to_owned()), kind);
        r
    }

    #[tokio::test]
    async fn parses_typical_systemctl_output() {
        let out = "GTK_IM_MODULE=fcitx\nQT_IM_MODULE=fcitx\nXMODIFIERS=@im=fcitx\n";
        let det = SystemdEnvDetector::with_runner(Arc::new(fake(out)));
        let res = det.run(&DetectorContext::default()).await.expect("detector ok");
        let facts = res.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("fcitx"));
        assert_eq!(facts.qt_im_module.as_deref(), Some("fcitx"));
        assert_eq!(facts.xmodifiers.as_deref(), Some("@im=fcitx"));
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::SystemdUserEnv));
    }

    #[tokio::test]
    async fn empty_output_is_not_a_failure() {
        let det = SystemdEnvDetector::with_runner(Arc::new(fake("")));
        let res = det.run(&DetectorContext::default()).await.expect("detector ok");
        // Empty stdout → nothing to contribute, but also no anomaly.
        assert!(res.partial.env.is_none());
    }

    #[tokio::test]
    async fn missing_systemctl_produces_detector_error() {
        let det = SystemdEnvDetector::with_runner(Arc::new(fake_err(std::io::ErrorKind::NotFound)));
        let err = det.run(&DetectorContext::default()).await.expect_err("should error");
        match err {
            DetectorError::Other(msg) => {
                assert!(msg.contains("systemctl"), "message should cite systemctl: {msg}");
            }
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn other_io_errors_also_surface_as_detector_error() {
        let det = SystemdEnvDetector::with_runner(Arc::new(fake_err(
            std::io::ErrorKind::PermissionDenied,
        )));
        let err = det.run(&DetectorContext::default()).await.expect_err("should error");
        assert!(matches!(err, DetectorError::Other(_)));
    }

    #[tokio::test]
    async fn id_is_env_systemd() {
        let d = SystemdEnvDetector::new();
        assert_eq!(d.id(), "env.systemd");
    }
}
