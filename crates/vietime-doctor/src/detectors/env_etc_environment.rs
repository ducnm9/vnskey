// SPDX-License-Identifier: GPL-3.0-or-later
//
// `env.etc_environment` — reads `/etc/environment` (or `{sysroot}/etc/environment`
// when a sysroot is configured for tests), parses it with
// `vietime_core::parse_etc_environment`, and tags every field with
// [`EnvSource::EtcEnvironment`].
//
// Missing file is NOT a failure. Cluster distros, single-user chroots, and
// minimal containers frequently have no `/etc/environment` at all; it's the
// distro installer's job to drop one in. Treating it like `DistroDetector`
// treats a missing `os-release` means the rest of the report still renders.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-11).

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::{parse_etc_environment, EnvFacts, EnvSource};

use crate::detector::{
    Detector, DetectorContext, DetectorError, DetectorOutput, DetectorResult, PartialFacts,
};

/// Reads `{sysroot}/etc/environment`.
#[derive(Debug, Default)]
pub struct EtcEnvironmentDetector;

impl EtcEnvironmentDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Detector for EtcEnvironmentDetector {
    fn id(&self) -> &'static str {
        "env.etc_environment"
    }

    fn timeout(&self) -> Duration {
        // Small file read; 1s is generous.
        Duration::from_secs(1)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let base: &Path = ctx.sysroot.as_deref().unwrap_or_else(|| Path::new("/"));
        let path: PathBuf = base.join("etc").join("environment");

        let contents = match tokio::fs::read_to_string(&path).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("no /etc/environment at {}", path.display());
                return Ok(DetectorOutput::default());
            }
            Err(e) => {
                return Err(DetectorError::Other(format!(
                    "failed to read {}: {e}",
                    path.display()
                )));
            }
        };

        let kv = parse_etc_environment(&contents);
        let facts = EnvFacts::from_env_with_source(&kv, EnvSource::EtcEnvironment);

        let mut notes = Vec::new();
        notes.push(format!("parsed {}", path.display()));

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
    use std::collections::HashMap;

    async fn run_with_sysroot(root: &std::path::Path) -> DetectorOutput {
        let det = EtcEnvironmentDetector::new();
        let ctx = DetectorContext {
            env: HashMap::default(),
            sysroot: Some(root.to_path_buf()),
            target_app: None,
        };
        det.run(&ctx).await.expect("detector should not fail")
    }

    fn seed_etc_environment(root: &std::path::Path, body: &str) {
        let etc = root.join("etc");
        std::fs::create_dir_all(&etc).expect("mkdir etc");
        std::fs::write(etc.join("environment"), body).expect("write /etc/environment");
    }

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

    #[tokio::test]
    async fn parses_ibus_vars_and_tags_them_etc_environment() {
        let tmp = tempfile_dir("env-etc-env-ibus");
        seed_etc_environment(&tmp, "GTK_IM_MODULE=ibus\nQT_IM_MODULE=ibus\nXMODIFIERS=@im=ibus\n");
        let out = run_with_sysroot(&tmp).await;
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.qt_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.xmodifiers.as_deref(), Some("@im=ibus"));
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::EtcEnvironment));
        assert_eq!(facts.sources.get("QT_IM_MODULE"), Some(&EnvSource::EtcEnvironment));
        assert_eq!(facts.sources.get("XMODIFIERS"), Some(&EnvSource::EtcEnvironment));
    }

    #[tokio::test]
    async fn missing_file_is_not_a_failure() {
        let tmp = tempfile_dir("env-etc-env-missing");
        let out = run_with_sysroot(&tmp).await;
        // Missing file → empty partial, no env facts emitted.
        assert!(out.partial.env.is_none());
    }

    #[tokio::test]
    async fn handles_export_prefix_and_quotes() {
        let tmp = tempfile_dir("env-etc-env-export");
        seed_etc_environment(&tmp, "export GTK_IM_MODULE=\"fcitx\"\nexport QT_IM_MODULE='fcitx'\n");
        let out = run_with_sysroot(&tmp).await;
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("fcitx"));
        assert_eq!(facts.qt_im_module.as_deref(), Some("fcitx"));
    }

    #[tokio::test]
    async fn malformed_lines_are_silently_dropped() {
        // Parser dropping bad lines is covered in the core's parser tests;
        // this guards the detector doesn't somehow upgrade the silent drop
        // to an error on its way out.
        let tmp = tempfile_dir("env-etc-env-bad");
        seed_etc_environment(&tmp, "no-equals-here\n1INVALID=x\nGTK_IM_MODULE=ibus\n bad key =z\n");
        let out = run_with_sysroot(&tmp).await;
        let facts = out.partial.env.expect("env facts present");
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
    }

    #[tokio::test]
    async fn id_is_env_etc_environment() {
        let d = EtcEnvironmentDetector::new();
        assert_eq!(d.id(), "env.etc_environment");
    }
}
