// SPDX-License-Identifier: GPL-3.0-or-later
//
// Sudo handler — pre-flight privilege check + a thin wrapper around
// `sudo --validate`.
//
// The installer never caches a password, never stores credentials, and
// never shells out with `echo foo | sudo -S`. We ask sudo itself to
// re-use its own credential cache: if the user ran `sudo -v` once at the
// top of the session, every subsequent `sudo` call in this process
// inherits that cache. If no cache is available, we call `sudo -v` once
// and let sudo itself prompt on the TTY.
//
// Two entry points:
//
//   * `preflight(plan, mode)` — called by the Executor before any
//     mutating step runs. Returns `Ok` if we already have the creds we
//     need, otherwise prompts via `sudo -v`. In `Unattended` mode
//     (`--yes` or no TTY) this hard-fails instead of prompting.
//   * `prime_cache()` — convenience used by the install wizard after
//     confirmation. Same as `preflight` but always interactive.
//
// Spec ref: `spec/02-phase2-installer.md` §B.7.

use std::process::Stdio;

use tokio::process::Command;

use crate::model::{Plan, Step};
use crate::packageops::Sudo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightMode {
    /// Prompt on the TTY if sudo isn't already primed.
    Interactive,
    /// Never prompt; fail if a password would be required.
    Unattended,
}

#[derive(Debug, thiserror::Error)]
pub enum SudoError {
    #[error("this plan requires sudo but `sudo` is not installed on PATH")]
    SudoMissing,
    #[error(
        "this plan requires sudo but `--yes` / non-interactive mode was requested. \
         Re-run without `--yes`, or pre-authenticate with `sudo -v` before retrying."
    )]
    WouldPrompt,
    #[error("sudo credential check failed: {stderr}")]
    CheckFailed { stderr: String },
    #[error("I/O error running sudo: {0}")]
    Io(#[from] std::io::Error),
}

/// Does this plan need sudo at all? Runs through every step and returns
/// `true` the moment it hits a system-scope mutation.
#[must_use]
pub fn plan_requires_sudo(plan: &Plan) -> bool {
    plan.requires_sudo || plan.steps.iter().any(step_requires_sudo)
}

fn step_requires_sudo(step: &Step) -> bool {
    use crate::model::{EnvFile, Step as S};
    match step {
        S::InstallPackages { .. } | S::UninstallPackages { .. } | S::RunImConfig { .. } => true,
        S::SetEnvVar { file, .. } | S::UnsetEnvVar { file, .. } => {
            matches!(file, EnvFile::EtcEnvironment)
        }
        S::WriteFile { path, .. } => is_system_path(path),
        // BackupFile is read-only from the installer's side; reads are
        // fine as the current user (even `/etc/environment` is
        // world-readable). No sudo needed.
        S::BackupFile { .. }
        // systemd user units live under the user's config dir — no sudo.
        | S::SystemctlUserEnable { .. }
        | S::SystemctlUserDisable { .. }
        | S::SystemctlUserStart { .. }
        | S::SystemctlUserStop { .. }
        | S::Verify { .. }
        | S::Prompt { .. } => false,
    }
}

/// Narrow system-path heuristic. Excludes `/var/folders` (macOS tmp) and
/// `/var/tmp` to avoid false positives in unit tests run on macOS devs'
/// machines.
fn is_system_path(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    if s.starts_with("/var/folders/") || s.starts_with("/var/tmp/") || s.starts_with("/tmp/") {
        return false;
    }
    s.starts_with("/etc/") || s.starts_with("/usr/") || s.starts_with("/var/")
}

