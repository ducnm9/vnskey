// SPDX-License-Identifier: GPL-3.0-or-later
//
// `im.ibus.daemon` — probes whether `ibus-daemon` is running, what pid,
// what version.
//
// Detection uses two complementary signals:
//
//   1. **D-Bus** (`org.freedesktop.IBus` on the session bus) — the
//      authoritative answer but not always available (headless CI, some
//      Flatpak sandboxes).
//   2. **`pgrep -x ibus-daemon`** — the pid lookup, and a fallback when
//      D-Bus is unreachable.
//
// Version comes from `ibus --version`, parsed conservatively. We tolerate
// malformed output by leaving `version = None` rather than erroring;
// consumers shouldn't see a daemon "disappear" because its version string
// changed shape.
//
// `config_dir` is always `$HOME/.config/ibus` if `HOME` is set, resolving
// through any test `sysroot` like `env_home_profile` does. The directory
// need not exist — IBus creates it on first run, and whether it exists is
// a weaker signal than whether the daemon is up.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-20).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::IbusFacts;

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};
use crate::process::{CommandRunner, DbusProbe, TokioCommandRunner, ZbusProbe};

const IBUS_BUS_NAME: &str = "org.freedesktop.IBus";

#[derive(Debug)]
pub struct IbusDaemonDetector {
    runner: Arc<dyn CommandRunner>,
    dbus: Arc<dyn DbusProbe>,
}

impl Default for IbusDaemonDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl IbusDaemonDetector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            runner: Arc::new(TokioCommandRunner::with_timeout(Duration::from_secs(2))),
            dbus: Arc::new(ZbusProbe::new()),
        }
    }

    /// Test seam: inject fake deps.
    #[must_use]
    pub fn with_deps(runner: Arc<dyn CommandRunner>, dbus: Arc<dyn DbusProbe>) -> Self {
        Self { runner, dbus }
    }
}

#[async_trait]
impl Detector for IbusDaemonDetector {
    fn id(&self) -> &'static str {
        "im.ibus.daemon"
    }

    fn timeout(&self) -> Duration {
        // Internal probes each cap at ~2s; 3s is the orchestrator-level
        // belt so a wedged D-Bus can't starve siblings.
        Duration::from_secs(3)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        // --- daemon running? --------------------------------------------
        let dbus_said_running = self.dbus.name_has_owner(IBUS_BUS_NAME).await.unwrap_or_else(|e| {
            debug!("dbus probe failed: {e}");
            false
        });

        let pgrep_pid = match self.runner.run("pgrep", &["-x", "ibus-daemon"]).await {
            Ok(stdout) => parse_first_pid(&stdout),
            Err(e) => {
                debug!("pgrep failed: {e}");
                None
            }
        };

        let daemon_running = dbus_said_running || pgrep_pid.is_some();

        // --- version ----------------------------------------------------
        let version = match self.runner.run("ibus", &["--version"]).await {
            Ok(stdout) => parse_ibus_version(&stdout),
            Err(e) => {
                debug!("ibus --version failed: {e}");
                None
            }
        };

        // --- config dir -------------------------------------------------
        let config_dir = resolve_home_subdir(ctx, ".config/ibus");

        // If *nothing* worked — no daemon, no version, no config dir — we
        // still emit an `IbusFacts` with all-default values. That's more
        // useful to the checker layer than `None`: it signals "we looked".
        let facts = IbusFacts {
            version,
            daemon_running,
            daemon_pid: pgrep_pid,
            config_dir,
            registered_engines: vec![],
        };

        let mut notes = Vec::new();
        if dbus_said_running {
            notes.push(format!("dbus: {IBUS_BUS_NAME} has owner"));
        }
        if let Some(pid) = pgrep_pid {
            notes.push(format!("pgrep: ibus-daemon pid={pid}"));
        }

        Ok(DetectorOutput {
            partial: PartialFacts { ibus: Some(facts), ..PartialFacts::default() },
            notes,
        })
    }
}

/// Extract the first integer pid from `pgrep`'s stdout (one pid per line).
fn parse_first_pid(stdout: &str) -> Option<u32> {
    stdout.lines().next().and_then(|l| l.trim().parse::<u32>().ok())
}

/// Parse `ibus --version` → version string.
///
/// Example output: `IBus 1.5.29`. We look for the first whitespace-
/// separated token that looks like a version (starts with a digit). This
/// is tolerant of future upstream reformatting.
pub(crate) fn parse_ibus_version(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        for tok in line.split_whitespace() {
            if tok.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return Some(tok.to_owned());
            }
        }
    }
    None
}

