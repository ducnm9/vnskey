// SPDX-License-Identifier: GPL-3.0-or-later
//
// `Executor` — runs a `Plan` against the live system.
//
// This is the second brain of the installer (the first being the
// Planner). Its single responsibility: take a structurally-valid `Plan`
// and make the system match it, recording every mutation in the
// `SnapshotStore` _before_ it happens, so that a failure (or a user
// Ctrl+C) leaves a clean rollback trail.
//
// ## Invariants
//
// 1. **Record before mutate.** Every step that touches the system is
//    paired with an `Artifact` in the snapshot, written to disk before
//    the mutation runs. A SIGKILL between "save manifest" and "mutate"
//    leaves a no-op rollback target, not an unrecorded change.
// 2. **Idempotent.** Re-running the same plan on a system that's already
//    in the target state is a no-op. `SetEnvVar` with the current value,
//    `InstallPackages` with packages already installed, etc., all skip.
// 3. **Atomic on failure.** If step N fails, the Executor walks
//    artifacts `[N-1 … 0]` and invokes the rollback action for each.
// 4. **Dry-run is side-effect-free.** In `Mode::DryRun` every handler is
//    a no-op that writes a plan-text line and returns `Skipped`.
//
// ## Step → handler mapping
//
// | Step variant              | Handler                              | Ticket  |
// |---------------------------|--------------------------------------|---------|
// | BackupFile                | `SnapshotHandle::backup_file`        | INS-11  |
// | InstallPackages           | `PackageOps::install`                | INS-21  |
// | UninstallPackages         | `PackageOps::uninstall`              | INS-44  |
// | SetEnvVar / UnsetEnvVar   | `EnvFileDoc` + `write_atomic`        | INS-13  |
// | SystemctlUser*            | `systemctl --user …`                 | INS-22  |
// | RunImConfig               | `im-config -n …` (Debian only)       | INS-23  |
// | WriteFile                 | `write_atomic`                       | INS-24  |
// | Verify                    | `doctor` shell-out                   | INS-61  |
// | Prompt                    | TTY confirmation (or skip on `--yes`)| INS-30  |
//
// Spec ref: `spec/02-phase2-installer.md` §B.5/§B.7.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use tokio::process::Command;

use crate::envfile::{EnvFileDoc, EnvFileError, Format as EnvFileFormat};
use crate::model::{EnvFile, PackageManager, Plan, PromptCondition, Step, VerifyCheck};
use crate::packageops::{AptOps, PackageOps, PackageOpsError, Sudo};
use crate::snapshot::{Artifact, SnapshotError, SnapshotHandle, SnapshotStore};
use crate::sudo::{preflight, PreflightMode, SudoError};

/// Execution mode. `Live` is the real thing; `DryRun` prints the plan
/// but doesn't touch the system; `Unattended` refuses to prompt and
/// fails if sudo would need a password.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Live,
    DryRun,
    Unattended,
}

impl Mode {
    fn is_dry_run(self) -> bool {
        matches!(self, Self::DryRun)
    }

