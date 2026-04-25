// SPDX-License-Identifier: GPL-3.0-or-later
//
// `im.fcitx5.config` — reads Fcitx5's on-disk config to learn which
// input methods the user has selected and which addons are enabled.
//
// We parse two filesystem locations under `$HOME/.config/fcitx5`:
//
// ## `profile`
//
// INI-shaped. The `[Groups/0]` section tells us the default IM:
//
//     [Groups/0]
//     Name=Default
//     Default Layout=us
//     DefaultIM=bamboo
//
// Each method entry sits in a numbered subsection:
//
//     [Groups/0/Items/0]
//     Name=keyboard-us
//     Layout=
//
//     [Groups/0/Items/1]
//     Name=bamboo
//     Layout=
//
// We collect every `Name=` line in `[Groups/0/Items/*]` into
// `input_methods_configured`, with `DefaultIM` promoted to the front
// (it's the primary IM).
//
// ## `conf/*.conf`
//
// Each `.conf` file is an addon's config. Presence means the addon is
// *installed*; the top-level `Enabled=True/False` pair decides whether
// it's active. Missing `Enabled=` defaults to `True` in Fcitx5's own
// logic. File stem (minus `.conf`) is the addon id.
//
// ## Parser
//
// We do NOT add an `ini` crate for this — the shape is simple enough
// that a ~25 line section-aware loop handles it with no dep. We look
// only for the keys we care about; unknown keys in unknown sections are
// silently dropped.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-23).

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::Fcitx5Facts;

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};

#[derive(Debug, Default)]
pub struct Fcitx5ConfigDetector;

impl Fcitx5ConfigDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Detector for Fcitx5ConfigDetector {
    fn id(&self) -> &'static str {
        "im.fcitx5.config"
    }

    fn timeout(&self) -> Duration {
        // Pure filesystem — a second is generous.
        Duration::from_secs(1)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let Some(home) = ctx.env.get("HOME") else {
            debug!("HOME not set; skipping fcitx5 config detector");
            return Ok(DetectorOutput::default());
        };
        let home_root: PathBuf = if let Some(sysroot) = ctx.sysroot.as_deref() {
            let stripped = Path::new(home).strip_prefix("/").unwrap_or(Path::new(home));
            sysroot.join(stripped)
        } else {
            PathBuf::from(home)
        };
        let cfg_dir = home_root.join(".config").join("fcitx5");

        let input_methods_configured =
            match tokio::fs::read_to_string(cfg_dir.join("profile")).await {
                Ok(body) => parse_profile_imes(&body),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(e) => {
                    debug!("reading profile: {e}");
                    Vec::new()
                }
            };

        let addons_enabled = read_addons(&cfg_dir.join("conf")).await;

        if input_methods_configured.is_empty() && addons_enabled.is_empty() {
            // No on-disk fcitx5 config at all. Stay silent — the merge
            // layer leaves DOC-22's daemon facts intact.
            return Ok(DetectorOutput::default());
        }

        let facts = Fcitx5Facts {
            version: None,
            daemon_running: false,
            daemon_pid: None,
            config_dir: Some(cfg_dir.clone()),
            addons_enabled,
            input_methods_configured,
        };

        Ok(DetectorOutput {
            partial: PartialFacts { fcitx5: Some(facts), ..PartialFacts::default() },
            notes: vec![format!("parsed {}", cfg_dir.display())],
        })
    }
}

/// Parse the `profile` body into an ordered IM list with `DefaultIM` first.
pub(crate) fn parse_profile_imes(body: &str) -> Vec<String> {
    let mut default_im: Option<String> = None;
    // Preserve insertion order and dedup.
    let mut names: Vec<String> = Vec::new();
    let mut section: String = String::new();

    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            rest.clone_into(&mut section);
            continue;
        }
        // KEY=VALUE
        let Some((k, v)) = line.split_once('=') else { continue };
        let key = k.trim();
        let val = v.trim();
        if section == "Groups/0" && key == "DefaultIM" && !val.is_empty() {
            default_im = Some(val.to_owned());
            continue;
        }
        // `[Groups/0/Items/N]` — any N, we don't care about the index.
        if section.starts_with("Groups/0/Items/") && key == "Name" && !val.is_empty() {
            let v = val.to_owned();
            if !names.contains(&v) {
                names.push(v);
            }
        }
    }

    // Promote DefaultIM to the front. If it's already in the list, move
    // it; if it's not, prepend it.
    if let Some(d) = default_im {
        names.retain(|n| n != &d);
        names.insert(0, d);
    }
    names
}

