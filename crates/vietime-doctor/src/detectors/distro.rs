// SPDX-License-Identifier: GPL-3.0-or-later
//
// `sys.distro` — identifies the Linux distribution by parsing
// `/etc/os-release`. Pure wrapper around `vietime_core::detect_from_os_release`.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::detect_from_os_release;

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};

/// Reads `/etc/os-release` (or `{sysroot}/etc/os-release` when a sysroot
/// is configured for tests) and emits `SystemFacts.distro`.
#[derive(Debug, Default)]
pub struct DistroDetector;

impl DistroDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Detector for DistroDetector {
    fn id(&self) -> &'static str {
        "sys.distro"
    }

    fn timeout(&self) -> Duration {
        // Pure file read; 1s is generous.
        Duration::from_secs(1)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let base: &Path = ctx.sysroot.as_deref().unwrap_or_else(|| Path::new("/"));
        let candidates = ["etc/os-release", "usr/lib/os-release"];

        let mut contents: Option<String> = None;
        let mut used_path: Option<std::path::PathBuf> = None;
        for rel in candidates {
            let p = base.join(rel);
            match tokio::fs::read_to_string(&p).await {
                Ok(s) => {
                    contents = Some(s);
                    used_path = Some(p);
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(crate::detector::DetectorError::Other(format!(
                        "failed to read {}: {e}",
                        p.display()
                    )));
                }
            }
        }

        let Some(raw) = contents else {
            // os-release is absent on exotic systems (plain chroot, container
            // without it). Don't fail — just emit no distro and log.
            debug!("no os-release file found under {}", base.display());
            return Ok(DetectorOutput::default());
        };

        let distro = detect_from_os_release(&raw);
        let mut notes = Vec::new();
        if let Some(p) = used_path {
            notes.push(format!("parsed {}", p.display()));
        }

        Ok(DetectorOutput {
            partial: PartialFacts { distro: Some(distro), ..PartialFacts::default() },
            notes,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::map_unwrap_or)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    async fn run_with_sysroot(root: &std::path::Path) -> DetectorOutput {
        let det = DistroDetector::new();
        let ctx = DetectorContext {
            env: HashMap::default(),
            sysroot: Some(root.to_path_buf()),
            target_app: None,
        };
        det.run(&ctx).await.expect("detector should not fail")
    }

    fn seed_os_release(tmp: &std::path::Path, body: &str) {
        let etc = tmp.join("etc");
        std::fs::create_dir_all(&etc).expect("mkdir etc");
        std::fs::write(etc.join("os-release"), body).expect("write os-release");
    }

    #[tokio::test]
    async fn parses_ubuntu_24_04_from_sysroot() {
        let tmp = tempfile_dir();
        seed_os_release(
            &tmp,
            "NAME=\"Ubuntu\"\nID=ubuntu\nVERSION_ID=\"24.04\"\nPRETTY_NAME=\"Ubuntu 24.04 LTS\"\n",
        );
        let out = run_with_sysroot(&tmp).await;
        let distro = out.partial.distro.expect("distro parsed");
        assert_eq!(distro.id, "ubuntu");
        assert_eq!(distro.version_id.as_deref(), Some("24.04"));
    }

    #[tokio::test]
    async fn missing_os_release_is_not_a_failure() {
        let tmp = tempfile_dir();
        let out = run_with_sysroot(&tmp).await;
        assert!(out.partial.distro.is_none());
    }

    #[tokio::test]
    async fn falls_back_to_usr_lib_os_release() {
        let tmp = tempfile_dir();
        let usr_lib = tmp.join("usr/lib");
        std::fs::create_dir_all(&usr_lib).expect("mkdir usr/lib");
        std::fs::write(
            usr_lib.join("os-release"),
            "ID=fedora\nVERSION_ID=40\nPRETTY_NAME=\"Fedora 40\"\n",
        )
        .expect("write usr/lib/os-release");
        let out = run_with_sysroot(&tmp).await;
        let distro = out.partial.distro.expect("distro parsed");
        assert_eq!(distro.id, "fedora");
    }

    /// Makes a unique temp dir under `$TMPDIR` without external crates;
    /// the test cleans up best-effort on drop via a guard.
    fn tempfile_dir() -> PathBuf {
        let base =
            std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        let name = format!(
            "vietime-doctor-distro-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        );
        let dir = base.join(name);
        std::fs::create_dir_all(&dir).expect("mkdir tmp");
        dir
    }
}
