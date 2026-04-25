// SPDX-License-Identifier: GPL-3.0-or-later
//
// `env.home_profile` — scans a user's login shell rc files for IM env
// variable assignments.
//
// The target files are:
//   * `$HOME/.profile`
//   * `$HOME/.bashrc`
//   * `$HOME/.zshrc`
//   * `$HOME/.config/environment.d/*.conf`
//
// All four are parsed with [`vietime_core::parse_etc_environment`]. The
// parser accepts `KEY=value`, `export KEY=value`, matched single/double
// quotes, and silently drops shell-script lines it can't turn into an
// assignment — which is exactly what we want for `.bashrc`-style files
// that mix assignments with `alias` / function definitions.
//
// Within this detector earlier files in the list above win on conflict:
// most users put their "authoritative" settings in `.profile`, and
// `.bashrc` often contains framework-agnostic shell glue that we'd rather
// not have clobber the `.profile` value. This in-detector ordering is
// cosmetic — the orchestrator's `merge_by_priority` step will still let
// `ProcessEnvDetector` override us.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-12).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::{parse_etc_environment, EnvFacts, EnvSource, IM_ENV_KEYS};

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};

/// Files scanned in this order — earlier wins on conflict within the detector.
const HOME_FILES: [&str; 3] = [".profile", ".bashrc", ".zshrc"];

/// Reads `~/.profile`, `~/.bashrc`, `~/.zshrc`, and `~/.config/environment.d/*.conf`.
#[derive(Debug, Default)]
pub struct HomeProfileDetector;

impl HomeProfileDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Detector for HomeProfileDetector {
    fn id(&self) -> &'static str {
        "env.home_profile"
    }

    fn timeout(&self) -> Duration {
        // Handful of small files; 2s is generous.
        Duration::from_secs(2)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let Some(home) = ctx.env.get("HOME") else {
            debug!("HOME is not set — skipping home-profile detector");
            return Ok(DetectorOutput::default());
        };

        // When tests configure a sysroot, strip the leading `/` from HOME
        // and join relative to sysroot; that keeps the seed pattern from
        // `distro.rs` tests working unchanged.
        let home_root: PathBuf = if let Some(sysroot) = ctx.sysroot.as_deref() {
            let stripped = Path::new(home).strip_prefix("/").unwrap_or(Path::new(home));
            sysroot.join(stripped)
        } else {
            PathBuf::from(home)
        };

        // First-wins accumulator over our known-set of IM keys.
        let mut merged: HashMap<String, String> = HashMap::new();
        let mut notes = Vec::new();

        for rel in HOME_FILES {
            let path = home_root.join(rel);
            match tokio::fs::read_to_string(&path).await {
                Ok(s) => {
                    merge_first_wins(&mut merged, &parse_etc_environment(&s));
                    notes.push(format!("parsed {}", path.display()));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    // Permissions or I/O errors on a single file shouldn't
                    // crash the whole detector — log and carry on.
                    debug!("skipping {}: {e}", path.display());
                }
            }
        }

        // `~/.config/environment.d/*.conf` — alphabetical order, earlier wins.
        let env_d = home_root.join(".config").join("environment.d");
        if let Ok(mut entries) = tokio::fs::read_dir(&env_d).await {
            let mut confs: Vec<PathBuf> = Vec::new();
            loop {
                match entries.next_entry().await {
                    Ok(Some(e)) => {
                        let p = e.path();
                        if p.extension().and_then(|s| s.to_str()) == Some("conf") {
                            confs.push(p);
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        debug!("stopped reading {}: {err}", env_d.display());
                        break;
                    }
                }
            }
            confs.sort();
            for path in confs {
                match tokio::fs::read_to_string(&path).await {
                    Ok(s) => {
                        merge_first_wins(&mut merged, &parse_etc_environment(&s));
                        notes.push(format!("parsed {}", path.display()));
                    }
                    Err(err) => {
                        debug!("skipping {}: {err}", path.display());
                    }
                }
            }
        }

        if merged.is_empty() && notes.is_empty() {
            // Nothing to report. Keep the partial empty so the orchestrator
            // doesn't think we found a source it can then clobber.
            return Ok(DetectorOutput::default());
        }

        let facts = EnvFacts::from_env_with_source(&merged, EnvSource::HomeProfile);
        Ok(DetectorOutput {
            partial: PartialFacts { env: Some(facts), ..PartialFacts::default() },
            notes,
        })
    }
}

