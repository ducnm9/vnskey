// SPDX-License-Identifier: GPL-3.0-or-later
//
// `im.fcitx5.daemon` — parallel to `im.ibus.daemon` but for Fcitx5.
//
//   * D-Bus name: `org.fcitx.Fcitx5`
//   * pgrep pattern: `pgrep -x fcitx5`
//   * version: `fcitx5 --version` (output like `fcitx5 5.1.12`)
//   * config dir: `$HOME/.config/fcitx5`
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-22).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::Fcitx5Facts;

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};
use crate::detectors::ibus_daemon::{parse_ibus_version, resolve_home_subdir};
use crate::process::{CommandRunner, DbusProbe, TokioCommandRunner, ZbusProbe};

const FCITX5_BUS_NAME: &str = "org.fcitx.Fcitx5";

#[derive(Debug)]
pub struct Fcitx5DaemonDetector {
    runner: Arc<dyn CommandRunner>,
    dbus: Arc<dyn DbusProbe>,
}

impl Default for Fcitx5DaemonDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Fcitx5DaemonDetector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            runner: Arc::new(TokioCommandRunner::with_timeout(Duration::from_secs(2))),
            dbus: Arc::new(ZbusProbe::new()),
        }
    }

    #[must_use]
    pub fn with_deps(runner: Arc<dyn CommandRunner>, dbus: Arc<dyn DbusProbe>) -> Self {
        Self { runner, dbus }
    }
}

#[async_trait]
impl Detector for Fcitx5DaemonDetector {
    fn id(&self) -> &'static str {
        "im.fcitx5.daemon"
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(3)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let dbus_said_running =
            self.dbus.name_has_owner(FCITX5_BUS_NAME).await.unwrap_or_else(|e| {
                debug!("dbus probe failed: {e}");
                false
            });

        let pgrep_pid = match self.runner.run("pgrep", &["-x", "fcitx5"]).await {
            Ok(stdout) => stdout.lines().next().and_then(|l| l.trim().parse::<u32>().ok()),
            Err(e) => {
                debug!("pgrep failed: {e}");
                None
            }
        };

        let daemon_running = dbus_said_running || pgrep_pid.is_some();

        let version = match self.runner.run("fcitx5", &["--version"]).await {
            Ok(stdout) => parse_ibus_version(&stdout),
            Err(e) => {
                debug!("fcitx5 --version failed: {e}");
                None
            }
        };

        let config_dir = resolve_home_subdir(ctx, ".config/fcitx5");

        let facts = Fcitx5Facts {
            version,
            daemon_running,
            daemon_pid: pgrep_pid,
            config_dir,
            addons_enabled: vec![],
            input_methods_configured: vec![],
        };

        let mut notes = Vec::new();
        if dbus_said_running {
            notes.push(format!("dbus: {FCITX5_BUS_NAME} has owner"));
        }
        if let Some(pid) = pgrep_pid {
            notes.push(format!("pgrep: fcitx5 pid={pid}"));
        }

        Ok(DetectorOutput {
            partial: PartialFacts { fcitx5: Some(facts), ..PartialFacts::default() },
            notes,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::process::tests::{FakeCommandRunner, FakeDbus};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn ctx_with_home(home: &str) -> DetectorContext {
        let mut env = HashMap::new();
        env.insert("HOME".to_owned(), home.to_owned());
        DetectorContext { env, sysroot: None, target_app: None }
    }

    fn runner(pgrep: Option<&str>, version: Option<&str>) -> FakeCommandRunner {
        let mut r = FakeCommandRunner::default();
        if let Some(o) = pgrep {
            r.ok.insert(("pgrep".to_owned(), "-x fcitx5".to_owned()), o.to_owned());
        }
        if let Some(o) = version {
            r.ok.insert(("fcitx5".to_owned(), "--version".to_owned()), o.to_owned());
        }
        r
    }

    #[tokio::test]
    async fn dbus_and_pgrep_agree_full_facts() {
        let r = runner(Some("2222\n"), Some("fcitx5 5.1.12\n"));
        let dbus =
            FakeDbus { owners: [FCITX5_BUS_NAME.to_owned()].into_iter().collect(), fail: false };
        let det = Fcitx5DaemonDetector::with_deps(Arc::new(r), Arc::new(dbus));
        let res = det.run(&ctx_with_home("/home/bob")).await.expect("ok");
        let f = res.partial.fcitx5.expect("fcitx5 set");
        assert!(f.daemon_running);
        assert_eq!(f.daemon_pid, Some(2222));
        assert_eq!(f.version.as_deref(), Some("5.1.12"));
        assert_eq!(f.config_dir, Some(PathBuf::from("/home/bob/.config/fcitx5")));
    }

    #[tokio::test]
    async fn dbus_fails_pgrep_finds_daemon() {
        let r = runner(Some("42\n"), Some("fcitx5 5.1.12"));
        let dbus = FakeDbus::default();
        let det = Fcitx5DaemonDetector::with_deps(Arc::new(r), Arc::new(dbus));
        let res = det.run(&ctx_with_home("/home/a")).await.expect("ok");
        let f = res.partial.fcitx5.expect("fcitx5 set");
        assert!(f.daemon_running);
        assert_eq!(f.daemon_pid, Some(42));
    }

    #[tokio::test]
    async fn neither_signal_sees_daemon() {
        let r = runner(Some(""), Some(""));
        let dbus = FakeDbus::default();
        let det = Fcitx5DaemonDetector::with_deps(Arc::new(r), Arc::new(dbus));
        let res = det.run(&ctx_with_home("/home/a")).await.expect("ok");
        let f = res.partial.fcitx5.expect("fcitx5 set");
        assert!(!f.daemon_running);
    }

    #[tokio::test]
    async fn missing_home_means_no_config_dir() {
        let r = runner(None, None);
        let dbus = FakeDbus::default();
        let det = Fcitx5DaemonDetector::with_deps(Arc::new(r), Arc::new(dbus));
        let res = det.run(&DetectorContext::default()).await.expect("ok");
        let f = res.partial.fcitx5.expect("fcitx5 set");
        assert_eq!(f.config_dir, None);
    }

    #[tokio::test]
    async fn id_is_im_fcitx5_daemon() {
        let d = Fcitx5DaemonDetector::new();
        assert_eq!(d.id(), "im.fcitx5.daemon");
    }
}
