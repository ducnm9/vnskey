// SPDX-License-Identifier: GPL-3.0-or-later
//
// Snapshot store — the on-disk record of everything one Installer run did.
//
// Every mutating `Step` writes an `Artifact` into the snapshot *before* it
// touches system state, so a crash or user Ctrl+C in the middle of a run
// always leaves a recoverable trail. `rollback` reads the manifest back
// and walks the artifact list in reverse.
//
// ## Layout
//
// ```text
// $ROOT/                                    # default: ~/.config/vietime/snapshots/
// ├── 2026-04-26T12-34-56Z/                 # one run
// │   ├── manifest.toml                     # serialized `Manifest`
// │   ├── files/                            # backed-up originals + sha256 sidecars
// │   │   ├── etc_environment.bak
// │   │   ├── etc_environment.bak.sha256
// │   │   └── home_profile.bak
// │   ├── packages-installed.txt            # what InstallPackages added
// │   └── services-changed.json             # previous systemd state
// └── latest -> 2026-04-26T12-34-56Z        # atomic symlink update
// ```
//
// ## `incomplete` flag
//
// The manifest is written with `incomplete = true` at the start of a run
// and flipped to `false` on clean exit. A stray `true` on a subsequent
// run is the signal that we were SIGKILL'd last time — `rollback` in
// "force" mode handles that case (INS-41).
//
// Spec ref: `spec/02-phase2-installer.md` §B.5.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::{Goal, Plan, Step};

/// Schema version stamped into every `manifest.toml`. Bumped on any
/// breaking change to the artifact shape.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Timestamp format used for snapshot directory names. ISO 8601 with
/// colons replaced by dashes so the name is filesystem-safe on every
/// target OS.
const TS_FORMAT: &str = "%Y-%m-%dT%H-%M-%SZ";

/// All failure modes the snapshot store can emit. Using `thiserror` lets
/// the CLI pattern-match on specific cases (e.g. `DiskFull`) for tailored
/// user messages.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("snapshot root `{path}` could not be created: {source}")]
    CreateRoot { path: PathBuf, source: std::io::Error },

    #[error("backup source `{path}` could not be read: {source}")]
    ReadSource { path: PathBuf, source: std::io::Error },

    #[error("backup destination `{path}` could not be written: {source}")]
    WriteBackup { path: PathBuf, source: std::io::Error },

    #[error("manifest `{path}` could not be written: {source}")]
    WriteManifest { path: PathBuf, source: std::io::Error },

    #[error("manifest `{path}` could not be serialised: {source}")]
    SerialiseManifest { path: PathBuf, source: toml::ser::Error },

    #[error("manifest `{path}` could not be parsed: {source}")]
    ReadManifest { path: PathBuf, source: anyhow::Error },

    #[error("`latest` symlink at `{path}` could not be updated: {source}")]
    Symlink { path: PathBuf, source: std::io::Error },

    #[error("snapshot `{id}` not found under `{root}`")]
    NotFound { id: String, root: PathBuf },

    #[error(
        "insufficient disk space at `{root}`: need {need_bytes} bytes, \
         have {free_bytes}. Free up some space and retry."
    )]
    DiskFull { root: PathBuf, need_bytes: u64, free_bytes: u64 },
}

/// A side-effect we performed (or intend to perform) as part of a Plan.
///
/// Each `Artifact` is written **before** the Executor actually mutates
/// anything: backup the file → record the artifact → mutate. That way a
/// crash between "record" and "mutate" leaves a recoverable
/// (no-op-rollback) artifact rather than unrecorded damage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Artifact {
    /// A file was copied into `files/<backup_name>`. `existed_before`
    /// distinguishes "we backed up your old config" from "there was no
    /// file, so rollback means: delete any file we created here".
    BackupFile {
        step_index: usize,
        original_path: PathBuf,
        backup_relpath: PathBuf,
        sha256: String,
        existed_before: bool,
    },
    /// Packages we added via the distro package manager. Rollback removes
    /// exactly these and nothing else.
    InstalledPackages {
        step_index: usize,
        manager: String,
        packages: Vec<String>,
        /// Sub-list of `packages` that were already installed before the
        /// run. Rollback must NOT uninstall these.
        already_present: Vec<String>,
    },
    /// systemd user unit state before we enabled/disabled it. Rollback
    /// sets it back.
    ServiceChange {
        step_index: usize,
        unit: String,
        previous_enabled: Option<bool>,
        previous_active: Option<bool>,
    },
    /// Successful verification marker — not a rollback target, but useful
    /// for debugging.
    VerifyOk { step_index: usize, summary: String },
    /// A no-op (idempotent skip). Recorded so the audit trail doesn't
    /// have holes.
    Skipped { step_index: usize, reason: String },
}

