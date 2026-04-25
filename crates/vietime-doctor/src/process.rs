// SPDX-License-Identifier: GPL-3.0-or-later
//
// Shared subprocess + D-Bus seams for Week 3 IM detectors.
//
// Week 2's `env_systemd.rs` had its own one-off trait for mocking
// `systemctl --user show-environment`. Week 3 adds at least five more
// subprocess callers (`ibus --version`, `ibus list-engine`, `fcitx5
// --version`, `pgrep`, `dpkg-query`/`rpm`/`pacman`) and two D-Bus probes
// (`org.freedesktop.IBus`, `org.fcitx.Fcitx5`). A single shared seam per
// kind keeps the injection surface small across detectors.
//
// Design:
//
// * `CommandRunner` — run a program with args, produce stdout as a
//   string. Any failure (spawn error, non-zero exit) surfaces as
//   `io::Error`. Callers decide whether to map that to
//   `DetectorError::Other` or silently fall back.
// * `TokioCommandRunner` — the real `tokio::process::Command` impl, with
//   a per-call timeout so a wedged subprocess can't wait out the whole
//   orchestrator budget on its own.
// * `DbusProbe` — "does name X own the session bus?" — the smallest
//   question our framework detectors need to answer. Real impl
//   (`ZbusProbe`) lazily connects to the session bus; on connection
//   failure it returns `Ok(false)` so headless CI (no session bus) falls
//   through to the `pgrep` fallback instead of spamming anomalies.
//
// Tests never exercise the real impls: they inject fakes (see the
// end-of-file `mod tests`).
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-20, DOC-22).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::OnceCell;

/// Run an external command and capture its stdout.
///
/// Implementations must be `Send + Sync + Debug` so detectors can hold
/// them behind `Arc<dyn CommandRunner>` and move them across the
/// orchestrator's `JoinSet`.
#[async_trait]
pub trait CommandRunner: Send + Sync + std::fmt::Debug {
    /// Execute `program` with `args`. Return the captured stdout as a
    /// UTF-8 string (lossy — binary output is a programmer error here,
    /// not a safety concern).
    ///
    /// Errors:
    /// * `NotFound` if the binary isn't on `PATH`.
    /// * `TimedOut` if the per-call timeout expired.
    /// * Any other `io::Error` from `tokio::process::Command`.
    ///
    /// A non-zero exit is **not** an error — we still return stdout,
    /// because many of our subprocesses (pgrep, dpkg-query) use non-zero
    /// to mean "no match" rather than "something broke". Callers that
    /// care about exit status should inspect the returned string instead.
    async fn run(&self, program: &str, args: &[&str]) -> Result<String, std::io::Error>;
}

/// Real `tokio::process::Command`-backed runner.
///
/// The per-call `timeout` is independent of the `Detector::timeout()`
/// that the orchestrator enforces — it's an extra belt so a single
/// subprocess that hangs can't starve siblings running inside the same
/// detector (e.g. `ibus --version` + `pgrep` in `IbusDaemonDetector`).
#[derive(Debug, Clone)]
pub struct TokioCommandRunner {
    pub timeout: Duration,
}

impl Default for TokioCommandRunner {
    fn default() -> Self {
        Self { timeout: Duration::from_secs(2) }
    }
}

impl TokioCommandRunner {
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

#[async_trait]
impl CommandRunner for TokioCommandRunner {
    async fn run(&self, program: &str, args: &[&str]) -> Result<String, std::io::Error> {
        let fut = async {
            let out =
                tokio::process::Command::new(program).args(args).output().await.map_err(|e| {
                    // Bubble up the original kind so callers can match on
                    // `ErrorKind::NotFound` without string parsing.
                    std::io::Error::new(e.kind(), format!("spawning {program}: {e}"))
                })?;
            Ok::<_, std::io::Error>(String::from_utf8_lossy(&out.stdout).into_owned())
        };
        match tokio::time::timeout(self.timeout, fut).await {
            Ok(res) => res,
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("{program} exceeded {:?}", self.timeout),
            )),
        }
    }
}

/// Ask "does `name` currently own the session bus?".
///
/// We deliberately expose the smallest useful question instead of a
/// generic `Connection` handle — this keeps the mock surface tiny and
/// makes the `zbus` dep swappable for an alternative D-Bus library later
/// without changing detector code.
#[async_trait]
pub trait DbusProbe: Send + Sync + std::fmt::Debug {
    /// Returns `Ok(true)` iff `name` currently has an owner on the
    /// session bus.
    ///
    /// A failure to reach the session bus at all surfaces as `Ok(false)`
    /// (headless CI, no DISPLAY/DBUS vars) — that's not an error because
    /// the detectors always have a `pgrep` fallback for the same signal.
    /// Genuine I/O errors (bus reachable but call failed) propagate as
    /// `Err`.
    async fn name_has_owner(&self, name: &str) -> Result<bool, std::io::Error>;
}