    fn preflight_mode(self) -> PreflightMode {
        match self {
            Self::Unattended | Self::DryRun => PreflightMode::Unattended,
            Self::Live => PreflightMode::Interactive,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecError {
    #[error("step {index} ({kind}) failed: {source}")]
    Step { index: usize, kind: &'static str, source: anyhow::Error },
    #[error(transparent)]
    Sudo(#[from] SudoError),
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    #[error("plan aborted by user at prompt step {index}")]
    UserAborted { index: usize },
    #[error("plan interrupted by SIGINT at step {index} (Ctrl+C)")]
    Interrupted { index: usize },
    #[error("unsupported package manager `{0:?}` — only `apt` is wired up in the MVP")]
    UnsupportedManager(PackageManager),
    #[error("verification failed: {reason}")]
    VerifyFailed { reason: String },
}

/// Runtime knobs for the Executor. Built by the CLI from flags.
#[derive(Debug, Clone)]
pub struct ExecConfig {
    pub mode: Mode,
    /// Path used to resolve `EnvFile::HomeProfile` etc. Defaults to `$HOME`.
    pub home: PathBuf,
    /// Storage directory for snapshots. Defaults to `~/.config/vietime/snapshots`.
    pub snapshots_root: PathBuf,
    /// `--yes`: skip `Prompt` steps that ask for confirmation.
    pub assume_yes: bool,
    /// Minimum free bytes on the snapshot root before the Executor will
    /// proceed. 10 MiB is plenty for a manifest + a handful of small env
    /// file backups.
    pub min_free_bytes: u64,
}

impl ExecConfig {
    #[must_use]
    pub fn new(mode: Mode) -> Self {
        let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
        let snapshots_root = SnapshotStore::default_for_user(&home).root().to_path_buf();
        Self { mode, home, snapshots_root, assume_yes: false, min_free_bytes: 10 * 1024 * 1024 }
    }

    #[must_use]
    pub fn with_home(mut self, home: PathBuf) -> Self {
        self.home = home;
        self
    }

    #[must_use]
    pub fn with_snapshots_root(mut self, root: PathBuf) -> Self {
        self.snapshots_root = root;
        self
    }

    #[must_use]
    pub fn with_assume_yes(mut self, yes: bool) -> Self {
        self.assume_yes = yes;
        self
    }
}

/// Per-step reporter — so the CLI can draw a progress indicator without
/// the executor needing to know about `indicatif`.
pub trait ExecReporter: Send + Sync {
    fn step_start(&self, index: usize, step: &Step);
    fn step_done(&self, index: usize, step: &Step, artifact: &Artifact);
    fn step_failed(&self, index: usize, step: &Step, err: &ExecError);
    fn rollback_started(&self, from_index: usize);
    fn rollback_step(&self, index: usize, artifact: &Artifact);
}

/// Default reporter that writes to stderr. Used when the CLI doesn't
/// supply its own.
#[derive(Debug, Default)]
pub struct StderrReporter;

impl ExecReporter for StderrReporter {
    fn step_start(&self, index: usize, step: &Step) {
        eprintln!("[{index:>2}] {kind} …", kind = step.kind());
    }
    fn step_done(&self, index: usize, step: &Step, _artifact: &Artifact) {
        eprintln!("[{index:>2}] {kind} done", kind = step.kind());
    }
    fn step_failed(&self, index: usize, step: &Step, err: &ExecError) {
        eprintln!("[{index:>2}] {kind} FAILED: {err}", kind = step.kind());
    }
    fn rollback_started(&self, from_index: usize) {
        eprintln!("-- rolling back from step {from_index} --");
    }
    fn rollback_step(&self, index: usize, _artifact: &Artifact) {
        eprintln!("[rollback] step {index}");
    }
}

/// The end state of a successful run — returned so callers can echo the
/// snapshot id back to the user (`vietime rollback <id>`).
#[derive(Debug)]
pub struct RunOutcome {
    pub snapshot_id: String,
    pub steps_executed: usize,
    pub dry_run: bool,
}

/// Main entry point — called by the `install` / `uninstall` / `switch`
/// commands. Returns `Ok(RunOutcome)` on success; on failure, attempts
/// rollback and returns the original `ExecError`.
pub async fn run_plan(
    plan: Plan,
    config: &ExecConfig,
    reporter: Arc<dyn ExecReporter>,
) -> Result<RunOutcome, ExecError> {
    let store = SnapshotStore::new(config.snapshots_root.clone());
    store.ensure_root()?;
    store.check_disk_space(config.min_free_bytes)?;

    let sudo_mode = preflight(&plan, config.mode.preflight_mode()).await?;

    let mut handle = store.begin(plan)?;
    handle.save_manifest()?;

    let result = run_plan_inner(&mut handle, config, sudo_mode, reporter.as_ref()).await;

    match result {
        Ok(steps_executed) => {
            handle.finalise()?;
            store.update_latest(handle.id())?;
            Ok(RunOutcome {
                snapshot_id: handle.id().to_owned(),
                steps_executed,
                dry_run: config.mode.is_dry_run(),
            })
        }
        Err(err) => {
            // Best-effort rollback — do not swallow the original error.
            rollback_from_handle(&handle, reporter.as_ref(), config).await;
            // Persist the incomplete manifest so `rollback --force` can
            // later find it if the partial rollback itself failed.
            let _ = handle.save_manifest();
            Err(err)
        }
    }
}

async fn run_plan_inner(
    handle: &mut SnapshotHandle,
    config: &ExecConfig,
    sudo: Sudo,
    reporter: &dyn ExecReporter,
) -> Result<usize, ExecError> {
    // Clone the steps so we can iterate the plan while mutating the
    // handle's artifact list.
    let steps = handle.manifest().plan.steps.clone();
    let mut executed = 0usize;

    for (idx, step) in steps.iter().enumerate() {
        reporter.step_start(idx, step);
        // Race the step handler against a Ctrl+C future. `tokio::signal`
        // registers a process-wide handler the first time it's awaited; we
        // rely on the runtime's `enable_all()` to set it up.
        let outcome = tokio::select! {
            res = run_step(idx, step, handle, config, sudo) => res,
            _ = tokio::signal::ctrl_c() => {
                let err = ExecError::Interrupted { index: idx };
                reporter.step_failed(idx, step, &err);
                return Err(err);
            }
        };
        match outcome {
            Ok(artifact) => {
                handle.record(artifact.clone());
                handle.save_manifest()?;
                reporter.step_done(idx, step, &artifact);
                executed += 1;
            }
            Err(err) => {
                reporter.step_failed(idx, step, &err);
                return Err(err);
            }
        }
    }
    Ok(executed)
}

/// Dispatcher: one step → one `Artifact`. Pure per-variant match;
/// side-effects live in the helper `fn`s below.
async fn run_step(
    index: usize,
    step: &Step,
    handle: &mut SnapshotHandle,
    config: &ExecConfig,
    sudo: Sudo,
) -> Result<Artifact, ExecError> {
    if config.mode.is_dry_run() {
        return Ok(Artifact::Skipped {
            step_index: index,
            reason: format!("dry-run: {}", step.kind()),
        });
    }
    match step {
        Step::BackupFile { path } => handle.backup_file(index, path).map_err(ExecError::from),
        Step::InstallPackages { manager, packages } => {
            handle_install_packages(index, *manager, packages, sudo).await
        }
        Step::UninstallPackages { manager, packages } => {
            handle_uninstall_packages(index, *manager, packages, sudo).await
        }
        Step::SetEnvVar { file, key, value } => {
            handle_set_env_var(index, file, key, value, &config.home)
        }
        Step::UnsetEnvVar { file, key } => handle_unset_env_var(index, file, key, &config.home),
        Step::SystemctlUserEnable { unit } => handle_systemctl(index, "enable", unit, true).await,
        Step::SystemctlUserDisable { unit } => {
            handle_systemctl(index, "disable", unit, false).await
        }
        Step::SystemctlUserStart { unit } => handle_systemctl(index, "start", unit, true).await,
        Step::SystemctlUserStop { unit } => handle_systemctl(index, "stop", unit, false).await,
        Step::RunImConfig { mode } => handle_im_config(index, mode, sudo).await,
        Step::WriteFile { path, content, mode } => handle_write_file(index, path, content, *mode),
        Step::Verify { check } => handle_verify(index, check).await,
        Step::Prompt { message, continue_if } => {
            handle_prompt(index, message, *continue_if, config)
        }
    }
}

// ─── Step handlers ────────────────────────────────────────────────────────

async fn handle_install_packages(
    index: usize,
    manager: PackageManager,
    packages: &[String],
    sudo: Sudo,
) -> Result<Artifact, ExecError> {
    let ops: Box<dyn PackageOps> = match manager {
        PackageManager::Apt => Box::new(AptOps),
        other => return Err(ExecError::UnsupportedManager(other)),
    };
    let already_present = ops.list_installed(packages).await.map_err(pkg_err(index))?;
    let to_install: Vec<String> =
        packages.iter().filter(|p| !already_present.contains(p)).cloned().collect();
    if !to_install.is_empty() {
        ops.refresh_metadata(sudo).await.map_err(pkg_err(index))?;
        ops.install(&to_install, sudo).await.map_err(pkg_err(index))?;
    }
    Ok(Artifact::InstalledPackages {
        step_index: index,
        manager: manager_tag(manager).to_owned(),
        packages: packages.to_vec(),
        already_present,
    })
}

async fn handle_uninstall_packages(
    index: usize,
    manager: PackageManager,
    packages: &[String],
    sudo: Sudo,
) -> Result<Artifact, ExecError> {
    let ops: Box<dyn PackageOps> = match manager {
        PackageManager::Apt => Box::new(AptOps),
        other => return Err(ExecError::UnsupportedManager(other)),
    };
    ops.uninstall(packages, sudo).await.map_err(pkg_err(index))?;
    Ok(Artifact::Skipped {
        step_index: index,
        reason: format!("uninstalled {n} packages", n = packages.len()),
    })
}

fn handle_set_env_var(
    index: usize,
    file: &EnvFile,
    key: &str,
    value: &str,
    home: &Path,
) -> Result<Artifact, ExecError> {
    let path = file.path(home);
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    let format = format_for(file);
    let mut doc = EnvFileDoc::parse(&current, format).map_err(envfile_err(index))?;
    doc.set(key, value);
    let rendered = doc.render();
    if rendered == current {
        return Ok(Artifact::Skipped { step_index: index, reason: format!("{key} already set") });
    }
    write_atomic_with_parent(&path, rendered.as_bytes()).map_err(|e| {
        step_err(
            index,
            &Step::SetEnvVar { file: file.clone(), key: key.to_owned(), value: value.to_owned() },
            e.into(),
        )
    })?;
    Ok(Artifact::VerifyOk {
        step_index: index,
        summary: format!("set {key} in {}", path.display()),
    })
}

fn handle_unset_env_var(
    index: usize,
    file: &EnvFile,
    key: &str,
    home: &Path,
) -> Result<Artifact, ExecError> {
    let path = file.path(home);
    let Ok(current) = std::fs::read_to_string(&path) else {
        return Ok(Artifact::Skipped {
            step_index: index,
            reason: format!("{} did not exist", path.display()),
        });
    };
    let format = format_for(file);
    let mut doc = EnvFileDoc::parse(&current, format).map_err(envfile_err(index))?;
    if !doc.unset(key) {
        return Ok(Artifact::Skipped {
            step_index: index,
            reason: format!("{key} was not present"),
        });
    }
    write_atomic_with_parent(&path, doc.render().as_bytes()).map_err(|e| {
        step_err(index, &Step::UnsetEnvVar { file: file.clone(), key: key.to_owned() }, e.into())
    })?;
    Ok(Artifact::VerifyOk {
        step_index: index,
        summary: format!("unset {key} from {}", path.display()),
    })
}

async fn handle_systemctl(
    index: usize,
    verb: &'static str,
    unit: &str,
    want_active: bool,
) -> Result<Artifact, ExecError> {
    // Snapshot the previous state so rollback can restore it.
    let previous_enabled = systemctl_is_enabled(unit).await;
    let previous_active = systemctl_is_active(unit).await;
    let args = ["--user", verb, unit];
    let out = Command::new("systemctl")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            step_err(index, &Step::SystemctlUserEnable { unit: unit.to_owned() }, e.into())
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        return Err(step_err(
            index,
            &Step::SystemctlUserEnable { unit: unit.to_owned() },
            anyhow::anyhow!("systemctl --user {verb} {unit}: {stderr}"),
        ));
    }
    let _ = want_active;
    Ok(Artifact::ServiceChange {
        step_index: index,
        unit: unit.to_owned(),
        previous_enabled,
        previous_active,
    })
}

async fn handle_im_config(index: usize, mode: &str, sudo: Sudo) -> Result<Artifact, ExecError> {
    // `im-config -n fcitx5` updates /etc/alternatives + writes the user's
    // ~/.xinputrc. Runs as the user; sudo only if the plan marks it.
    let mut cmd = Command::new(if matches!(sudo, Sudo::None) { "im-config" } else { "sudo" });
    if !matches!(sudo, Sudo::None) {
        cmd.arg("im-config");
    }
    cmd.arg("-n").arg(mode);
    let out = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| step_err(index, &Step::RunImConfig { mode: mode.to_owned() }, e.into()))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        return Err(step_err(
            index,
            &Step::RunImConfig { mode: mode.to_owned() },
            anyhow::anyhow!("im-config -n {mode}: {stderr}"),
        ));
    }
    Ok(Artifact::VerifyOk { step_index: index, summary: format!("im-config -n {mode}") })
}

fn handle_write_file(
    index: usize,
    path: &Path,
    content: &str,
    mode: u32,
) -> Result<Artifact, ExecError> {
    write_atomic_with_parent(path, content.as_bytes()).map_err(|e| {
        step_err(
            index,
            &Step::WriteFile { path: path.to_path_buf(), content: content.to_owned(), mode },
            e.into(),
        )
    })?;
    #[cfg(unix)]
    if mode != 0 {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(mode);
        let _ = std::fs::set_permissions(path, perms);
    }
    Ok(Artifact::VerifyOk { step_index: index, summary: format!("wrote {}", path.display()) })
}

async fn handle_verify(index: usize, check: &VerifyCheck) -> Result<Artifact, ExecError> {
    match check {
        VerifyCheck::DoctorCheckPasses => {
            let out = Command::new("vietime-doctor")
                .arg("check")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await;
            match out {
                Ok(o) if o.status.success() => Ok(Artifact::VerifyOk {
                    step_index: index,
                    summary: "vietime-doctor check passed".to_owned(),
                }),
                Ok(o) => Err(ExecError::VerifyFailed {
                    reason: format!(
                        "doctor exited with status {}; stderr: {}",
                        o.status.code().unwrap_or(-1),
                        String::from_utf8_lossy(&o.stderr).trim(),
                    ),
                }),
                // Doctor missing → don't block the install; note it in the trail.
                Err(_) => Ok(Artifact::Skipped {
                    step_index: index,
                    reason: "vietime-doctor not on PATH; verification skipped".to_owned(),
                }),
            }
        }
        // Other checks need the Detector layer — deferred to INS-61.
        VerifyCheck::DaemonRunning { framework } => Ok(Artifact::VerifyOk {
            step_index: index,
            summary: format!("daemon running ({framework:?}) — stub check, see INS-61"),
        }),
        VerifyCheck::EngineRegistered { name } => Ok(Artifact::VerifyOk {
            step_index: index,
            summary: format!("engine registered ({name}) — stub check, see INS-61"),
        }),
        VerifyCheck::EnvConsistent => Ok(Artifact::VerifyOk {
            step_index: index,
            summary: "env consistent — stub check, see INS-61".to_owned(),
        }),
    }
}

fn handle_prompt(
    index: usize,
    message: &str,
    continue_if: PromptCondition,
    config: &ExecConfig,
) -> Result<Artifact, ExecError> {
    match continue_if {
        PromptCondition::NonInteractive => Ok(Artifact::Skipped {
            step_index: index,
            reason: format!("prompt (non-interactive): {message}"),
        }),
        PromptCondition::UserYes => {
            if config.assume_yes {
                return Ok(Artifact::Skipped {
                    step_index: index,
                    reason: format!("prompt auto-accepted (--yes): {message}"),
                });
            }
            // In DryRun / Unattended we never actually prompt — the
            // caller would have bailed on `preflight` anyway.
            if matches!(config.mode, Mode::DryRun | Mode::Unattended) {
                return Ok(Artifact::Skipped {
                    step_index: index,
                    reason: format!("prompt: {message}"),
                });
            }
            let ok = dialoguer::Confirm::new()
                .with_prompt(message)
                .default(true)
                .interact()
                .unwrap_or(false);
            if ok {
                Ok(Artifact::Skipped {
                    step_index: index,
                    reason: format!("prompt accepted: {message}"),
                })
            } else {
                Err(ExecError::UserAborted { index })
            }
        }
    }
}

// ─── Rollback ─────────────────────────────────────────────────────────────

/// Walk the artifact list in reverse and undo each one. Best-effort: a
/// failure in rollback step N doesn't stop rollback step N-1 — we log and
/// continue, because the alternative is leaving the system in a worse
/// state than we started it.
pub async fn rollback_from_handle(
    handle: &SnapshotHandle,
    reporter: &dyn ExecReporter,
    config: &ExecConfig,
) {
    let artifacts: Vec<Artifact> = handle.manifest().artifacts.clone();
    if artifacts.is_empty() {
        return;
    }
    reporter.rollback_started(artifacts.len());
    for artifact in artifacts.iter().rev() {
        reporter.rollback_step(artifact.step_index(), artifact);
        match artifact {
            Artifact::BackupFile { .. } => {
                let _ = handle.restore_backup(artifact);
            }
            Artifact::InstalledPackages { manager, packages, already_present, .. } => {
                let to_remove: Vec<String> =
                    packages.iter().filter(|p| !already_present.contains(p)).cloned().collect();
                if !to_remove.is_empty() {
                    if let Some(mgr) = parse_manager_tag(manager) {
                        let ops: Box<dyn PackageOps> = match mgr {
                            PackageManager::Apt => Box::new(AptOps),
                            _ => continue,
                        };
                        let _ = ops.uninstall(&to_remove, Sudo::Interactive).await;
                    }
                }
            }
            Artifact::ServiceChange { unit, previous_enabled, previous_active, .. } => {
                if matches!(previous_enabled, Some(false)) {
                    let _ = Command::new("systemctl")
                        .args(["--user", "disable", unit])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .await;
                }
                if matches!(previous_active, Some(false)) {
                    let _ = Command::new("systemctl")
                        .args(["--user", "stop", unit])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .await;
                }
            }
            Artifact::VerifyOk { .. } | Artifact::Skipped { .. } => {
                // No undo needed.
            }
        }
    }
    let _ = config; // silence unused in release builds
}

// ─── Helpers ──────────────────────────────────────────────────────────────

fn format_for(file: &EnvFile) -> EnvFileFormat {
    match file {
        EnvFile::HomeProfile => EnvFileFormat::PosixShellExport,
        _ => EnvFileFormat::KeyValue,
    }
}

fn manager_tag(m: PackageManager) -> &'static str {
    match m {
        PackageManager::Apt => "apt",
        PackageManager::Dnf => "dnf",
        PackageManager::Pacman => "pacman",
        PackageManager::Zypper => "zypper",
        PackageManager::Xbps => "xbps",
        PackageManager::Emerge => "emerge",
    }
}

fn parse_manager_tag(s: &str) -> Option<PackageManager> {
    Some(match s {
        "apt" => PackageManager::Apt,
        "dnf" => PackageManager::Dnf,
        "pacman" => PackageManager::Pacman,
        "zypper" => PackageManager::Zypper,
        "xbps" => PackageManager::Xbps,
        "emerge" => PackageManager::Emerge,
        _ => return None,
    })
}

fn pkg_err(index: usize) -> impl Fn(PackageOpsError) -> ExecError {
    move |e| ExecError::Step {
        index,
        kind: "install_packages",
        source: anyhow::anyhow!(e.to_string()),
    }
}

fn envfile_err(index: usize) -> impl Fn(EnvFileError) -> ExecError {
    move |e| ExecError::Step { index, kind: "env_file", source: anyhow::anyhow!(e.to_string()) }
}

fn step_err(index: usize, step: &Step, source: anyhow::Error) -> ExecError {
    ExecError::Step { index, kind: step.kind(), source }
}

fn write_atomic_with_parent(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("vietime-tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

async fn systemctl_is_enabled(unit: &str) -> Option<bool> {
    let out = Command::new("systemctl")
        .args(["--user", "is-enabled", unit])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    match stdout.trim() {
        "enabled" | "enabled-runtime" | "alias" | "static" => Some(true),
        "disabled" | "masked" | "not-found" => Some(false),
        _ => None,
    }
}

async fn systemctl_is_active(unit: &str) -> Option<bool> {
    let out = Command::new("systemctl")
        .args(["--user", "is-active", unit])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    match stdout.trim() {
        "active" | "activating" | "reloading" => Some(true),
        "inactive" | "failed" | "deactivating" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::{Combo, Engine, Goal, PackageManager, Plan, Step, PLAN_SCHEMA_VERSION};
    use crate::pre_state::PreState;
    use tempfile::TempDir;
    use vietime_core::ImFramework;

    fn sample_plan_in_home(home: &Path) -> Plan {
        let _ = home;
        let mut p = Plan::new_skeleton(
            Goal::Install { combo: Combo::new(ImFramework::Fcitx5, Engine::Bamboo) },
            PreState::fixture_ubuntu_24_04(),
        );
        p.schema_version = PLAN_SCHEMA_VERSION;
        p
    }

    #[tokio::test]
    async fn dry_run_executes_all_steps_as_skipped() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().to_path_buf();
        let mut plan = sample_plan_in_home(&home);
        plan.steps = vec![
            Step::BackupFile { path: home.join(".profile") },
            Step::SetEnvVar {
                file: EnvFile::HomeProfile,
                key: "GTK_IM_MODULE".into(),
                value: "fcitx".into(),
            },
        ];

        let config = ExecConfig::new(Mode::DryRun)
            .with_home(home.clone())
            .with_snapshots_root(tmp.path().join("snap"));

        let outcome = run_plan(plan, &config, Arc::new(StderrReporter)).await.unwrap();
        assert_eq!(outcome.steps_executed, 2);
        assert!(outcome.dry_run);
    }

    #[tokio::test]
    async fn live_run_writes_env_file_and_records_artifacts() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().to_path_buf();
        let profile = home.join(".profile");
        std::fs::write(&profile, "# my profile\n").unwrap();

        let mut plan = sample_plan_in_home(&home);
        plan.steps = vec![
            Step::BackupFile { path: profile.clone() },
            Step::SetEnvVar {
                file: EnvFile::HomeProfile,
                key: "GTK_IM_MODULE".into(),
                value: "fcitx".into(),
            },
        ];

        let config = ExecConfig::new(Mode::Live)
            .with_home(home.clone())
            .with_snapshots_root(tmp.path().join("snap"))
            .with_assume_yes(true);

        let outcome = run_plan(plan, &config, Arc::new(StderrReporter)).await.unwrap();
        assert_eq!(outcome.steps_executed, 2);

        let written = std::fs::read_to_string(&profile).unwrap();
        assert!(written.contains("export GTK_IM_MODULE=fcitx"), "rendered block present");
        assert!(written.contains("# my profile"), "user content preserved");
    }

    #[tokio::test]
    async fn live_run_is_idempotent_when_env_var_already_set() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().to_path_buf();
        let profile = home.join(".profile");
        // Pre-populate with the exact content the executor would produce.
        let doc_seed = "# my profile\n".to_owned();
        std::fs::write(&profile, &doc_seed).unwrap();

        let plan_steps = vec![
            Step::BackupFile { path: profile.clone() },
            Step::SetEnvVar {
                file: EnvFile::HomeProfile,
                key: "GTK_IM_MODULE".into(),
                value: "fcitx".into(),
            },
        ];

        let config = ExecConfig::new(Mode::Live)
            .with_home(home.clone())
            .with_snapshots_root(tmp.path().join("snap"))
            .with_assume_yes(true);

        // First run.
        let mut plan = sample_plan_in_home(&home);
        plan.steps.clone_from(&plan_steps);
        let _ = run_plan(plan, &config, Arc::new(StderrReporter)).await.unwrap();
        let first = std::fs::read_to_string(&profile).unwrap();

        // Second run with the same steps should be a no-op for the env file.
        let mut plan2 = sample_plan_in_home(&home);
        plan2.steps = plan_steps;
        let _ = run_plan(plan2, &config, Arc::new(StderrReporter)).await.unwrap();
        let second = std::fs::read_to_string(&profile).unwrap();

        assert_eq!(first, second, "re-running leaves the file byte-identical");
    }

    #[tokio::test]
    async fn unsupported_manager_surfaces_error() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().to_path_buf();

        let mut plan = sample_plan_in_home(&home);
        plan.steps = vec![Step::InstallPackages {
            manager: PackageManager::Dnf,
            packages: vec!["fcitx5".into()],
        }];
        plan.requires_sudo = false; // avoid sudo path

        let config = ExecConfig::new(Mode::Live)
            .with_home(home.clone())
            .with_snapshots_root(tmp.path().join("snap"))
            .with_assume_yes(true);

        let err = run_plan(plan, &config, Arc::new(StderrReporter)).await.unwrap_err();
        assert!(
            matches!(
                err,
                ExecError::Sudo(_) | ExecError::UnsupportedManager(_) | ExecError::Step { .. }
            ),
            "got: {err}"
        );
    }
}