impl Artifact {
    #[must_use]
    pub fn step_index(&self) -> usize {
        match self {
            Self::BackupFile { step_index, .. }
            | Self::InstalledPackages { step_index, .. }
            | Self::ServiceChange { step_index, .. }
            | Self::VerifyOk { step_index, .. }
            | Self::Skipped { step_index, .. } => *step_index,
        }
    }
}

/// Full manifest written to `manifest.toml`. The Plan is embedded so a
/// rollback session doesn't need the original command-line inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub created_at: DateTime<Utc>,
    /// `true` until the last step completes. A stray `true` on the next
    /// run means the previous invocation was SIGKILL'd and needs
    /// `rollback --force`.
    pub incomplete: bool,
    pub plan: Plan,
    pub artifacts: Vec<Artifact>,
}

impl Manifest {
    #[must_use]
    pub fn new(plan: Plan, id: String) -> Self {
        Self {
            schema_version: MANIFEST_SCHEMA_VERSION,
            id,
            created_at: Utc::now(),
            incomplete: true,
            plan,
            artifacts: Vec::new(),
        }
    }

    /// High-level human description of the goal — used by `snapshots`.
    #[must_use]
    pub fn goal_summary(&self) -> String {
        match &self.plan.goal {
            Goal::Install { combo } => format!("install {combo}"),
            Goal::Uninstall { snapshot_id: Some(id) } => format!("uninstall (of {id})"),
            Goal::Uninstall { snapshot_id: None } => "uninstall".to_owned(),
            Goal::Switch { from, to } => format!("switch {from} → {to}"),
        }
    }
}

/// Lightweight row used by `SnapshotStore::list`. Avoids loading every
/// manifest's full Plan when the caller just wants the table view.
#[derive(Debug, Clone)]
pub struct SnapshotMeta {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub goal: String,
    pub incomplete: bool,
}

/// On-disk snapshot store. One `SnapshotStore` manages one root
/// directory — typically `~/.config/vietime/snapshots`, but any path
/// works (tests use `tempdir`).
#[derive(Debug, Clone)]
pub struct SnapshotStore {
    root: PathBuf,
}

impl SnapshotStore {
    /// Construct a store over `root`. The directory is created lazily on
    /// first write, NOT here — callers can cheaply build a `SnapshotStore`
    /// in `main` without touching the filesystem.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The default user-level store location: `$XDG_CONFIG_HOME/vietime/
    /// snapshots` or `~/.config/vietime/snapshots`.
    #[must_use]
    pub fn default_for_user(home: &Path) -> Self {
        let config =
            std::env::var_os("XDG_CONFIG_HOME").map_or_else(|| home.join(".config"), PathBuf::from);
        Self::new(config.join("vietime").join("snapshots"))
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ensure the root directory exists.
    pub fn ensure_root(&self) -> Result<(), SnapshotError> {
        fs::create_dir_all(&self.root)
            .map_err(|source| SnapshotError::CreateRoot { path: self.root.clone(), source })
    }

    /// Refuse to proceed if `root` has less than `need_bytes` free. The
    /// INS-42 disk-space pre-check — called from `ExecContext::begin`
    /// before we back anything up.
    pub fn check_disk_space(&self, need_bytes: u64) -> Result<(), SnapshotError> {
        self.ensure_root()?;
        let free = free_bytes(&self.root).unwrap_or(u64::MAX);
        if free < need_bytes {
            return Err(SnapshotError::DiskFull {
                root: self.root.clone(),
                need_bytes,
                free_bytes: free,
            });
        }
        Ok(())
    }

    /// Begin a new snapshot from `plan`. Creates the directory and the
    /// `files/` subdir, but doesn't write the manifest yet — callers
    /// `commit` artifacts via `record` and `save_manifest`.
    pub fn begin(&self, plan: Plan) -> Result<SnapshotHandle, SnapshotError> {
        self.ensure_root()?;
        let id = Utc::now().format(TS_FORMAT).to_string();
        let dir = self.root.join(&id);
        fs::create_dir_all(dir.join("files"))
            .map_err(|source| SnapshotError::CreateRoot { path: dir.clone(), source })?;
        let manifest = Manifest::new(plan, id.clone());

        let handle = SnapshotHandle { dir, manifest };
        handle.save_manifest()?; // writes with incomplete=true
        Ok(handle)
    }

    /// Atomically update the `latest` symlink to point at `id`. Used on
    /// successful completion so `rollback` with no args finds the most
    /// recent snapshot.
    pub fn update_latest(&self, id: &str) -> Result<(), SnapshotError> {
        let link = self.root.join("latest");
        // Best-effort cleanup of any prior link or file.
        let _ = fs::remove_file(&link);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(id, &link)
                .map_err(|source| SnapshotError::Symlink { path: link, source })
        }
        #[cfg(not(unix))]
        {
            // On non-Unix we fall back to a plain-text pointer. The
            // Installer only runs on Linux so this branch exists purely
            // to keep cargo-check green on macOS dev machines.
            fs::write(&link, id).map_err(|source| SnapshotError::Symlink { path: link, source })
        }
    }