/// Merge `next` into `acc`, but only for IM keys we care about, and only
/// when the key isn't already set — "first wins" semantics inside the
/// detector so `.profile` beats `.bashrc` on the same key.
fn merge_first_wins(acc: &mut HashMap<String, String>, next: &HashMap<String, String>) {
    for key in IM_ENV_KEYS {
        if acc.contains_key(key) {
            continue;
        }
        if let Some(v) = next.get(key) {
            acc.insert(key.to_owned(), v.clone());
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::map_unwrap_or)]
mod tests {
    use super::*;

    fn tempfile_dir(label: &str) -> PathBuf {
        let base =
            std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        let name = format!(
            "vietime-doctor-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        );
        let dir = base.join(name);
        std::fs::create_dir_all(&dir).expect("mkdir tmp");
        dir
    }

    fn seed(root: &std::path::Path, rel: &str, body: &str) {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("mkdir parent");
        }
        std::fs::write(p, body).expect("write file");
    }

    fn make_ctx(sysroot: PathBuf, home: &str) -> DetectorContext {
        let mut env = HashMap::new();
        env.insert("HOME".to_owned(), home.to_owned());
        DetectorContext { env, sysroot: Some(sysroot), target_app: None }
    }

    #[tokio::test]
    async fn profile_wins_over_bashrc_on_same_key() {
        let tmp = tempfile_dir("env-home-profile-wins");
        seed(&tmp, "home/alice/.profile", "GTK_IM_MODULE=ibus\n");
        seed(&tmp, "home/alice/.bashrc", "GTK_IM_MODULE=fcitx\n");
        let ctx = make_ctx(tmp, "/home/alice");
        let out = HomeProfileDetector::new().run(&ctx).await.expect("detector ok");
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::HomeProfile));
    }

    #[tokio::test]
    async fn picks_up_environment_d_conf_snippets() {
        let tmp = tempfile_dir("env-home-environment-d");
        seed(
            &tmp,
            "home/bob/.config/environment.d/10-fcitx.conf",
            "GTK_IM_MODULE=fcitx\nQT_IM_MODULE=fcitx\nXMODIFIERS=@im=fcitx\n",
        );
        let ctx = make_ctx(tmp, "/home/bob");
        let out = HomeProfileDetector::new().run(&ctx).await.expect("detector ok");
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("fcitx"));
        assert_eq!(facts.qt_im_module.as_deref(), Some("fcitx"));
        assert_eq!(facts.xmodifiers.as_deref(), Some("@im=fcitx"));
    }

    #[tokio::test]
    async fn lexicographically_earlier_conf_wins() {
        let tmp = tempfile_dir("env-home-environment-d-sort");
        seed(&tmp, "home/c/.config/environment.d/10-ibus.conf", "GTK_IM_MODULE=ibus\n");
        seed(&tmp, "home/c/.config/environment.d/20-fcitx.conf", "GTK_IM_MODULE=fcitx\n");
        let ctx = make_ctx(tmp, "/home/c");
        let out = HomeProfileDetector::new().run(&ctx).await.expect("detector ok");
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
    }

    #[tokio::test]
    async fn handles_export_style_assignments() {
        let tmp = tempfile_dir("env-home-export");
        seed(&tmp, "home/d/.profile", "export GTK_IM_MODULE=ibus\nexport QT_IM_MODULE=ibus\n");
        let ctx = make_ctx(tmp, "/home/d");
        let out = HomeProfileDetector::new().run(&ctx).await.expect("detector ok");
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.qt_im_module.as_deref(), Some("ibus"));
    }

    #[tokio::test]
    async fn missing_home_means_no_partial() {
        let ctx = DetectorContext::default();
        let out = HomeProfileDetector::new().run(&ctx).await.expect("detector ok");
        assert!(out.partial.env.is_none());
    }

    #[tokio::test]
    async fn no_profile_files_means_no_partial() {
        // HOME is set but no rc files exist under it.
        let tmp = tempfile_dir("env-home-empty");
        std::fs::create_dir_all(tmp.join("home/e")).expect("mkdir home");
        let ctx = make_ctx(tmp, "/home/e");
        let out = HomeProfileDetector::new().run(&ctx).await.expect("detector ok");
        assert!(out.partial.env.is_none());
    }

    #[tokio::test]
    async fn id_is_env_home_profile() {
        let d = HomeProfileDetector::new();
        assert_eq!(d.id(), "env.home_profile");
    }
}
