// SPDX-License-Identifier: GPL-3.0-or-later
//
// `app.generic` — the base-layer app detector that runs when the user passes
// `--app <X>`. Resolves the target to a static `AppProfile`, locates the
// binary on disk, asks it for its `--version`, and runs `file(1)` to
// double-check the `kind_hint` against what the executable actually is.
//
// This detector emits *one* `AppFacts` row even when most probes miss, so
// the user always sees *some* output explaining what Doctor looked for. The
// only time we emit nothing is when `ctx.target_app` is `None` (nothing to
// do) or the app id is completely unrecognised (user typo — we emit a note
// instead of an anomaly so the report still renders cleanly).
//
// Deeper probes — Electron version strings, `/proc/*/cmdline` scans for
// Ozone flags — live in DOC-32 (`ElectronAppDetector`) and run alongside
// this detector; the orchestrator's `reconcile_app_facts` collapses the two
// outputs into a single row.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-31).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::{AppFacts, AppKind};

use crate::apps::{parse_version_token, resolve_app, resolve_binary, AppProfile};
use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};
use crate::process::{CommandRunner, TokioCommandRunner};

#[derive(Debug)]
pub struct GenericAppDetector {
    runner: Arc<dyn CommandRunner>,
}

impl Default for GenericAppDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl GenericAppDetector {
    #[must_use]
    pub fn new() -> Self {
        Self { runner: Arc::new(TokioCommandRunner::with_timeout(Duration::from_secs(2))) }
    }

