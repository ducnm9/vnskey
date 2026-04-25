// SPDX-License-Identifier: GPL-3.0-or-later
//
// Subprocess-using helpers shared by DOC-31 (`GenericAppDetector`) and
// DOC-32 (`ElectronAppDetector`).
//
// * `resolve_binary` — find the app's executable on disk, preferring the
//   profile's `binary_hints` over `$PATH` because `which` will pick up
//   wrapper scripts (`/usr/bin/code` is the VS Code shell wrapper, not the
//   actual Electron binary).
// * `parse_version_token` — pull the first dotted-number token out of a
//   `--version` line. Hand-rolled so we don't drag in the `regex` crate
//   for 15 lines of matching.
//
// Both helpers are async and take an `Arc<dyn CommandRunner>` so tests can
// inject a `FakeCommandRunner` without touching the real filesystem.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::debug;

use crate::apps::registry::AppProfile;
use crate::process::CommandRunner;

/// Locate the binary for `profile` in order of preference:
///
///  1. Each `binary_hint` (prefixed with `sysroot` if set) that exists on
///     disk.
///  2. `which <profile.id>` on `$PATH`.
///  3. `which <alias>` for each alias, in order.
///
/// Returns the first path that resolves. `None` means we looked
/// everywhere we know about and found nothing — the caller should still
/// emit an `AppFacts` with an empty `binary_path` so the user sees that
/// the app was recognised but not found.
pub async fn resolve_binary(
    runner: &Arc<dyn CommandRunner>,
    profile: &AppProfile,
    sysroot: Option<&Path>,
) -> Option<PathBuf> {
    // 1. Try hardcoded hints first — they bypass $PATH and are less
    //    likely to return a wrapper script.
    for hint in profile.binary_hints {
        let candidate = if let Some(root) = sysroot {
            let stripped = hint.strip_prefix('/').unwrap_or(hint);
            root.join(stripped)
        } else {
            PathBuf::from(hint)
        };
        if tokio::fs::metadata(&candidate).await.is_ok() {
            return Some(candidate);
        }
    }

    // 2. `which <id>`.
    if let Some(p) = which_lookup(runner, profile.id).await {
        return Some(p);
    }

    // 3. `which <alias>` for each alias (case preserved — `which` wants it).
    for alias in profile.aliases {
        if let Some(p) = which_lookup(runner, alias).await {
            return Some(p);
        }
    }

    None
}

async fn which_lookup(runner: &Arc<dyn CommandRunner>, name: &str) -> Option<PathBuf> {
    match runner.run("which", &[name]).await {
        Ok(stdout) => {
            let first = stdout.lines().next().map_or("", str::trim);
            if first.is_empty() {
                None
            } else {
                Some(PathBuf::from(first))
            }
        }
        Err(e) => {
            debug!("which {name}: {e}");
            None
        }
    }
}

/// Extract the first `MAJOR.MINOR(.PATCH)?` token from `stdout`.
///
/// Matches the shape VS Code (`1.87.2\nabcdef\nx64\n`), Chrome
/// (`Google Chrome 128.0.6613.119`), Firefox (`Mozilla Firefox 126.0`),
/// and similar `--version` outputs. Returns `None` if nothing that looks
/// like a version number is present. Trailing suffixes like `-beta` are
/// included when glued to the numeric token (no whitespace between).
#[must_use]
pub fn parse_version_token(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        for tok in line.split_whitespace() {
            if looks_like_version(tok) {
                return Some(tok.to_owned());
            }
        }
    }
    None
}

fn looks_like_version(tok: &str) -> bool {
    // Must start with a digit and contain at least one `.` — rules out
    // plain integers (build numbers) and pure strings.
    let mut chars = tok.chars();
    let Some(first) = chars.next() else { return false };
    if !first.is_ascii_digit() {
        return false;
    }
    let mut saw_dot = false;
    let mut only_digits_and_allowed = true;
    for c in chars {
        if c == '.' {
            saw_dot = true;
        } else if !(c.is_ascii_digit() || c == '-' || c == '_' || c.is_ascii_alphabetic()) {
            only_digits_and_allowed = false;
            break;
        }
    }
    saw_dot && only_digits_and_allowed
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::apps::registry::resolve_app;
    use crate::process::tests::FakeCommandRunner;

    fn fake(pairs: &[(&str, &[&str], &str)]) -> Arc<dyn CommandRunner> {
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

    fn tmp_with_binary(rel: &str) -> PathBuf {
        let base = std::env::var_os("TMPDIR")
            .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from)
            .join(format!(
                "vietime-apps-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos())
            ));
        let target = base.join(rel.strip_prefix('/').unwrap_or(rel));
        std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        std::fs::write(&target, b"#!/bin/sh\n").expect("write");
        base
    }

    #[tokio::test]
    async fn resolve_binary_hits_binary_hint_first() {
        let profile = resolve_app("vscode").expect("vscode");
        // Stage `/usr/bin/code` under a temporary sysroot.
        let sysroot = tmp_with_binary("/usr/bin/code");
        let runner = fake(&[]);
        let got = resolve_binary(&runner, profile, Some(&sysroot)).await.expect("found");
        assert!(got.ends_with("usr/bin/code"));
    }

    #[tokio::test]
    async fn resolve_binary_falls_back_to_which_on_id() {
        let profile = resolve_app("vscode").expect("vscode");
        // No sysroot-staged binary — we rely on `which` returning a path.
        let runner = fake(&[("which", &["vscode"], "/opt/vscode/bin/code\n")]);
        let got = resolve_binary(&runner, profile, None).await.expect("found");
        assert_eq!(got, PathBuf::from("/opt/vscode/bin/code"));
    }

    #[tokio::test]
    async fn resolve_binary_tries_alias_when_id_misses() {
        let profile = resolve_app("vscode").expect("vscode");
        // `which vscode` prints nothing → fall back to alias `code`.
        let runner =
            fake(&[("which", &["vscode"], ""), ("which", &["code"], "/usr/local/bin/code\n")]);
        let got = resolve_binary(&runner, profile, None).await.expect("found");
        assert_eq!(got, PathBuf::from("/usr/local/bin/code"));
    }

    #[tokio::test]
    async fn resolve_binary_returns_none_when_nothing_found() {
        let profile = resolve_app("vscode").expect("vscode");
        // Every `which` returns empty; no sysroot hints staged.
        let runner = fake(&[]);
        assert!(resolve_binary(&runner, profile, None).await.is_none());
    }

    #[test]
    fn parse_version_picks_first_dotted_number() {
        assert_eq!(parse_version_token("1.87.2\nabcdef\nx64\n").as_deref(), Some("1.87.2"));
        assert_eq!(
            parse_version_token("Google Chrome 128.0.6613.119\n").as_deref(),
            Some("128.0.6613.119")
        );
        assert_eq!(parse_version_token("Mozilla Firefox 126.0\n").as_deref(), Some("126.0"));
    }

    #[test]
    fn parse_version_returns_none_on_empty_or_alpha_output() {
        assert!(parse_version_token("").is_none());
        assert!(parse_version_token("unknown version\nno numbers here\n").is_none());
        // Integer-only tokens are not versions.
        assert!(parse_version_token("build 12345\n").is_none());
    }
}