    /// Resolve the `latest` pointer into a snapshot id.
    pub fn latest_id(&self) -> Option<String> {
        let link = self.root.join("latest");
        #[cfg(unix)]
        {
            fs::read_link(&link)
                .ok()
                .and_then(|p| p.file_name().and_then(|s| s.to_os_string().into_string().ok()))
        }
        #[cfg(not(unix))]
        {
            fs::read_to_string(&link).ok().map(|s| s.trim().to_owned())
        }
    }

    /// Load a previously-committed snapshot by id.
    pub fn load(&self, id: &str) -> Result<SnapshotHandle, SnapshotError> {
        let dir = self.root.join(id);
        if !dir.exists() {
            return Err(SnapshotError::NotFound { id: id.to_owned(), root: self.root.clone() });
        }
        let manifest_path = dir.join("manifest.toml");
        let text = fs::read_to_string(&manifest_path).map_err(|source| {
            SnapshotError::ReadManifest { path: manifest_path.clone(), source: source.into() }
        })?;
        let manifest: Manifest = toml::from_str(&text).map_err(|source| {
            SnapshotError::ReadManifest { path: manifest_path, source: source.into() }
        })?;
        Ok(SnapshotHandle { dir, manifest })
    }

    /// List every snapshot in the root, newest first. Silently skips
    /// entries that fail to parse — the caller gets a partial view
    /// rather than a hard error mid-listing.
    pub fn list(&self) -> Result<Vec<SnapshotMeta>, SnapshotError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.root)
            .map_err(|source| SnapshotError::CreateRoot { path: self.root.clone(), source })?
        {
            let Ok(entry) = entry else { continue };
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let Ok(id) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(handle) = self.load(&id) else { continue };
            out.push(SnapshotMeta {
                id: handle.manifest.id.clone(),
                created_at: handle.manifest.created_at,
                goal: handle.manifest.goal_summary(),
                incomplete: handle.manifest.incomplete,
            });
        }
        out.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(out)
    }
}

/// Live handle to a single snapshot directory. Returned by
/// `SnapshotStore::begin` / `::load`. Mutating methods update the
/// in-memory manifest; `save_manifest` persists.
#[derive(Debug)]
pub struct SnapshotHandle {
    dir: PathBuf,
    manifest: Manifest,
}

impl SnapshotHandle {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Append an artifact to the manifest (in memory). Callers invoke
    /// `save_manifest` to persist — typically after each mutating step so
    /// a crash leaves an up-to-date trail.
    pub fn record(&mut self, artifact: Artifact) {
        self.manifest.artifacts.push(artifact);
    }

    /// Mark the run complete. Flips `incomplete` to `false` and persists.
    pub fn finalise(&mut self) -> Result<(), SnapshotError> {
        self.manifest.incomplete = false;
        self.save_manifest()
    }