    #[must_use]
    pub fn with_runner(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Detector for GenericAppDetector {
    fn id(&self) -> &'static str {
        "app.generic"
    }

    fn timeout(&self) -> Duration {
        // We fan out to up to three subprocess calls (`which`, the app's
        // `--version`, and `file`), each with its own 2s runner belt.
        Duration::from_secs(4)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let Some(raw) = ctx.target_app.as_deref() else {
            // No `--app` on the CLI — nothing to do. The orchestrator
            // always registers this detector unconditionally when an app
            // flag is present, so we never actually get here in practice,
            // but keep the guard for direct unit tests.
            return Ok(DetectorOutput::default());
        };

        let Some(profile) = resolve_app(raw) else {
            // User typo on `--app`. Emit a note (rendered in verbose mode)
            // but no `AppFacts` and no anomaly — a bad CLI arg isn't a
            // Doctor bug.
            return Ok(DetectorOutput {
                partial: PartialFacts::default(),
                notes: vec![format!("unknown app id: {raw}")],
            });
        };

        let bin = resolve_binary(&self.runner, profile, ctx.sysroot.as_deref()).await;

        // Ask the binary for its version. A missing binary short-circuits
        // to `None` — we still emit an AppFacts below so the user sees
        // "we recognised this app but couldn't find it on disk".
        let version = match bin.as_deref() {
            Some(path) => run_version(&self.runner, path).await,
            None => None,
        };

        // `file(1)` probe — may promote the profile's kind_hint (e.g.
        // Electron → AppImage) or confirm it. Falls through silently on
        // failure.
        let kind = refine_kind(&self.runner, profile, bin.as_deref()).await;

        let mut notes = Vec::new();
        if bin.is_none() {
            notes.push(format!("{}: binary not found on PATH or binary_hints", profile.id));
        }
        if version.is_none() && bin.is_some() {
            notes.push(format!("{}: --version produced no parseable token", profile.id));
        }

        let facts = AppFacts {
            app_id: profile.id.to_owned(),
            binary_path: bin.unwrap_or_default(),
            version,
            kind,
            // Electron-specific probes live in DOC-32.
            electron_version: None,
            uses_wayland: None,
            detector_notes: notes.clone(),
        };

        Ok(DetectorOutput {
            partial: PartialFacts { apps: vec![facts], ..PartialFacts::default() },
            notes,
        })
    }
}

async fn run_version(runner: &Arc<dyn CommandRunner>, path: &Path) -> Option<String> {
    let s = path.to_str()?;
    match runner.run(s, &["--version"]).await {
        Ok(stdout) => parse_version_token(&stdout),
        Err(e) => {
            debug!("{s} --version: {e}");
            None
        }
    }
}

/// Use `file(1)` to refine the profile's `kind_hint`. When `file` isn't
/// available or says something we don't understand we keep the hint.
async fn refine_kind(
    runner: &Arc<dyn CommandRunner>,
    profile: &AppProfile,
    bin: Option<&Path>,
) -> AppKind {
    let Some(bin) = bin else {
        return profile.kind_hint.clone();
    };
    let Some(s) = bin.to_str() else {
        return profile.kind_hint.clone();
    };
    let Ok(stdout) = runner.run("file", &["--brief", s]).await else {
        return profile.kind_hint.clone();
    };
    let desc = stdout.to_ascii_lowercase();
    if desc.contains("appimage") {
        return AppKind::AppImage;
    }
    if desc.contains("python script") || desc.contains("shell script") {
        // Most launch wrappers for IDEs (e.g. `idea.sh`) are shell scripts
        // — we still want to render the app, but the "kind" we report is
        // the launcher form, not the JVM child process.
        return AppKind::Native;
    }
    // ELF / Mach-O / etc. — the hint was already our best guess.
    profile.kind_hint.clone()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::process::tests::FakeCommandRunner;
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

    fn ctx_with_app(app: &str) -> DetectorContext {
        DetectorContext {
            env: std::collections::HashMap::new(),
            sysroot: None,
            target_app: Some(app.to_owned()),
        }
    }

    #[tokio::test]
    async fn target_app_none_emits_nothing() {
        let det = GenericAppDetector::with_runner(runner(&[]));
        let ctx = DetectorContext::default();
        let out = det.run(&ctx).await.expect("runs");
        assert!(out.partial.apps.is_empty());
        assert!(out.notes.is_empty());
    }

    #[tokio::test]
    async fn unknown_app_emits_note_but_no_facts() {
        let det = GenericAppDetector::with_runner(runner(&[]));
        let ctx = ctx_with_app("notepad++");
        let out = det.run(&ctx).await.expect("runs");
        assert!(out.partial.apps.is_empty());
        assert_eq!(out.notes.len(), 1);
        assert!(out.notes[0].contains("unknown app id"));
    }

    #[tokio::test]
    async fn finds_vscode_on_path_and_parses_version() {
        // `which vscode` resolves to `/opt/bin/code`; `file --brief` returns
        // ELF (unknown to us → keep hint); `code --version` prints the
        // real VS Code format.
        let det = GenericAppDetector::with_runner(runner(&[
            ("which", &["vscode"], "/opt/bin/code\n"),
            ("file", &["--brief", "/opt/bin/code"], "ELF 64-bit LSB executable\n"),
            ("/opt/bin/code", &["--version"], "1.87.2\nabcdef\nx64\n"),
        ]));
        let out = det.run(&ctx_with_app("vscode")).await.expect("runs");
        assert_eq!(out.partial.apps.len(), 1);
        let app = &out.partial.apps[0];
        assert_eq!(app.app_id, "vscode");
        assert_eq!(app.binary_path, PathBuf::from("/opt/bin/code"));
        assert_eq!(app.version.as_deref(), Some("1.87.2"));
        assert!(matches!(app.kind, AppKind::Electron));
    }

    #[tokio::test]
    async fn missing_binary_emits_row_with_empty_path_and_note() {
        // No `which` response → binary not found. Kind stays at the hint.
        let det = GenericAppDetector::with_runner(runner(&[]));
        let out = det.run(&ctx_with_app("vscode")).await.expect("runs");
        assert_eq!(out.partial.apps.len(), 1);
        let app = &out.partial.apps[0];
        assert!(app.binary_path.as_os_str().is_empty());
        assert!(app.version.is_none());
        assert!(matches!(app.kind, AppKind::Electron));
        assert!(app.detector_notes.iter().any(|n| n.contains("binary not found")));
    }

    #[tokio::test]
    async fn file_probe_promotes_shell_script_launcher_to_native() {
        let det = GenericAppDetector::with_runner(runner(&[
            ("which", &["intellij"], "/opt/idea/bin/idea.sh\n"),
            ("file", &["--brief", "/opt/idea/bin/idea.sh"], "POSIX shell script, ASCII text\n"),
            ("/opt/idea/bin/idea.sh", &["--version"], "IntelliJ IDEA 2024.1.3\n"),
        ]));
        let out = det.run(&ctx_with_app("intellij")).await.expect("runs");
        let app = &out.partial.apps[0];
        // JVM → Native via script-launcher heuristic.
        assert!(matches!(app.kind, AppKind::Native));
        assert_eq!(app.version.as_deref(), Some("2024.1.3"));
    }

    #[tokio::test]
    async fn appimage_is_recognised_via_file_output() {
        let det = GenericAppDetector::with_runner(runner(&[
            ("which", &["obsidian"], "/opt/Obsidian.AppImage\n"),
            (
                "file",
                &["--brief", "/opt/Obsidian.AppImage"],
                "ELF 64-bit LSB executable, (AppImage)\n",
            ),
            ("/opt/Obsidian.AppImage", &["--version"], "Obsidian 1.5.11\n"),
        ]));
        let out = det.run(&ctx_with_app("obsidian")).await.expect("runs");
        let app = &out.partial.apps[0];
        assert!(matches!(app.kind, AppKind::AppImage));
        assert_eq!(app.version.as_deref(), Some("1.5.11"));
    }
}
