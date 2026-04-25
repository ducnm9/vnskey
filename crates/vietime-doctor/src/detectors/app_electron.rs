// SPDX-License-Identifier: GPL-3.0-or-later
//
// `app.electron` — deep-dives the target app when its profile kind_hint is
// `Electron` or `Chromium`. Runs alongside `GenericAppDetector`; the
// orchestrator's `reconcile_app_facts` collapses the two outputs into a
// single `AppFacts` row.
//
// Two sub-tasks:
//
//   1. **Electron version** — scan the first ~10 MB of the executable for
//      a `Electron/<ver>` byte-string (Chromium apps embed `Chrome/<ver>`
//      instead). This matches the user-agent blob Chromium hardcodes into
//      every Blink binary; it's stable enough that asar parsing is a
//      Week-6 nicety, not a correctness requirement.
//   2. **Runtime Ozone/Wayland detection** — ask the injected
//      `ProcScanner` for every running process whose argv[0] matches the
//      target binary, and look at their argv for `--ozone-platform=wayland`
//      or `--enable-features=UseOzonePlatform`. The session type comes
//      from `ctx.env["XDG_SESSION_TYPE"]` (already populated by envctx),
//      so we can flag apps that *should* be Ozone-enabled but aren't.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-32).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::{AppFacts, AppKind};

use crate::apps::{resolve_app, resolve_binary};
use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};
use crate::process::{
    CommandRunner, ProcScanner, ProcfsScanner, SharedProcScanner, SharedRunner, TokioCommandRunner,
};

/// Cap on bytes we scan from the executable when hunting for the embedded
/// `Electron/<ver>` token. 10 MiB is enough to cover the user-agent blob
/// in every shipped Electron/Chromium build of the last five years without
/// dragging the whole binary into memory.
const MAX_SCAN_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug)]
pub struct ElectronAppDetector {
    runner: SharedRunner,
    proc: SharedProcScanner,
}

impl Default for ElectronAppDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ElectronAppDetector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            runner: Arc::new(TokioCommandRunner::with_timeout(Duration::from_secs(2))),
            proc: Arc::new(ProcfsScanner::new()),
        }
    }

    #[must_use]
    pub fn with_deps(runner: Arc<dyn CommandRunner>, proc: Arc<dyn ProcScanner>) -> Self {
        Self { runner, proc }
    }
}

#[async_trait]
impl Detector for ElectronAppDetector {
    fn id(&self) -> &'static str {
        "app.electron"
    }

    fn timeout(&self) -> Duration {
        // Binary scan is blocking I/O capped at 10 MiB; proc scan is
        // spawn_blocking; `which` is 2s. 4s covers the sum with margin.
        Duration::from_secs(4)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let Some(raw) = ctx.target_app.as_deref() else {
            return Ok(DetectorOutput::default());
        };
        let Some(profile) = resolve_app(raw) else {
            // GenericAppDetector already emitted the "unknown app" note —
            // we just exit silently.
            return Ok(DetectorOutput::default());
        };
        // Only meaningful for Electron / Chromium apps.
        if !matches!(profile.kind_hint, AppKind::Electron | AppKind::Chromium) {
            return Ok(DetectorOutput::default());
        }

        let Some(bin) = resolve_binary(&self.runner, profile, ctx.sysroot.as_deref()).await else {
            // No binary → nothing to probe. GenericAppDetector will have
            // already emitted the "not found" row.
            return Ok(DetectorOutput::default());
        };

        // --- Electron version string from binary ------------------------
        let electron_version = scan_electron_version(&bin).await;

        // --- Runtime Ozone flag scan ------------------------------------
        let running = self.proc.find_processes(&bin).await;
        let session_is_wayland =
            ctx.env.get("XDG_SESSION_TYPE").is_some_and(|v| v.eq_ignore_ascii_case("wayland"));
        let uses_wayland = derive_uses_wayland(&running, session_is_wayland);

        let mut notes = Vec::new();
        if let Some(v) = electron_version.as_deref() {
            notes.push(format!("{}: electron version {v} (binary scan)", profile.id));
        }
        match uses_wayland {
            Some(true) => notes.push(format!("{}: running with Ozone/Wayland", profile.id)),
            Some(false) if !running.is_empty() => {
                notes.push(format!("{}: running without --ozone-platform=wayland", profile.id));
            }
            Some(false) => {
                notes.push(format!("{}: not running; inferring no Ozone flag", profile.id));
            }
            None => {}
        }

        // We only emit an AppFacts contribution if we actually learned
        // something new — otherwise the reconcile pass has nothing to
        // merge.
        if electron_version.is_none() && uses_wayland.is_none() {
            return Ok(DetectorOutput::default());
        }

        let facts = AppFacts {
            app_id: profile.id.to_owned(),
            binary_path: bin,
            version: None,
            kind: profile.kind_hint.clone(),
            electron_version,
            uses_wayland,
            detector_notes: notes.clone(),
        };
        Ok(DetectorOutput {
            partial: PartialFacts { apps: vec![facts], ..PartialFacts::default() },
            notes,
        })
    }
}