    /// Persist `manifest.toml` atomically: write to `manifest.toml.tmp`,
    /// fsync, rename. Callers who want the trail to survive a SIGKILL
    /// should call this after every artifact.
    pub fn save_manifest(&self) -> Result<(), SnapshotError> {
        let final_path = self.dir.join("manifest.toml");
        let tmp_path = self.dir.join("manifest.toml.tmp");
        let serialised = toml::to_string_pretty(&self.manifest).map_err(|source| {
            SnapshotError::SerialiseManifest { path: final_path.clone(), source }
        })?;
        {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|source| SnapshotError::WriteManifest {
                    path: tmp_path.clone(),
                    source,
                })?;
            file.write_all(serialised.as_bytes()).map_err(|source| {
                SnapshotError::WriteManifest { path: tmp_path.clone(), source }
            })?;
            file.sync_all().map_err(|source| SnapshotError::WriteManifest {
                path: tmp_path.clone(),
                source,
            })?;
        }
        fs::rename(&tmp_path, &final_path)
            .map_err(|source| SnapshotError::WriteManifest { path: final_path, source })
    }

    /// Back up `source` into `files/<sanitised>.bak` with a `.sha256`
    /// sidecar, and return the recorded `Artifact`. Idempotent: if the
    /// backup already exists and its sha matches, the old file is
    /// re-used and no disk write happens.
    pub fn backup_file(
        &mut self,
        step_index: usize,
        source: &Path,
    ) -> Result<Artifact, SnapshotError> {
        let backup_name = sanitise_path_for_backup(source);
        let backup_relpath = PathBuf::from("files").join(&backup_name);
        let backup_abs = self.dir.join(&backup_relpath);

        match fs::read(source) {
            Ok(bytes) => {
                let sha = sha256_hex(&bytes);
                // Idempotency: if we've already backed up the same bytes,
                // leave the existing file untouched.
                let must_write = match fs::read(&backup_abs) {
                    Ok(existing) => sha256_hex(&existing) != sha,
                    Err(_) => true,
                };
                if must_write {
                    write_atomic(&backup_abs, &bytes)?;
                    write_atomic(
                        &self.dir.join(format!("{}.sha256", backup_relpath.display())),
                        format!("{sha}  {backup_name}\n").as_bytes(),
                    )?;
                }
                Ok(Artifact::BackupFile {
                    step_index,
                    original_path: source.to_path_buf(),
                    backup_relpath,
                    sha256: sha,
                    existed_before: true,
                })
            }
            // File didn't exist → record that fact so rollback knows to
            // remove the file we'll create, rather than to restore.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Artifact::BackupFile {
                step_index,
                original_path: source.to_path_buf(),
                backup_relpath: PathBuf::new(),
                sha256: String::new(),
                existed_before: false,
            }),
            Err(err) => Err(SnapshotError::ReadSource { path: source.to_path_buf(), source: err }),
        }
    }

    /// Restore a single `BackupFile` artifact onto the live filesystem.
    /// Rollback driver.
    pub fn restore_backup(&self, artifact: &Artifact) -> Result<(), SnapshotError> {
        let Artifact::BackupFile { original_path, backup_relpath, sha256, existed_before, .. } =
            artifact
        else {
            return Ok(());
        };
        if !existed_before {
            // Nothing existed before → rollback deletes whatever we may
            // have created. Missing file is not an error.
            let _ = fs::remove_file(original_path);
            return Ok(());
        }
        let backup_abs = self.dir.join(backup_relpath);
        let bytes = fs::read(&backup_abs)
            .map_err(|source| SnapshotError::ReadSource { path: backup_abs.clone(), source })?;
        if !sha256.is_empty() {
            let got = sha256_hex(&bytes);
            if &got != sha256 {
                return Err(SnapshotError::ReadSource {
                    path: backup_abs,
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("backup sha256 mismatch: expected {sha256}, got {got}"),
                    ),
                });
            }
        }
        write_atomic(original_path, &bytes)
    }

    /// Pair a `Step` with its `Artifact` on rollback. Needed because
    /// artifacts don't carry the full `Step` — the manifest stores both
    /// separately and they line up by `step_index`.
    #[must_use]
    pub fn find_step(&self, step_index: usize) -> Option<&Step> {
        self.manifest.plan.steps.get(step_index)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────

/// Hash bytes with SHA-256 and return the lowercase hex digest.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Turn an absolute system path into a backup-friendly filename. `/etc/
/// environment` → `etc_environment.bak`. Replaces separators with `_`
/// and strips a leading `/`.
fn sanitise_path_for_backup(p: &Path) -> String {
    let s = p.display().to_string();
    let trimmed = s.trim_start_matches('/');
    let mut out = String::with_capacity(trimmed.len() + 4);
    for c in trimmed.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out.push_str(".bak");
    out
}

/// Atomic file write: `path.tmp` → fsync → rename. Same idiom as
/// `save_manifest`, lifted out because `backup_file` uses it too.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), SnapshotError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| SnapshotError::WriteBackup { path: parent.to_path_buf(), source })?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|source| SnapshotError::WriteBackup { path: tmp.clone(), source })?;
        file.write_all(bytes)
            .map_err(|source| SnapshotError::WriteBackup { path: tmp.clone(), source })?;
        file.sync_all()
            .map_err(|source| SnapshotError::WriteBackup { path: tmp.clone(), source })?;
    }
    fs::rename(&tmp, path)
        .map_err(|source| SnapshotError::WriteBackup { path: path.to_path_buf(), source })
}

