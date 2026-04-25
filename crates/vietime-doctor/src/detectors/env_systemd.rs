// SPDX-License-Identifier: GPL-3.0-or-later
//
// `env.systemd` — reads the systemd user manager's environment block by
// shelling out to `systemctl --user show-environment` and parsing the
// output with [`vietime_core::parse_etc_environment`] (the output is
// plain `KEY=value\n` — compatible with that parser).
//
// Interacting with subprocesses makes the detector hard to unit-test
// hermetically, so the command is behind the [`EnvCommand`] trait.
// Production wiring uses [`SystemctlEnvCommand`]; tests inject a fake
// that returns whatever the test wants (incl. an `Err` to exercise the
// anomaly path).
//
// On `Err` the detector returns [`DetectorError::Other`] so the
// orchestrator records it as a run-level anomaly and carries on. That's
// the same behaviour the user would see if `systemctl` simply wasn't
// on `PATH` — fine for a diagnostic tool.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-13b).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use vietime_core::{parse_etc_environment, EnvFacts, EnvSource};

use crate::detector::{
    Detector, DetectorContext, DetectorError, DetectorOutput, DetectorResult, PartialFacts,
};

/// Small abstraction over "run `systemctl --user show-environment`"
/// so tests can inject a fake without spawning a real process.
#[async_trait]
pub trait EnvCommand: Send + Sync + std::fmt::Debug {
    /// Produce the stdout of the command. Errors should be surface-level
    /// (binary missing, user session dead) — not "systemctl returned
    /// non-zero"; that's rare enough we fold it into an empty output.
    async fn run(&self) -> Result<String, std::io::Error>;
}

/// Real `systemctl --user show-environment` wrapper.
#[derive(Debug, Default)]
pub struct SystemctlEnvCommand;

#[async_trait]
impl EnvCommand for SystemctlEnvCommand {
    async fn run(&self) -> Result<String, std::io::Error> {
        let out = tokio::process::Command::new("systemctl")
            .args(["--user", "show-environment"])
            .output()
            .await?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// Queries the systemd `--user` environment.
#[derive(Debug)]
pub struct SystemdEnvDetector {
    cmd: Arc<dyn EnvCommand>,
}

impl Default for SystemdEnvDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemdEnvDetector {
    #[must_use]
    pub fn new() -> Self {
        Self { cmd: Arc::new(SystemctlEnvCommand) }
    }

    /// Test seam: inject a fake [`EnvCommand`].
    #[must_use]
    pub fn with_command(cmd: Arc<dyn EnvCommand>) -> Self {
        Self { cmd }
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
        let raw = match self.cmd.run().await {
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

    #[derive(Debug)]
    struct FakeOk(String);

    #[async_trait]
    impl EnvCommand for FakeOk {
        async fn run(&self) -> Result<String, std::io::Error> {
            Ok(self.0.clone())
        }
    }

    #[derive(Debug)]
    struct FakeErr(std::io::ErrorKind);

    #[async_trait]
    impl EnvCommand for FakeErr {
        async fn run(&self) -> Result<String, std::io::Error> {
            Err(std::io::Error::new(self.0, "boom"))
        }
    }

    #[tokio::test]
    async fn parses_typical_systemctl_output() {
        let out = "GTK_IM_MODULE=fcitx\nQT_IM_MODULE=fcitx\nXMODIFIERS=@im=fcitx\n";
        let det = SystemdEnvDetector::with_command(Arc::new(FakeOk(out.to_owned())));
        let res = det.run(&DetectorContext::default()).await.expect("detector ok");
        let facts = res.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("fcitx"));
        assert_eq!(facts.qt_im_module.as_deref(), Some("fcitx"));
        assert_eq!(facts.xmodifiers.as_deref(), Some("@im=fcitx"));
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::SystemdUserEnv));
    }

    #[tokio::test]
    async fn empty_output_is_not_a_failure() {
        let det = SystemdEnvDetector::with_command(Arc::new(FakeOk(String::new())));
        let res = det.run(&DetectorContext::default()).await.expect("detector ok");
        // Empty stdout → nothing to contribute, but also no anomaly.
        assert!(res.partial.env.is_none());
    }

    #[tokio::test]
    async fn missing_systemctl_produces_detector_error() {
        let det = SystemdEnvDetector::with_command(Arc::new(FakeErr(std::io::ErrorKind::NotFound)));
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
        let det = SystemdEnvDetector::with_command(Arc::new(FakeErr(
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