/// Real `zbus`-backed probe.
///
/// The session-bus connection is lazily constructed on first call and
/// reused for subsequent calls within the same `Arc<dyn DbusProbe>`. On
/// connection failure we cache the error as `None` so we don't retry
/// (and spam logs) on every call.
#[derive(Debug, Default)]
pub struct ZbusProbe {
    conn: OnceCell<Option<zbus::Connection>>,
}

impl ZbusProbe {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    async fn conn(&self) -> Option<&zbus::Connection> {
        self.conn.get_or_init(|| async { zbus::Connection::session().await.ok() }).await.as_ref()
    }
}

#[async_trait]
impl DbusProbe for ZbusProbe {
    async fn name_has_owner(&self, name: &str) -> Result<bool, std::io::Error> {
        let Some(conn) = self.conn().await else {
            // No session bus (headless CI / Flatpak sandbox / early
            // boot). Fall through silently — detectors always have a
            // pgrep fallback.
            return Ok(false);
        };
        // Use the well-known `org.freedesktop.DBus.NameHasOwner` method.
        let reply: Result<bool, zbus::Error> = conn
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "NameHasOwner",
                &name,
            )
            .await
            .and_then(|msg| msg.body().deserialize::<bool>());
        match reply {
            Ok(v) => Ok(v),
            Err(e) => Err(std::io::Error::other(e.to_string())),
        }
    }
}

/// Convenience alias — most detectors hold their command runner in an `Arc`.
pub type SharedRunner = Arc<dyn CommandRunner>;
/// Convenience alias — same for D-Bus.
pub type SharedDbus = Arc<dyn DbusProbe>;

/// Walk `/proc/*/cmdline` and return the argv of every process whose
/// argv[0] matches `binary`.
///
/// The Electron detector (DOC-32) uses this to answer "is this app
/// currently running, and if so did it get `--ozone-platform=wayland`?".
/// We expose it as a trait so tests can feed curated process lists
/// without touching the real `/proc`.
#[async_trait]
pub trait ProcScanner: Send + Sync + std::fmt::Debug {
    /// Return argv vectors of every live process whose argv[0] equals
    /// `binary` (or whose basename equals `binary`'s basename — the same
    /// app can appear under `/opt/foo/foo` and `foo` in cmdline depending
    /// on how it was launched).
    ///
    /// Returning an empty `Vec` means "no matching processes running" —
    /// **not** an error. I/O failures reading `/proc` are swallowed into
    /// an empty vec with a `tracing::debug!` trace, because a sysfs
    /// hiccup shouldn't fail the detector.
    async fn find_processes(&self, binary: &std::path::Path) -> Vec<Vec<String>>;
}

/// Real `/proc`-backed scanner. Iterates numeric subdirectories, reads
/// `cmdline` (NUL-separated argv), matches the first token against the
/// target path.
#[derive(Debug, Default)]
pub struct ProcfsScanner;

impl ProcfsScanner {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProcScanner for ProcfsScanner {
    async fn find_processes(&self, binary: &std::path::Path) -> Vec<Vec<String>> {
        // Do all the blocking fs work on the Tokio blocking pool so we
        // don't stall the runtime. /proc is fast but iterating it is
        // still a syscall per pid.
        let target = binary.to_owned();
        tokio::task::spawn_blocking(move || scan_procfs(&target)).await.unwrap_or_else(|e| {
            tracing::debug!("proc scan join error: {e}");
            Vec::new()
        })
    }
}

fn scan_procfs(binary: &std::path::Path) -> Vec<Vec<String>> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let target_name = binary.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let target_str = binary.to_str().unwrap_or("");
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let cmdline_path = entry.path().join("cmdline");
        let Ok(bytes) = std::fs::read(&cmdline_path) else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        let argv: Vec<String> = bytes
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect();
        let Some(argv0) = argv.first() else { continue };
        let argv0_base =
            std::path::Path::new(argv0).file_name().and_then(|s| s.to_str()).unwrap_or(argv0);
        if argv0 == target_str || (!target_name.is_empty() && argv0_base == target_name) {
            out.push(argv);
        }
    }
    out
}

