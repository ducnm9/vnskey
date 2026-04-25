// SPDX-License-Identifier: GPL-3.0-or-later
//
// `im.engines.packages` — enumerates Vietnamese IME packages installed
// via the host's native package manager.
//
// Routed by distro family:
//
//   * **Debian / Ubuntu / Mint / Pop** → `dpkg-query`
//   * **Redhat / Fedora / CentOS** → `rpm`
//   * **Arch / Manjaro** → `pacman`
//   * **Unknown** → no-op (no engines emitted, no anomaly)
//
// Package set we ask for: `ibus-bamboo`, `ibus-unikey`, `fcitx5-bamboo`,
// `fcitx5-unikey`. Spec-scope gate for Phase 1 — additions go via a
// Week 5+ config, not here.
//
// Why re-parse `os-release` ourselves instead of reading from
// `DetectorContext` (which currently does NOT carry a `DistroFamily`):
// Week 3 deliberately avoids the 2-pass orchestrator refactor that would
// have injected distro facts into `DetectorContext` before this detector
// runs. `os-release` is a 200-byte file; parsing it twice (once here,
// once in DOC-03) costs < 1 ms. The alternative — having detectors call
// each other — violates rule 4 of the `Detector` trait contract. A
// `TODO` anchor below pegs this to the Week 4 2-pass work.
//
// On any command failure (binary missing, non-zero exit with empty
// stdout) we return an empty partial. The checker layer and DOC-21/23
// still have signal from `ibus list-engine` / `fcitx5 profile` — we're
// just enriching the engines list with package provenance.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-24).

// TODO(week-4): drop the self-parse of os-release once DetectorContext
// carries a pre-computed DistroFamily (2-pass model).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::{
    detect_from_os_release, is_vietnamese_engine, DistroFamily, EngineFact, ImFramework,
};

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};
use crate::process::{CommandRunner, TokioCommandRunner};

/// The four packages we look for on every family. Order is stable so
/// test snapshots stay deterministic.
const VIETNAMESE_PKGS: [&str; 4] = ["ibus-bamboo", "ibus-unikey", "fcitx5-bamboo", "fcitx5-unikey"];

#[derive(Debug)]
pub struct PackageEnginesDetector {
    runner: Arc<dyn CommandRunner>,
    /// Override for tests — skips the os-release parse entirely.
    family_override: Option<DistroFamily>,
}

impl Default for PackageEnginesDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageEnginesDetector {
    #[must_use]
    pub fn new() -> Self {
        // Package queries can be slow on large dpkg databases; give them
        // a 4s head.
        Self {
            runner: Arc::new(TokioCommandRunner::with_timeout(Duration::from_secs(4))),
            family_override: None,
        }
    }

    #[must_use]
    pub fn with_runner(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner, family_override: None }
    }

    /// Test seam: skip the os-release parse and use this family directly.
    #[must_use]
    pub fn with_family(runner: Arc<dyn CommandRunner>, family: DistroFamily) -> Self {
        Self { runner, family_override: Some(family) }
    }
}

#[async_trait]
impl Detector for PackageEnginesDetector {
    fn id(&self) -> &'static str {
        "im.engines.packages"
    }

    fn timeout(&self) -> Duration {
        // Package managers are the slowest path in Phase 1; 5s safety net.
        Duration::from_secs(5)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        let family = if let Some(f) = self.family_override {
            f
        } else if let Some(f) = detect_family(ctx).await {
            f
        } else {
            debug!("could not determine distro family; skipping");
            return Ok(DetectorOutput::default());
        };

        let engines = match family {
            DistroFamily::Debian => query_dpkg(&self.runner).await,
            DistroFamily::Redhat => query_rpm(&self.runner).await,
            DistroFamily::Arch => query_pacman(&self.runner).await,
            // SUSE → zypper, Alpine → apk, Nix → nix-env — planned for
            // Week 6 package routing. Until then, fall through silently.
            DistroFamily::Suse
            | DistroFamily::Alpine
            | DistroFamily::Nix
            | DistroFamily::Unknown => Vec::new(),
        };

        if engines.is_empty() {
            return Ok(DetectorOutput::default());
        }

        let note = format!("{:?}: {} Vietnamese package(s) installed", family, engines.len());
        Ok(DetectorOutput {
            partial: PartialFacts { engines, ..PartialFacts::default() },
            notes: vec![note],
        })
    }
}

/// Read `/etc/os-release` (sysroot-aware) and map to a `DistroFamily`.
async fn detect_family(ctx: &DetectorContext) -> Option<DistroFamily> {
    let base: &Path = ctx.sysroot.as_deref().unwrap_or_else(|| Path::new("/"));
    for rel in ["etc/os-release", "usr/lib/os-release"] {
        let p = base.join(rel);
        if let Ok(body) = tokio::fs::read_to_string(&p).await {
            return Some(detect_from_os_release(&body).family);
        }
    }
    None
}