/// Return the number of free bytes on the filesystem backing `root`, or
/// `None` when we can't cheaply tell. Callers treat `None` as "don't
/// know, proceed".
///
/// Implementation note: the workspace forbids `unsafe_code`, which rules
/// out a direct `libc::statvfs` FFI call. We shell out to `/bin/df` and
/// parse the `Available` column instead — same figure, one `fork+exec`
/// amortised across an entire install run. If `df` is missing (exotic
/// embedded systems, busybox without `df`) we fall back to `None` and
/// the disk-space pre-check becomes a no-op.
fn free_bytes(root: &Path) -> Option<u64> {
    // Keep the syscall/exec cost out of unit tests — they pass
    // `need_bytes: 1024` which trivially succeeds even on full disks.
    use std::process::Command;
    let output = Command::new("df").args(["-Pk", "--"]).arg(root).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // Skip the header line ("Filesystem  1024-blocks Used Available …").
    let data = text.lines().nth(1)?;
    // Fields: [Filesystem, 1K-blocks, Used, Available, Capacity, Mountpoint]
    let avail_kb: u64 = data.split_whitespace().nth(3)?.parse().ok()?;
    avail_kb.checked_mul(1024)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::{Combo, Engine, Plan, PLAN_SCHEMA_VERSION};
    use crate::pre_state::PreState;
    use tempfile::TempDir;
    use vietime_core::ImFramework;

    fn sample_plan() -> Plan {
        let pre = PreState::fixture_ubuntu_24_04();
        let goal = Goal::Install { combo: Combo::new(ImFramework::Fcitx5, Engine::Bamboo) };
        let mut p = Plan::new_skeleton(goal, pre);
        p.schema_version = PLAN_SCHEMA_VERSION;
        p.steps.push(Step::BackupFile { path: PathBuf::from("/etc/environment") });
        p
    }

    #[test]
    fn begin_creates_directory_and_writes_incomplete_manifest() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        let handle = store.begin(sample_plan()).unwrap();

        assert!(handle.dir().exists());
        assert!(handle.dir().join("files").is_dir());
        let on_disk = fs::read_to_string(handle.dir().join("manifest.toml")).unwrap();
        assert!(on_disk.contains("incomplete = true"), "fresh manifest must be marked incomplete");
    }

    #[test]
    fn finalise_flips_incomplete_to_false() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        let mut handle = store.begin(sample_plan()).unwrap();

        handle.finalise().unwrap();
        let on_disk = fs::read_to_string(handle.dir().join("manifest.toml")).unwrap();
        assert!(on_disk.contains("incomplete = false"));
    }

    #[test]
    fn backup_file_records_sha_and_copies_bytes() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        let mut handle = store.begin(sample_plan()).unwrap();

        let src = tmp.path().join("fake_environment");
        fs::write(&src, b"FOO=bar\nGTK_IM_MODULE=ibus\n").unwrap();

        let artifact = handle.backup_file(0, &src).unwrap();
        let Artifact::BackupFile { sha256, existed_before, backup_relpath, .. } = &artifact else {
            panic!("expected BackupFile artifact");
        };
        assert!(existed_before);
        assert_eq!(
            sha256,
            &sha256_hex(b"FOO=bar\nGTK_IM_MODULE=ibus\n"),
            "sha256 must match source bytes"
        );

        let backup_abs = handle.dir().join(backup_relpath);
        let on_disk = fs::read(&backup_abs).unwrap();
        assert_eq!(on_disk, b"FOO=bar\nGTK_IM_MODULE=ibus\n");
    }

    #[test]
    fn backup_file_handles_missing_source_as_did_not_exist() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        let mut handle = store.begin(sample_plan()).unwrap();

        let artifact = handle.backup_file(0, Path::new("/nonexistent/nope")).unwrap();
        let Artifact::BackupFile { existed_before, .. } = &artifact else {
            panic!("expected BackupFile artifact");
        };
        assert!(!existed_before);
    }

    #[test]
    fn backup_is_idempotent_on_matching_sha() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        let mut handle = store.begin(sample_plan()).unwrap();

        let src = tmp.path().join("fake_environment");
        fs::write(&src, b"same bytes").unwrap();
        let a1 = handle.backup_file(0, &src).unwrap();
        let a2 = handle.backup_file(0, &src).unwrap();
        assert_eq!(a1, a2, "second call should return identical artifact");
    }

    #[test]
    fn restore_backup_overwrites_source_with_saved_bytes() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        let mut handle = store.begin(sample_plan()).unwrap();

        let src = tmp.path().join("fake_environment");
        fs::write(&src, b"original").unwrap();
        let artifact = handle.backup_file(0, &src).unwrap();

        // Simulate the install step mutating the file.
        fs::write(&src, b"mutated by installer").unwrap();

        handle.restore_backup(&artifact).unwrap();
        let restored = fs::read(&src).unwrap();
        assert_eq!(restored, b"original");
    }

    #[test]
    fn restore_backup_removes_file_that_did_not_exist_before() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        let mut handle = store.begin(sample_plan()).unwrap();

        let target = tmp.path().join("created_by_install");
        let artifact = handle.backup_file(0, &target).unwrap();

        // Simulate the install step creating the file.
        fs::write(&target, b"new file").unwrap();

        handle.restore_backup(&artifact).unwrap();
        assert!(!target.exists(), "rollback must delete files that did not exist pre-install");
    }

    #[test]
    fn update_latest_symlinks_to_id() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        let handle = store.begin(sample_plan()).unwrap();
        let id = handle.id().to_owned();

        store.update_latest(&id).unwrap();
        assert_eq!(store.latest_id().as_deref(), Some(id.as_str()));
    }

    #[test]
    fn list_returns_snapshots_newest_first() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        let first = store.begin(sample_plan()).unwrap();
        // Tiny sleep so the timestamp-derived IDs differ.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let second = store.begin(sample_plan()).unwrap();

        let rows = store.list().unwrap();
        assert!(rows.len() >= 2);
        assert_eq!(rows[0].id, second.id(), "newest first");
        assert_eq!(rows[1].id, first.id());
    }

    #[test]
    fn list_on_empty_root_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().join("does-not-exist"));
        let rows = store.list().unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn sanitise_path_produces_backup_friendly_name() {
        assert_eq!(sanitise_path_for_backup(Path::new("/etc/environment")), "etc_environment.bak");
        assert_eq!(
            sanitise_path_for_backup(Path::new("/home/alice/.config/fcitx5/profile")),
            "home_alice_.config_fcitx5_profile.bak"
        );
    }

    #[test]
    fn manifest_roundtrips_through_toml() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        let mut handle = store.begin(sample_plan()).unwrap();
        handle.record(Artifact::InstalledPackages {
            step_index: 3,
            manager: "apt".to_owned(),
            packages: vec!["fcitx5".to_owned(), "fcitx5-bamboo".to_owned()],
            already_present: vec![],
        });
        handle.save_manifest().unwrap();

        let reloaded = store.load(handle.id()).unwrap();
        assert_eq!(reloaded.manifest().artifacts.len(), 1);
        let Artifact::InstalledPackages { packages, .. } = &reloaded.manifest().artifacts[0] else {
            panic!("expected InstalledPackages artifact");
        };
        assert_eq!(packages, &vec!["fcitx5".to_owned(), "fcitx5-bamboo".to_owned()]);
    }

    #[test]
    fn check_disk_space_passes_for_tiny_budget() {
        let tmp = TempDir::new().unwrap();
        let store = SnapshotStore::new(tmp.path().to_path_buf());
        assert!(store.check_disk_space(1024).is_ok());
    }
}