/// Convenience alias.
pub type SharedProcScanner = Arc<dyn ProcScanner>;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex;

    /// Test fake: looks up `(program, joined args)` in a map to decide
    /// what to return. Gives us a compact way to configure multiple
    /// subprocess responses in one place per test.
    #[derive(Debug, Default)]
    pub struct FakeCommandRunner {
        /// Map: `(program, joined args)` → stdout.
        pub ok: std::collections::HashMap<(String, String), String>,
        /// Map: `(program, joined args)` → error kind to return.
        pub err: std::collections::HashMap<(String, String), std::io::ErrorKind>,
        /// Recording of every call made, in order.
        pub calls: Mutex<Vec<(String, Vec<String>)>>,
    }

    #[async_trait]
    impl CommandRunner for FakeCommandRunner {
        async fn run(&self, program: &str, args: &[&str]) -> Result<String, std::io::Error> {
            self.calls
                .lock()
                .unwrap()
                .push((program.to_owned(), args.iter().map(ToString::to_string).collect()));
            let key = (program.to_owned(), args.join(" "));
            if let Some(kind) = self.err.get(&key) {
                return Err(std::io::Error::new(*kind, "fake"));
            }
            Ok(self.ok.get(&key).cloned().unwrap_or_default())
        }
    }

    /// Test fake D-Bus: owns the names given at construction.
    #[derive(Debug, Default)]
    pub struct FakeDbus {
        pub owners: HashSet<String>,
        pub fail: bool,
    }

    /// Test fake proc scanner: returns a preset list of argv vectors for
    /// any binary path it's asked about. Keyed by the binary's basename so
    /// tests don't have to care whether the detector was given an absolute
    /// or relative path.
    #[derive(Debug, Default)]
    pub struct FakeProcScanner {
        /// Map: binary basename → list of argv vectors to return.
        pub by_basename: std::collections::HashMap<String, Vec<Vec<String>>>,
    }

    #[async_trait]
    impl ProcScanner for FakeProcScanner {
        async fn find_processes(&self, binary: &std::path::Path) -> Vec<Vec<String>> {
            let key =
                binary.file_name().and_then(|s| s.to_str()).map_or_else(String::new, str::to_owned);
            self.by_basename.get(&key).cloned().unwrap_or_default()
        }
    }

    #[async_trait]
    impl DbusProbe for FakeDbus {
        async fn name_has_owner(&self, name: &str) -> Result<bool, std::io::Error> {
            if self.fail {
                return Err(std::io::Error::other("fake dbus error"));
            }
            Ok(self.owners.contains(name))
        }
    }

    #[tokio::test]
    async fn fake_runner_returns_configured_stdout() {
        let mut r = FakeCommandRunner::default();
        r.ok.insert(("echo".to_owned(), "hello".to_owned()), "hi\n".to_owned());
        let out = r.run("echo", &["hello"]).await.expect("runs");
        assert_eq!(out, "hi\n");
        assert_eq!(r.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn fake_runner_surfaces_configured_error() {
        let mut r = FakeCommandRunner::default();
        r.err.insert(("ibus".to_owned(), "--version".to_owned()), std::io::ErrorKind::NotFound);
        let err = r.run("ibus", &["--version"]).await.expect_err("fails");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn fake_dbus_reports_ownership() {
        let fake = FakeDbus {
            owners: ["org.freedesktop.IBus".to_owned()].into_iter().collect(),
            fail: false,
        };
        assert!(fake.name_has_owner("org.freedesktop.IBus").await.expect("ok"));
        assert!(!fake.name_has_owner("org.fcitx.Fcitx5").await.expect("ok"));
    }

    #[tokio::test]
    async fn fake_dbus_failure_surfaces_as_err() {
        let fake = FakeDbus { owners: HashSet::new(), fail: true };
        let err = fake.name_has_owner("anything").await.expect_err("fails");
        assert_eq!(err.kind(), std::io::ErrorKind::Other);
    }

    #[tokio::test]
    async fn fake_proc_scanner_round_trips_argv_by_basename() {
        let mut fake = FakeProcScanner::default();
        fake.by_basename.insert(
            "code".to_owned(),
            vec![vec!["/usr/bin/code".to_owned(), "--ozone-platform=wayland".to_owned()]],
        );
        let found = fake.find_processes(std::path::Path::new("/opt/bin/code")).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0][1], "--ozone-platform=wayland");
        let none = fake.find_processes(std::path::Path::new("/usr/bin/firefox")).await;
        assert!(none.is_empty());
    }
}