async fn query_dpkg(runner: &Arc<dyn CommandRunner>) -> Vec<EngineFact> {
    // `dpkg-query -W -f='${Package}\t${Version}\n' <pkgs>...`
    // Unmatched packages go to stderr ("no packages found"); matched
    // ones print `name\tversion\n` to stdout.
    let mut args: Vec<&str> = vec!["-W", "-f=${Package}\t${Version}\n"];
    args.extend(VIETNAMESE_PKGS);
    let stdout = match runner.run("dpkg-query", &args).await {
        Ok(s) => s,
        Err(e) => {
            debug!("dpkg-query failed: {e}");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Tab-separated; tolerate space-separated too.
        let (pkg, ver) = match line.split_once('\t') {
            Some(p) => p,
            None => match line.split_once(char::is_whitespace) {
                Some(p) => p,
                None => continue,
            },
        };
        if let Some(e) = engine_from_package(pkg.trim(), Some(ver.trim().to_owned())) {
            out.push(e);
        }
    }
    out
}

async fn query_rpm(runner: &Arc<dyn CommandRunner>) -> Vec<EngineFact> {
    // `rpm -q --qf "%{NAME}\t%{VERSION}\n" <pkgs>...`
    // Unmatched entries print `package X is not installed` to stdout —
    // we skip those.
    let mut args: Vec<&str> = vec!["-q", "--qf", "%{NAME}\t%{VERSION}\n"];
    args.extend(VIETNAMESE_PKGS);
    let stdout = match runner.run("rpm", &args).await {
        Ok(s) => s,
        Err(e) => {
            debug!("rpm failed: {e}");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.contains("is not installed") {
            continue;
        }
        let Some((pkg, ver)) = line.split_once('\t') else { continue };
        if let Some(e) = engine_from_package(pkg.trim(), Some(ver.trim().to_owned())) {
            out.push(e);
        }
    }
    out
}

async fn query_pacman(runner: &Arc<dyn CommandRunner>) -> Vec<EngineFact> {
    // `pacman -Q <pkgs>...` prints `name version` per installed
    // package. Uninstalled ones are written to stderr ("error: package
    // ... was not found") which our runner discards.
    let mut args: Vec<&str> = vec!["-Q"];
    args.extend(VIETNAMESE_PKGS);
    let stdout = match runner.run("pacman", &args).await {
        Ok(s) => s,
        Err(e) => {
            debug!("pacman failed: {e}");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("error:") {
            continue;
        }
        let Some((pkg, ver)) = line.split_once(char::is_whitespace) else { continue };
        if let Some(e) = engine_from_package(pkg.trim(), Some(ver.trim().to_owned())) {
            out.push(e);
        }
    }
    out
}

/// Map a package name like `ibus-bamboo` to an `EngineFact`.
///
/// `is_registered` is `false` — the orchestrator's reconciliation pass
/// flips it to `true` if the engine also shows up in
/// `ibus.registered_engines` or `fcitx5.input_methods_configured`.
fn engine_from_package(pkg: &str, version: Option<String>) -> Option<EngineFact> {
    let (framework, engine_id) = if let Some(rest) = pkg.strip_prefix("ibus-") {
        (ImFramework::Ibus, rest)
    } else if let Some(rest) = pkg.strip_prefix("fcitx5-") {
        (ImFramework::Fcitx5, rest)
    } else {
        return None;
    };
    if engine_id.is_empty() {
        return None;
    }
    Some(EngineFact {
        name: engine_id.to_owned(),
        package: Some(pkg.to_owned()),
        version,
        framework,
        is_vietnamese: is_vietnamese_engine(engine_id),
        is_registered: false,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::process::tests::FakeCommandRunner;

    fn dpkg_runner(stdout: &str) -> Arc<FakeCommandRunner> {
        let mut r = FakeCommandRunner::default();
        r.ok.insert(
            (
                "dpkg-query".to_owned(),
                format!("-W -f=${{Package}}\\t${{Version}}\\n {}", VIETNAMESE_PKGS.join(" ")),
            ),
            stdout.to_owned(),
        );
        Arc::new(r)
    }

    // FakeCommandRunner's key is `args.join(" ")` — we need the exact
    // arg vector the detector passes in, including the literal format
    // string. Construct the key the same way the fake does.
    fn key(program: &str, args: &[&str]) -> (String, String) {
        (program.to_owned(), args.join(" "))
    }

    fn insert_ok(r: &mut FakeCommandRunner, program: &str, args: &[&str], stdout: &str) {
        r.ok.insert(key(program, args), stdout.to_owned());
    }

    fn insert_err(
        r: &mut FakeCommandRunner,
        program: &str,
        args: &[&str],
        kind: std::io::ErrorKind,
    ) {
        r.err.insert(key(program, args), kind);
    }

    fn dpkg_args() -> Vec<&'static str> {
        let mut a: Vec<&str> = vec!["-W", "-f=${Package}\t${Version}\n"];
        a.extend(VIETNAMESE_PKGS);
        a
    }

    fn rpm_args() -> Vec<&'static str> {
        let mut a: Vec<&str> = vec!["-q", "--qf", "%{NAME}\t%{VERSION}\n"];
        a.extend(VIETNAMESE_PKGS);
        a
    }

    fn pacman_args() -> Vec<&'static str> {
        let mut a: Vec<&str> = vec!["-Q"];
        a.extend(VIETNAMESE_PKGS);
        a
    }

    #[tokio::test]
    async fn debian_with_bamboo_and_unikey_installed() {
        let mut r = FakeCommandRunner::default();
        insert_ok(
            &mut r,
            "dpkg-query",
            &dpkg_args(),
            "ibus-bamboo\t0.8.2-1\nfcitx5-bamboo\t0.8.2-1\n",
        );
        // dpkg_runner helper not usable because our key uses real tabs
        // in the format string; build it manually above.
        let _ = dpkg_runner; // suppress unused warning
        let det = PackageEnginesDetector::with_family(Arc::new(r), DistroFamily::Debian);
        let res = det.run(&DetectorContext::default()).await.expect("ok");
        assert_eq!(res.partial.engines.len(), 2);
        assert_eq!(res.partial.engines[0].name, "bamboo");
        assert_eq!(res.partial.engines[0].framework, ImFramework::Ibus);
        assert_eq!(res.partial.engines[0].package.as_deref(), Some("ibus-bamboo"));
        assert_eq!(res.partial.engines[0].version.as_deref(), Some("0.8.2-1"));
        assert_eq!(res.partial.engines[1].name, "bamboo");
        assert_eq!(res.partial.engines[1].framework, ImFramework::Fcitx5);
        assert!(res.partial.engines[1].is_vietnamese);
        // None are registered yet — that's the orchestrator's job.
        assert!(res.partial.engines.iter().all(|e| !e.is_registered));
    }

    #[tokio::test]
    async fn redhat_with_single_ibus_unikey_installed() {
        let mut r = FakeCommandRunner::default();
        insert_ok(
            &mut r,
            "rpm",
            &rpm_args(),
            "ibus-unikey\t0.6.1\npackage ibus-bamboo is not installed\n",
        );
        let det = PackageEnginesDetector::with_family(Arc::new(r), DistroFamily::Redhat);
        let res = det.run(&DetectorContext::default()).await.expect("ok");
        assert_eq!(res.partial.engines.len(), 1);
        assert_eq!(res.partial.engines[0].name, "unikey");
        assert_eq!(res.partial.engines[0].framework, ImFramework::Ibus);
    }

    #[tokio::test]
    async fn arch_reports_empty_when_pacman_errors() {
        let mut r = FakeCommandRunner::default();
        insert_err(&mut r, "pacman", &pacman_args(), std::io::ErrorKind::NotFound);
        let det = PackageEnginesDetector::with_family(Arc::new(r), DistroFamily::Arch);
        let res = det.run(&DetectorContext::default()).await.expect("ok");
        assert!(res.partial.engines.is_empty());
    }

    #[tokio::test]
    async fn arch_parses_pacman_output() {
        let mut r = FakeCommandRunner::default();
        insert_ok(&mut r, "pacman", &pacman_args(), "fcitx5-unikey 0.6.1-2\n");
        let det = PackageEnginesDetector::with_family(Arc::new(r), DistroFamily::Arch);
        let res = det.run(&DetectorContext::default()).await.expect("ok");
        assert_eq!(res.partial.engines.len(), 1);
        assert_eq!(res.partial.engines[0].framework, ImFramework::Fcitx5);
        assert_eq!(res.partial.engines[0].name, "unikey");
        assert_eq!(res.partial.engines[0].version.as_deref(), Some("0.6.1-2"));
    }

    #[tokio::test]
    async fn unknown_distro_yields_empty_partial() {
        let det = PackageEnginesDetector::with_family(
            Arc::new(FakeCommandRunner::default()),
            DistroFamily::Unknown,
        );
        let res = det.run(&DetectorContext::default()).await.expect("ok");
        assert!(res.partial.engines.is_empty());
    }

    #[tokio::test]
    async fn missing_os_release_yields_empty_partial() {
        // No family override, no sysroot with os-release → family detect fails.
        let det = PackageEnginesDetector::with_runner(Arc::new(FakeCommandRunner::default()));
        // Use a sysroot that contains no os-release so detect_family returns None.
        let tmp = std::env::temp_dir().join(format!(
            "vietime-pkg-nope-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&tmp).expect("mkdir tmp");
        let ctx = DetectorContext { env: std::collections::HashMap::new(), sysroot: Some(tmp) };
        let res = det.run(&ctx).await.expect("ok");
        assert!(res.partial.engines.is_empty());
    }

    #[test]
    fn engine_from_package_strips_prefix_and_flags_vietnamese() {
        let e = engine_from_package("ibus-bamboo", Some("1.0".to_owned())).expect("some");
        assert_eq!(e.name, "bamboo");
        assert_eq!(e.framework, ImFramework::Ibus);
        assert!(e.is_vietnamese);
        assert_eq!(e.version.as_deref(), Some("1.0"));
        // Non-matching prefix is skipped.
        assert!(engine_from_package("libreoffice-core", None).is_none());
    }

    #[tokio::test]
    async fn id_is_im_engines_packages() {
        let d = PackageEnginesDetector::new();
        assert_eq!(d.id(), "im.engines.packages");
    }
}