/// Resolve `$HOME/<rel>` against the context's optional sysroot (tests),
/// mirroring `env_home_profile.rs`'s pattern.
pub(crate) fn resolve_home_subdir(ctx: &DetectorContext, rel: &str) -> Option<PathBuf> {
    let home = ctx.env.get("HOME")?;
    let home_root: PathBuf = if let Some(sysroot) = ctx.sysroot.as_deref() {
        let stripped = Path::new(home).strip_prefix("/").unwrap_or(Path::new(home));
        sysroot.join(stripped)
    } else {
        PathBuf::from(home)
    };
    Some(home_root.join(rel))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::process::tests::{FakeCommandRunner, FakeDbus};
    use std::collections::HashMap;

    fn ctx_with_home(home: &str) -> DetectorContext {
        let mut env = HashMap::new();
        env.insert("HOME".to_owned(), home.to_owned());
        DetectorContext { env, sysroot: None }
    }

    fn runner_with(
        pgrep_out: Option<&str>,
        version_out: Option<&str>,
        pgrep_err: Option<std::io::ErrorKind>,
        version_err: Option<std::io::ErrorKind>,
    ) -> FakeCommandRunner {
        let mut r = FakeCommandRunner::default();
        if let Some(o) = pgrep_out {
            r.ok.insert(("pgrep".to_owned(), "-x ibus-daemon".to_owned()), o.to_owned());
        }
        if let Some(k) = pgrep_err {
            r.err.insert(("pgrep".to_owned(), "-x ibus-daemon".to_owned()), k);
        }
        if let Some(o) = version_out {
            r.ok.insert(("ibus".to_owned(), "--version".to_owned()), o.to_owned());
        }
        if let Some(k) = version_err {
            r.err.insert(("ibus".to_owned(), "--version".to_owned()), k);
        }
        r
    }

    #[tokio::test]
    async fn dbus_and_pgrep_agree_full_facts() {
        let runner = runner_with(Some("2341\n"), Some("IBus 1.5.29\n"), None, None);
        let dbus =
            FakeDbus { owners: [IBUS_BUS_NAME.to_owned()].into_iter().collect(), fail: false };
        let det = IbusDaemonDetector::with_deps(Arc::new(runner), Arc::new(dbus));
        let res = det.run(&ctx_with_home("/home/alice")).await.expect("ok");
        let f = res.partial.ibus.expect("ibus set");
        assert!(f.daemon_running);
        assert_eq!(f.daemon_pid, Some(2341));
        assert_eq!(f.version.as_deref(), Some("1.5.29"));
        assert_eq!(f.config_dir, Some(PathBuf::from("/home/alice/.config/ibus")));
    }

    #[tokio::test]
    async fn dbus_fails_pgrep_finds_daemon() {
        let runner = runner_with(Some("999\n"), Some("IBus 1.5.29\n"), None, None);
        // Empty owners → name_has_owner returns Ok(false).
        let dbus = FakeDbus::default();
        let det = IbusDaemonDetector::with_deps(Arc::new(runner), Arc::new(dbus));
        let res = det.run(&ctx_with_home("/home/a")).await.expect("ok");
        let f = res.partial.ibus.expect("ibus set");
        assert!(f.daemon_running);
        assert_eq!(f.daemon_pid, Some(999));
    }

    #[tokio::test]
    async fn neither_dbus_nor_pgrep_report_running() {
        let runner = runner_with(Some(""), Some("IBus 1.5.29\n"), None, None);
        let dbus = FakeDbus::default();
        let det = IbusDaemonDetector::with_deps(Arc::new(runner), Arc::new(dbus));
        let res = det.run(&ctx_with_home("/home/a")).await.expect("ok");
        let f = res.partial.ibus.expect("ibus set");
        assert!(!f.daemon_running);
        assert_eq!(f.daemon_pid, None);
        // Version still reported even though daemon is down — harmless.
        assert_eq!(f.version.as_deref(), Some("1.5.29"));
    }

    #[tokio::test]
    async fn malformed_version_output_yields_none() {
        let runner = runner_with(None, Some("ibus version unknown\n"), None, None);
        let dbus = FakeDbus::default();
        let det = IbusDaemonDetector::with_deps(Arc::new(runner), Arc::new(dbus));
        let res = det.run(&ctx_with_home("/home/a")).await.expect("ok");
        let f = res.partial.ibus.expect("ibus set");
        assert_eq!(f.version, None);
    }

    #[tokio::test]
    async fn missing_home_means_no_config_dir() {
        let runner = runner_with(None, None, None, None);
        let dbus = FakeDbus::default();
        let det = IbusDaemonDetector::with_deps(Arc::new(runner), Arc::new(dbus));
        let res = det.run(&DetectorContext::default()).await.expect("ok");
        let f = res.partial.ibus.expect("ibus set");
        assert_eq!(f.config_dir, None);
    }

    #[tokio::test]
    async fn id_is_im_ibus_daemon() {
        let d = IbusDaemonDetector::new();
        assert_eq!(d.id(), "im.ibus.daemon");
    }
}
