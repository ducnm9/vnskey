// SPDX-License-Identifier: GPL-3.0-or-later
//
// `PackageOps` — async trait abstraction over the distro package manager.
//
// One implementation per family: `AptOps` for Debian/Ubuntu, `DnfOps` for
// Fedora (INS-50), `PacmanOps` for Arch (INS-51). The executor picks the
// right one from `PreState::distro_family` at runtime.
//
// The trait has four methods:
//
//   * `list_installed(packages)` — returns the subset already present.
//     Used by `InstallPackages` for idempotency and by the snapshot
//     `already_present` field (so rollback doesn't wipe packages the user
//     had before).
//   * `install(packages, sudo)`  — installs via the package manager,
//     non-interactive.
//   * `uninstall(packages, sudo)` — the inverse, for rollback + uninstall
//     command.
//   * `refresh_metadata(sudo)`   — `apt-get update` / `dnf makecache`
//     before install. Cheap to call repeatedly.
//
// All methods are `async` so the executor can stream progress. The MVP
// runs them serially.
//
// Spec ref: `spec/02-phase2-installer.md` §B.6.

use std::ffi::OsString;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;

use crate::model::PackageManager;

/// Errors that the package ops layer surfaces. `NotInstalled` is a soft
/// signal used by the executor to short-circuit install-skip paths; every
/// other variant is a hard failure.
#[derive(Debug, thiserror::Error)]
pub enum PackageOpsError {
    #[error("package manager `{cmd}` not found on PATH — install it or run via a distro packager")]
    ManagerMissing { cmd: &'static str },
    #[error("package manager `{cmd}` exited with status {status}: {stderr}")]
    CommandFailed { cmd: &'static str, status: i32, stderr: String },
    #[error("I/O error while running `{cmd}`: {source}")]
    Io { cmd: &'static str, source: std::io::Error },
}

/// Which privilege wrapper (if any) the caller wants prepended to the
/// package-manager command. `Unattended` lets the CLI pass a preflighted
/// batch that won't prompt at the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sudo {
    /// Run directly — either we already have root or the op is harmless
    /// (`apt-cache policy`, `dpkg-query`).
    None,
    /// Prefix with `sudo`. Interactive: may prompt on the TTY.
    Interactive,
    /// Prefix with `sudo -n` (non-interactive). Fails loudly if a
    /// password would be required.
    Unattended,
}

impl Sudo {
    fn args(self) -> &'static [&'static str] {
        match self {
            Self::None => &[],
            Self::Interactive => &["sudo"],
            Self::Unattended => &["sudo", "-n"],
        }
    }
}

/// Async trait all package-op implementations share. Each impl is a
/// zero-sized marker type; callers construct it on demand.
#[async_trait]
pub trait PackageOps: Send + Sync + std::fmt::Debug {
    /// The `PackageManager` variant this impl covers. Used by the
    /// executor to match against `Step::InstallPackages::manager`.
    fn manager(&self) -> PackageManager;

    /// Of the requested packages, return the names already installed.
    /// Must NOT shell out to a package manager that requires root; the
    /// executor relies on this being cheap and pre-flight-safe.
    async fn list_installed(&self, packages: &[String]) -> Result<Vec<String>, PackageOpsError>;

    /// Refresh the package index (`apt-get update` etc.). Requires sudo.
    async fn refresh_metadata(&self, sudo: Sudo) -> Result<(), PackageOpsError>;

    /// Install all of `packages` unconditionally. Caller is expected to
    /// have filtered out already-installed entries via `list_installed`.
    async fn install(&self, packages: &[String], sudo: Sudo) -> Result<(), PackageOpsError>;

    /// Uninstall. Used by rollback and the `uninstall` subcommand.
    async fn uninstall(&self, packages: &[String], sudo: Sudo) -> Result<(), PackageOpsError>;
}

// ─── AptOps ───────────────────────────────────────────────────────────────

/// `apt-get` + `dpkg-query` implementation. The only impl wired up for
/// the v0.1 installer MVP.
#[derive(Debug, Default, Clone, Copy)]
pub struct AptOps;

#[async_trait]
impl PackageOps for AptOps {
    fn manager(&self) -> PackageManager {
        PackageManager::Apt
    }

    async fn list_installed(&self, packages: &[String]) -> Result<Vec<String>, PackageOpsError> {
        if packages.is_empty() {
            return Ok(Vec::new());
        }
        // `dpkg-query -W` returns 1 when some packages are missing but
        // still prints the ones that are present. Treat a non-zero exit
        // as "not fatal" as long as we got stdout — but a missing binary
        // is always fatal.
        match run("dpkg-query", &build_dpkg_query_args(packages), Sudo::None).await {
            Ok(stdout) => Ok(parse_dpkg_query_output(&stdout)),
            Err(PackageOpsError::ManagerMissing { cmd }) => {
                Err(PackageOpsError::ManagerMissing { cmd })
            }
            Err(PackageOpsError::CommandFailed { .. }) => {
                // Some packages missing — rerun per-package to get the
                // names of those that ARE installed. Cheap since the
                // happy path (all installed) already returned via Ok.
                let mut installed = Vec::new();
                for pkg in packages {
                    if let Ok(stdout) = run(
                        "dpkg-query",
                        &build_dpkg_query_args(std::slice::from_ref(pkg)),
                        Sudo::None,
                    )
                    .await
                    {
                        installed.extend(parse_dpkg_query_output(&stdout));
                    }
                }
                Ok(installed)
            }
            Err(e) => Err(e),
        }
    }

