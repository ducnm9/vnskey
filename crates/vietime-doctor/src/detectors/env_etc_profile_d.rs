// SPDX-License-Identifier: GPL-3.0-or-later
//
// `env.etc_profile_d` — reads `*.sh` files from `{sysroot}/etc/profile.d/`
// looking for IM env var assignments. Files are processed in alphabetical
// order; earlier files win on conflict, matching shell behaviour where
// `/etc/profile` sources them in order.
//
// These scripts are real shell code (unlike `/etc/environment`), but the
// overwhelming majority of IM-related snippets are bare `export KEY=value`
// or `KEY=value` — which `vietime_core::parse_etc_environment` handles
// fine. Non-assignment shell constructs (`if`, `case`, `fi`) are silently
// dropped; adding a full shell-words parser is out of scope for Week 2.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-13a).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::{parse_etc_environment, EnvFacts, EnvSource, IM_ENV_KEYS};

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};

/// Scans `{sysroot}/etc/profile.d/*.sh`.
#[derive(Debug, Default)]
pub struct EtcProfileDDetector;

impl EtcProfileDDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Detector for EtcProfileDDetector {
    fn id(&self) -> &'static str {
        "env.etc_profile_d"
    }

    fn timeout(&self) -> Duration {
        // Dozen small shell snippets tops.
        Duration::from_secs(2)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let base: &Path = ctx.sysroot.as_deref().unwrap_or_else(|| Path::new("/"));
        let dir = base.join("etc").join("profile.d");

        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("no /etc/profile.d dir at {}", dir.display());
                return Ok(DetectorOutput::default());
            }
            Err(e) => {
                return Err(crate::detector::DetectorError::Other(format!(
                    "failed to read dir {}: {e}",
                    dir.display()
                )));
            }
        };

        let mut scripts: Vec<PathBuf> = Vec::new();
        loop {
            match entries.next_entry().await {
                Ok(Some(e)) => {
                    let p = e.path();
                    if p.extension().and_then(|s| s.to_str()) == Some("sh") {
                        scripts.push(p);
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    debug!("stopped reading {}: {err}", dir.display());
                    break;
                }
            }
        }
        scripts.sort();

        let mut merged: HashMap<String, String> = HashMap::new();
        let mut notes = Vec::new();
        for path in scripts {
            match tokio::fs::read_to_string(&path).await {
                Ok(s) => {
                    let parsed = parse_etc_environment(&s);
                    for key in IM_ENV_KEYS {
                        if merged.contains_key(key) {
                            continue;
                        }
                        if let Some(v) = parsed.get(key) {
                            merged.insert(key.to_owned(), v.clone());
                        }
                    }
                    notes.push(format!("parsed {}", path.display()));
                }
                Err(err) => {
                    debug!("skipping {}: {err}", path.display());
                }
            }
        }

        if merged.is_empty() && notes.is_empty() {
            return Ok(DetectorOutput::default());
        }

        let facts = EnvFacts::from_env_with_source(&merged, EnvSource::EtcProfileD);
        Ok(DetectorOutput {
            partial: PartialFacts { env: Some(facts), ..PartialFacts::default() },
            notes,
        })
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

    async fn run_with(root: PathBuf) -> DetectorOutput {
        let det = EtcProfileDDetector::new();
        let ctx = DetectorContext { env: HashMap::default(), sysroot: Some(root) };
        det.run(&ctx).await.expect("detector ok")
    }

    #[tokio::test]
    async fn lexicographically_earliest_script_wins() {
        let tmp = tempfile_dir("env-profile-d-order");
        seed(&tmp, "etc/profile.d/10-ibus.sh", "export GTK_IM_MODULE=ibus\n");
        seed(&tmp, "etc/profile.d/20-fcitx.sh", "export GTK_IM_MODULE=fcitx\n");
        let out = run_with(tmp).await;
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::EtcProfileD));
    }

    #[tokio::test]
    async fn ignores_non_sh_files() {
        let tmp = tempfile_dir("env-profile-d-nonsh");
        seed(&tmp, "etc/profile.d/ibus.sh", "export QT_IM_MODULE=ibus\n");
        // README and *.bak files are common noise in /etc/profile.d/.
        seed(&tmp, "etc/profile.d/README", "export GTK_IM_MODULE=bogus\n");
        seed(&tmp, "etc/profile.d/im-fcitx.sh.bak", "export GTK_IM_MODULE=fcitx\n");
        let out = run_with(tmp).await;
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.qt_im_module.as_deref(), Some("ibus"));
        assert!(
            facts.gtk_im_module.is_none(),
            "README and .bak files must not be parsed: {facts:?}"
        );
    }

    #[tokio::test]
    async fn missing_dir_is_not_a_failure() {
        let tmp = tempfile_dir("env-profile-d-missing");
        let out = run_with(tmp).await;
        assert!(out.partial.env.is_none());
    }

    #[tokio::test]
    async fn id_is_env_etc_profile_d() {
        let d = EtcProfileDDetector::new();
        assert_eq!(d.id(), "env.etc_profile_d");
    }
}