/// Read `<cfg_dir>/conf/*.conf` and return the ids of addons whose
/// `Enabled=` is missing or `True`.
async fn read_addons(conf_dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(conf_dir).await else {
        return out;
    };
    let mut files: Vec<PathBuf> = Vec::new();
    loop {
        match entries.next_entry().await {
            Ok(Some(e)) => {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("conf") {
                    files.push(p);
                }
            }
            Ok(None) => break,
            Err(err) => {
                debug!("stopped reading {}: {err}", conf_dir.display());
                break;
            }
        }
    }
    files.sort();
    for path in files {
        let body = match tokio::fs::read_to_string(&path).await {
            Ok(b) => b,
            Err(err) => {
                debug!("skipping {}: {err}", path.display());
                continue;
            }
        };
        if !addon_is_enabled(&body) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        out.push(stem.to_owned());
    }
    out
}

/// Parse an addon `.conf` body and decide if it's enabled.
///
/// Rule: `Enabled=False` → disabled. Anything else (including absence) →
/// enabled. This matches Fcitx5's in-tree behaviour.
pub(crate) fn addon_is_enabled(body: &str) -> bool {
    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == "Enabled" {
                return !v.trim().eq_ignore_ascii_case("false");
            }
        }
    }
    true
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn tempfile_dir(label: &str) -> PathBuf {
        let base = std::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
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

    fn seed(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("mkdir parent");
        }
        std::fs::write(p, body).expect("write file");
    }

    fn make_ctx(sysroot: PathBuf, home: &str) -> DetectorContext {
        let mut env = HashMap::new();
        env.insert("HOME".to_owned(), home.to_owned());
        DetectorContext { env, sysroot: Some(sysroot) }
    }

    #[test]
    fn parse_profile_promotes_default_im_to_front() {
        let body = "\
[Groups/0]
Name=Default
Default Layout=us
DefaultIM=bamboo

[Groups/0/Items/0]
Name=keyboard-us
Layout=

[Groups/0/Items/1]
Name=bamboo
Layout=
";
        let ims = parse_profile_imes(body);
        assert_eq!(ims, vec!["bamboo", "keyboard-us"]);
    }

    #[test]
    fn parse_profile_ignores_other_groups() {
        let body = "\
[Groups/1/Items/0]
Name=should-not-show
";
        let ims = parse_profile_imes(body);
        assert!(ims.is_empty());
    }

    #[test]
    fn addon_without_enabled_key_is_enabled_by_default() {
        let body = "[Addon]\nType=Frontend\n";
        assert!(addon_is_enabled(body));
    }

    #[test]
    fn addon_with_enabled_false_is_disabled() {
        let body = "[Addon]\nEnabled=False\n";
        assert!(!addon_is_enabled(body));
    }

    #[tokio::test]
    async fn detector_picks_up_profile_and_enabled_addons() {
        let tmp = tempfile_dir("fcitx5-config-basic");
        seed(
            &tmp,
            "home/alice/.config/fcitx5/profile",
            "[Groups/0]\nDefaultIM=bamboo\n\n[Groups/0/Items/0]\nName=keyboard-us\n\n[Groups/0/Items/1]\nName=bamboo\n",
        );
        seed(&tmp, "home/alice/.config/fcitx5/conf/unicode.conf", "[Addon]\nEnabled=True\n");
        seed(&tmp, "home/alice/.config/fcitx5/conf/dummy.conf", "[Addon]\nEnabled=False\n");
        let ctx = make_ctx(tmp, "/home/alice");
        let out = Fcitx5ConfigDetector::new().run(&ctx).await.expect("ok");
        let f = out.partial.fcitx5.expect("fcitx5 set");
        assert_eq!(f.input_methods_configured, vec!["bamboo", "keyboard-us"]);
        assert_eq!(f.addons_enabled, vec!["unicode"]);
        // Daemon facts left untouched by this detector.
        assert!(!f.daemon_running);
        assert!(f.version.is_none());
    }

    #[tokio::test]
    async fn detector_silent_when_config_missing() {
        let tmp = tempfile_dir("fcitx5-config-missing");
        // Create the home root but no fcitx5 config.
        std::fs::create_dir_all(tmp.join("home/bob")).expect("mkdir home");
        let ctx = make_ctx(tmp, "/home/bob");
        let out = Fcitx5ConfigDetector::new().run(&ctx).await.expect("ok");
        assert!(out.partial.fcitx5.is_none());
    }

    #[tokio::test]
    async fn missing_home_yields_no_partial() {
        let ctx = DetectorContext::default();
        let out = Fcitx5ConfigDetector::new().run(&ctx).await.expect("ok");
        assert!(out.partial.fcitx5.is_none());
    }

    #[tokio::test]
    async fn id_is_im_fcitx5_config() {
        let d = Fcitx5ConfigDetector::new();
        assert_eq!(d.id(), "im.fcitx5.config");
    }
}