/// Pre-flight check called at the top of `Executor::run`. Returns the
/// `Sudo` mode the executor should use for subsequent package-manager
/// calls. If the plan doesn't need sudo, returns `Sudo::None` without
/// talking to `sudo` at all.
pub async fn preflight(plan: &Plan, mode: PreflightMode) -> Result<Sudo, SudoError> {
    if !plan_requires_sudo(plan) {
        return Ok(Sudo::None);
    }
    let have_sudo = which_sudo().await;
    if !have_sudo {
        return Err(SudoError::SudoMissing);
    }

    // Already-cached credentials? `sudo -n -v` returns 0 silently if so.
    if sudo_validate(&["-n", "-v"]).await.is_ok() {
        return Ok(match mode {
            PreflightMode::Interactive => Sudo::Interactive,
            PreflightMode::Unattended => Sudo::Unattended,
        });
    }

    match mode {
        PreflightMode::Unattended => Err(SudoError::WouldPrompt),
        PreflightMode::Interactive => {
            // Prime the cache. `sudo -v` prompts on the controlling TTY.
            sudo_validate(&["-v"]).await?;
            Ok(Sudo::Interactive)
        }
    }
}

/// Run `sudo -v` to prime the credential cache. Useful as the last step
/// of the install wizard — it means subsequent `sudo` calls in the same
/// session don't re-prompt.
pub async fn prime_cache() -> Result<(), SudoError> {
    if !which_sudo().await {
        return Err(SudoError::SudoMissing);
    }
    sudo_validate(&["-v"]).await
}

async fn which_sudo() -> bool {
    Command::new("sudo")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success())
}

async fn sudo_validate(args: &[&str]) -> Result<(), SudoError> {
    let out =
        Command::new("sudo").args(args).stdin(Stdio::inherit()).output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SudoError::SudoMissing
            } else {
                SudoError::Io(e)
            }
        })?;
    if out.status.success() {
        Ok(())
    } else {
        Err(SudoError::CheckFailed {
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::{
        Combo, Engine, EnvFile, Goal, PackageManager, Plan, Step, PLAN_SCHEMA_VERSION,
    };
    use crate::pre_state::PreState;
    use vietime_core::ImFramework;

    fn sample_plan_with_steps(steps: Vec<Step>) -> Plan {
        let mut p = Plan::new_skeleton(
            Goal::Install { combo: Combo::new(ImFramework::Fcitx5, Engine::Bamboo) },
            PreState::fixture_ubuntu_24_04(),
        );
        p.schema_version = PLAN_SCHEMA_VERSION;
        p.steps = steps;
        p
    }

    #[test]
    fn install_packages_step_requires_sudo() {
        let plan = sample_plan_with_steps(vec![Step::InstallPackages {
            manager: PackageManager::Apt,
            packages: vec!["fcitx5".into()],
        }]);
        assert!(plan_requires_sudo(&plan));
    }

    #[test]
    fn home_scope_envfile_does_not_require_sudo() {
        let plan = sample_plan_with_steps(vec![Step::SetEnvVar {
            file: EnvFile::HomeProfile,
            key: "GTK_IM_MODULE".into(),
            value: "fcitx".into(),
        }]);
        assert!(!plan_requires_sudo(&plan));
    }

    #[test]
    fn etc_environment_setenv_requires_sudo() {
        let plan = sample_plan_with_steps(vec![Step::SetEnvVar {
            file: EnvFile::EtcEnvironment,
            key: "GTK_IM_MODULE".into(),
            value: "fcitx".into(),
        }]);
        assert!(plan_requires_sudo(&plan));
    }

    #[test]
    fn systemctl_user_does_not_require_sudo() {
        let plan = sample_plan_with_steps(vec![Step::SystemctlUserEnable {
            unit: "fcitx5.service".into(),
        }]);
        assert!(!plan_requires_sudo(&plan));
    }

    #[test]
    fn verify_prompt_do_not_require_sudo() {
        let plan = sample_plan_with_steps(vec![
            Step::Verify {
                check: crate::model::VerifyCheck::DaemonRunning { framework: ImFramework::Fcitx5 },
            },
            Step::Prompt {
                message: "Logout".into(),
                continue_if: crate::model::PromptCondition::UserYes,
            },
        ]);
        assert!(!plan_requires_sudo(&plan));
    }
}