/// Read up to `MAX_SCAN_BYTES` of `binary` and look for an embedded
/// `Electron/<ver>` byte-string. Falls back to `Chrome/<ver>` — Chromium
/// apps (the browser itself, as opposed to Electron wrappers like VS
/// Code) embed only the Chrome tag.
async fn scan_electron_version(binary: &Path) -> Option<String> {
    let path = binary.to_owned();
    tokio::task::spawn_blocking(move || scan_electron_version_blocking(&path)).await.unwrap_or_else(
        |e| {
            debug!("electron scan join error: {e}");
            None
        },
    )
}

fn scan_electron_version_blocking(binary: &Path) -> Option<String> {
    use std::io::Read;

    let Ok(f) = std::fs::File::open(binary) else {
        return None;
    };
    let mut buf = Vec::with_capacity(MAX_SCAN_BYTES.min(1 << 20));
    // Explicit bounded read so a /proc/self/maps or a FIFO doesn't try to
    // feed us infinity.
    let _ = f.take(MAX_SCAN_BYTES as u64).read_to_end(&mut buf);
    if buf.is_empty() {
        return None;
    }
    find_version_after_tag(&buf, b"Electron/").or_else(|| find_version_after_tag(&buf, b"Chrome/"))
}

/// Walk `haystack` looking for `tag` followed by a version-shaped token
/// ending at the first byte that isn't digit / dot / `-_.+a-zA-Z`. Return
/// the captured string if it looks plausible (two or more components).
fn find_version_after_tag(haystack: &[u8], tag: &[u8]) -> Option<String> {
    if tag.is_empty() || haystack.len() < tag.len() {
        return None;
    }
    let mut i = 0;
    while i + tag.len() <= haystack.len() {
        if &haystack[i..i + tag.len()] == tag {
            let start = i + tag.len();
            let mut end = start;
            while end < haystack.len() {
                let b = haystack[end];
                let ok = b.is_ascii_digit()
                    || b == b'.'
                    || b == b'-'
                    || b == b'_'
                    || b == b'+'
                    || b.is_ascii_alphabetic();
                if !ok {
                    break;
                }
                end += 1;
            }
            let slice = &haystack[start..end];
            if slice.contains(&b'.') && slice.first().is_some_and(u8::is_ascii_digit) {
                if let Ok(s) = std::str::from_utf8(slice) {
                    return Some(s.to_owned());
                }
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

/// Given the argv of every running copy of the app, plus whether the
/// session is Wayland, decide whether the app is using Ozone/Wayland.
///
/// Returns:
///
///  * `Some(true)`  — saw a matching process with an explicit Ozone flag.
///  * `Some(false)` — saw matching process(es), none had the flag, and we
///    have evidence (Wayland session) it matters; or no matching process
///    is running but the session is Wayland so a future launch would miss
///    Ozone by default.
///  * `None`        — session is X11 (Ozone is irrelevant) and we saw no
///    process, so we can't say.
fn derive_uses_wayland(running: &[Vec<String>], session_is_wayland: bool) -> Option<bool> {
    let has_flag = running.iter().any(|argv| argv.iter().any(|a| is_ozone_wayland_flag(a)));
    if has_flag {
        return Some(true);
    }
    if !running.is_empty() {
        // Running but no flag. A running Electron app under X11 can still
        // talk to Wayland if XDG_SESSION_TYPE says so — but here no flag
        // means no Ozone, period.
        return Some(false);
    }
    // Not running. Only speak up if the session is Wayland — that's the
    // case where "no --ozone-platform=wayland would hurt next time".
    if session_is_wayland {
        Some(false)
    } else {
        None
    }
}

fn is_ozone_wayland_flag(arg: &str) -> bool {
    // Two common forms:
    //   --ozone-platform=wayland
    //   --enable-features=UseOzonePlatform,WaylandWindowDecorations
    if arg == "--ozone-platform=wayland" || arg == "--ozone-platform-hint=wayland" {
        return true;
    }
    if let Some(rest) = arg.strip_prefix("--enable-features=") {
        return rest.split(',').any(|f| f == "UseOzonePlatform");
    }
    false
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::process::tests::{FakeCommandRunner, FakeProcScanner};
    use std::collections::HashMap;
    use std::io::Write;
    use std::path::PathBuf;

    fn runner(pairs: &[(&str, &[&str], &str)]) -> Arc<dyn CommandRunner> {
        let mut r = FakeCommandRunner::default();
        for (prog, args, out) in pairs {
            r.ok.insert(
                (
                    (*prog).to_owned(),
                    args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>().join(" "),
                ),
                (*out).to_owned(),
            );
        }
        Arc::new(r)
    }

    fn proc_with(map: &[(&str, Vec<Vec<String>>)]) -> Arc<dyn ProcScanner> {
        let mut by_basename = HashMap::new();
        for (k, v) in map {
            by_basename.insert((*k).to_owned(), v.clone());
        }
        Arc::new(FakeProcScanner { by_basename })
    }

    /// Create a temp file with the given bytes; returns the parent
    /// sysroot dir. An atomic counter prevents parallel-test collisions
    /// that `SystemTime::now()` alone doesn't.
    fn tmp_file(rel: &str, payload: &[u8]) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let base = std::env::var_os("TMPDIR")
            .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from)
            .join(format!("vietime-electron-{}-{n}", std::process::id()));
        let target = base.join(rel.strip_prefix('/').unwrap_or(rel));
        std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        let mut f = std::fs::File::create(&target).expect("create");
        f.write_all(payload).expect("write");
        base
    }

    fn ctx(app: &str, sysroot: Option<PathBuf>, env: &[(&str, &str)]) -> DetectorContext {
        let env: HashMap<String, String> =
            env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        DetectorContext { env, sysroot, target_app: Some(app.to_owned()) }
    }

    #[tokio::test]
    async fn target_app_none_emits_nothing() {
        let det = ElectronAppDetector::with_deps(runner(&[]), proc_with(&[]));
        let out = det.run(&DetectorContext::default()).await.expect("ok");
        assert!(out.partial.apps.is_empty());
    }

    #[tokio::test]
    async fn native_profile_emits_nothing() {
        let det = ElectronAppDetector::with_deps(runner(&[]), proc_with(&[]));
        let out = det.run(&ctx("firefox", None, &[])).await.expect("ok");
        assert!(out.partial.apps.is_empty());
    }

    #[tokio::test]
    async fn extracts_electron_version_from_binary_strings() {
        let payload = b"\x7fELF junk\0padding Electron/28.2.4\0Chrome/120.0.6099.109\0more junk";
        let sysroot = tmp_file("/usr/bin/code", payload);
        let det = ElectronAppDetector::with_deps(runner(&[]), proc_with(&[]));
        let out = det.run(&ctx("vscode", Some(sysroot), &[])).await.expect("ok");
        assert_eq!(out.partial.apps.len(), 1);
        let app = &out.partial.apps[0];
        assert_eq!(app.electron_version.as_deref(), Some("28.2.4"));
        // uses_wayland stays None: no process running, session unknown.
        assert!(app.uses_wayland.is_none());
    }

    #[tokio::test]
    async fn running_process_with_ozone_wayland_flag_sets_uses_wayland_true() {
        // Binary doesn't need real version bytes — just needs to exist so
        // resolve_binary returns something. Include a plausible Electron
        // tag so we also get version coverage.
        let payload = b"padding Electron/29.0.1\0";
        let sysroot = tmp_file("/usr/bin/code", payload);
        let det = ElectronAppDetector::with_deps(
            runner(&[]),
            proc_with(&[(
                "code",
                vec![vec![
                    "/usr/bin/code".to_owned(),
                    "--ozone-platform=wayland".to_owned(),
                    "--some-flag".to_owned(),
                ]],
            )]),
        );
        let out = det
            .run(&ctx("vscode", Some(sysroot), &[("XDG_SESSION_TYPE", "wayland")]))
            .await
            .expect("ok");
        let app = &out.partial.apps[0];
        assert_eq!(app.uses_wayland, Some(true));
        assert_eq!(app.electron_version.as_deref(), Some("29.0.1"));
    }

    #[tokio::test]
    async fn not_running_on_wayland_yields_some_false() {
        let payload = b"Electron/30.0.0\0";
        let sysroot = tmp_file("/usr/bin/code", payload);
        let det = ElectronAppDetector::with_deps(runner(&[]), proc_with(&[]));
        let out = det
            .run(&ctx("vscode", Some(sysroot), &[("XDG_SESSION_TYPE", "wayland")]))
            .await
            .expect("ok");
        let app = &out.partial.apps[0];
        assert_eq!(app.uses_wayland, Some(false));
    }

    #[tokio::test]
    async fn running_without_flag_yields_some_false() {
        let payload = b"Electron/28.0.0\0";
        let sysroot = tmp_file("/usr/bin/code", payload);
        let det = ElectronAppDetector::with_deps(
            runner(&[]),
            proc_with(&[("code", vec![vec!["/usr/bin/code".to_owned()]])]),
        );
        let out = det.run(&ctx("vscode", Some(sysroot), &[])).await.expect("ok");
        let app = &out.partial.apps[0];
        assert_eq!(app.uses_wayland, Some(false));
    }

    #[tokio::test]
    async fn enable_features_flag_also_counts_as_ozone_wayland() {
        let payload = b"Electron/27.0.0\0";
        let sysroot = tmp_file("/usr/bin/code", payload);
        let det = ElectronAppDetector::with_deps(
            runner(&[]),
            proc_with(&[(
                "code",
                vec![vec![
                    "/usr/bin/code".to_owned(),
                    "--enable-features=UseOzonePlatform,WaylandWindowDecorations".to_owned(),
                ]],
            )]),
        );
        let out = det.run(&ctx("vscode", Some(sysroot), &[])).await.expect("ok");
        let app = &out.partial.apps[0];
        assert_eq!(app.uses_wayland, Some(true));
    }

    #[tokio::test]
    async fn missing_binary_emits_nothing() {
        // No binary staged under sysroot → resolve_binary returns None.
        let det = ElectronAppDetector::with_deps(runner(&[]), proc_with(&[]));
        let empty_root = std::env::var_os("TMPDIR")
            .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from)
            .join(format!("vietime-electron-empty-{}", std::process::id()));
        std::fs::create_dir_all(&empty_root).expect("mkdir");
        let out = det.run(&ctx("vscode", Some(empty_root), &[])).await.expect("ok");
        assert!(out.partial.apps.is_empty());
    }

    #[test]
    fn version_scanner_prefers_electron_over_chrome() {
        let hay = b"Electron/1.2.3\0Chrome/4.5.6\0";
        assert_eq!(scan_electron_version_blocking_from(hay).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn version_scanner_falls_back_to_chrome() {
        let hay = b"Chrome/128.0.6613.119\0";
        assert_eq!(scan_electron_version_blocking_from(hay).as_deref(), Some("128.0.6613.119"));
    }

    #[test]
    fn version_scanner_none_on_empty() {
        assert!(scan_electron_version_blocking_from(b"").is_none());
        assert!(scan_electron_version_blocking_from(b"nothing interesting").is_none());
    }

    fn scan_electron_version_blocking_from(bytes: &[u8]) -> Option<String> {
        find_version_after_tag(bytes, b"Electron/")
            .or_else(|| find_version_after_tag(bytes, b"Chrome/"))
    }
}