    async fn refresh_metadata(&self, sudo: Sudo) -> Result<(), PackageOpsError> {
        let _ = run("apt-get", &["update", "-qq"], sudo).await?;
        Ok(())
    }

    async fn install(&self, packages: &[String], sudo: Sudo) -> Result<(), PackageOpsError> {
        if packages.is_empty() {
            return Ok(());
        }
        let mut args = vec!["install", "-y", "--no-install-recommends"];
        let package_args: Vec<&str> = packages.iter().map(String::as_str).collect();
        args.extend(package_args);
        let _ = run("apt-get", &args, sudo).await?;
        Ok(())
    }

    async fn uninstall(&self, packages: &[String], sudo: Sudo) -> Result<(), PackageOpsError> {
        if packages.is_empty() {
            return Ok(());
        }
        let mut args = vec!["remove", "-y"];
        let package_args: Vec<&str> = packages.iter().map(String::as_str).collect();
        args.extend(package_args);
        let _ = run("apt-get", &args, sudo).await?;
        Ok(())
    }
}

fn build_dpkg_query_args(packages: &[String]) -> Vec<&str> {
    // `dpkg-query -W -f='${Package}\t${db:Status-Status}\n' PKG1 PKG2 …`
    let mut args = vec!["-W", "-f=${Package}\t${db:Status-Status}\n"];
    args.extend(packages.iter().map(String::as_str));
    args
}

fn parse_dpkg_query_output(stdout: &str) -> Vec<String> {
    let mut installed = Vec::new();
    for line in stdout.lines() {
        let mut parts = line.split('\t');
        let Some(name) = parts.next() else { continue };
        let status = parts.next().unwrap_or("").trim();
        if status == "installed" && !name.is_empty() {
            installed.push(name.to_owned());
        }
    }
    installed
}

// ─── Process runner ───────────────────────────────────────────────────────

/// Internal runner. Prefixes `sudo` if requested, captures stdout/stderr,
/// and maps distro-specific failures to `PackageOpsError` variants.
async fn run(cmd: &'static str, args: &[&str], sudo: Sudo) -> Result<String, PackageOpsError> {
    let mut full: Vec<OsString> = sudo.args().iter().map(OsString::from).collect();
    full.push(OsString::from(cmd));
    full.extend(args.iter().map(OsString::from));

    // Split the first element off as the actual program to exec.
    let fallback = OsString::from(cmd);
    let (program, rest) = full.split_first().unwrap_or((&fallback, &[]));
    let program = program.clone();
    let rest: Vec<OsString> = rest.to_vec();

    let mut command = Command::new(&program);
    command
        .args(&rest)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("DEBIAN_FRONTEND", "noninteractive")
        .env("LC_ALL", "C");

    let output = command.output().await.map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            PackageOpsError::ManagerMissing { cmd }
        } else {
            PackageOpsError::Io { cmd, source }
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(PackageOpsError::CommandFailed {
            cmd,
            status: output.status.code().unwrap_or(-1),
            stderr,
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn sudo_args_match_expected_prefix() {
        assert_eq!(Sudo::None.args(), &[] as &[&'static str]);
        assert_eq!(Sudo::Interactive.args(), &["sudo"]);
        assert_eq!(Sudo::Unattended.args(), &["sudo", "-n"]);
    }

    #[test]
    fn dpkg_query_output_parses_installed_rows() {
        let stdout = "fcitx5\tinstalled\nfcitx5-bamboo\tnot-installed\ngit\tinstalled\n";
        let installed = parse_dpkg_query_output(stdout);
        assert_eq!(installed, vec!["fcitx5".to_owned(), "git".to_owned()]);
    }

    #[test]
    fn dpkg_query_output_tolerates_garbage_lines() {
        let stdout = "\n\nfcitx5\tinstalled\nthis is garbage\n";
        let installed = parse_dpkg_query_output(stdout);
        assert_eq!(installed, vec!["fcitx5".to_owned()]);
    }

    #[test]
    fn apt_manager_reports_apt() {
        assert_eq!(AptOps.manager(), PackageManager::Apt);
    }

    // Online tests that actually invoke `dpkg-query` / `apt-get` live in
    // `tests/package_ops_apt.rs` (gated on the Docker matrix, see
    // INS-70).
}
