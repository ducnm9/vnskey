// SPDX-License-Identifier: GPL-3.0-or-later
//
// Installer data model — `Plan`, `Step`, `Combo`, `Goal`, and friends.
//
// Every type here is plain data: serializable (TOML + JSON), comparable,
// `Clone`-able. The Planner in `planner.rs` builds `Plan`s from a
// `PreState`; the Executor (shipping in Week 3+) consumes them.
//
// Spec ref: `spec/02-phase2-installer.md` §B.2.

use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use vietime_core::ImFramework;

use crate::pre_state::PreState;

/// Current schema version for a serialized `Plan`. Bumped on every breaking
/// change to the wire format so that older manifests on disk can be rejected
/// or migrated explicitly rather than silently mis-parsed.
pub const PLAN_SCHEMA_VERSION: u32 = 1;

/// Vietnamese input method engine. Installer MVP targets two:
/// `Bamboo` (most common community choice) and `Unikey` (ported from the
/// Windows engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Bamboo,
    Unikey,
}

impl Engine {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bamboo => "bamboo",
            Self::Unikey => "unikey",
        }
    }
}

impl std::fmt::Display for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A framework + engine pair: the unit the Installer reasons about. There
/// are exactly four supported combos in the MVP (spec/02 §A.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Combo {
    pub framework: ImFramework,
    pub engine: Engine,
}

impl Combo {
    #[must_use]
    pub const fn new(framework: ImFramework, engine: Engine) -> Self {
        Self { framework, engine }
    }

    /// The four combos shipped in v0.1 — returned in a stable order so
    /// `list` output and golden tests don't shuffle between runs.
    #[must_use]
    pub fn all_supported() -> Vec<Self> {
        vec![
            Self::new(ImFramework::Fcitx5, Engine::Bamboo),
            Self::new(ImFramework::Fcitx5, Engine::Unikey),
            Self::new(ImFramework::Ibus, Engine::Bamboo),
            Self::new(ImFramework::Ibus, Engine::Unikey),
        ]
    }

    /// Short slug used on the CLI: `fcitx5-bamboo`, `ibus-unikey`, …
    #[must_use]
    pub fn slug(self) -> String {
        let fw = match self.framework {
            ImFramework::Fcitx5 => "fcitx5",
            ImFramework::Ibus => "ibus",
            ImFramework::None => "none",
        };
        format!("{fw}-{}", self.engine.as_str())
    }
}

impl std::fmt::Display for Combo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.slug())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown combo `{0}` — expected one of: {1}")]
pub struct ParseComboError(pub String, pub String);

impl FromStr for Combo {
    type Err = ParseComboError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        for c in Self::all_supported() {
            if c.slug() == normalized {
                return Ok(c);
            }
        }
        let supported =
            Self::all_supported().iter().copied().map(Self::slug).collect::<Vec<_>>().join(", ");
        Err(ParseComboError(s.to_owned(), supported))
    }
}

/// Package manager the current distro uses. MVP wires up `Apt` only; the
/// remaining variants are declared now so `Step::InstallPackages` can be
/// deserialized from old manifests after INS-50/51 ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageManager {
    Apt,
    Dnf,
    Pacman,
    Zypper,
    Xbps,
    Emerge,
}

/// A file whose contents the Installer wants to mutate. `EtcEnvironment` is
/// the canonical Ubuntu path; `ConfigEnvironmentD` is the systemd-native
/// path used on Fedora/Arch (spec/02 §B.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvFile {
    EtcEnvironment,
    HomeProfile,
    ConfigEnvironmentD { filename: String },
    SystemdUserEnv,
    Custom { path: PathBuf },
}

impl EnvFile {
    /// The filesystem path this `EnvFile` resolves to on a real system. The
    /// Planner uses this to pair `SetEnvVar` steps with the matching
    /// `BackupFile`.
    #[must_use]
    pub fn path(&self, home: &std::path::Path) -> PathBuf {
        match self {
            Self::EtcEnvironment => PathBuf::from("/etc/environment"),
            Self::HomeProfile => home.join(".profile"),
            Self::ConfigEnvironmentD { filename } => {
                home.join(".config/environment.d").join(filename)
            }
            Self::SystemdUserEnv => home.join(".config/systemd/user/environment.conf"),
            Self::Custom { path } => path.clone(),
        }
    }
}

/// A verify assertion the Executor runs after the mutating steps complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerifyCheck {
    DaemonRunning { framework: ImFramework },
    EngineRegistered { name: String },
    EnvConsistent,
    DoctorCheckPasses,
}

/// Gating rule for a `Step::Prompt` — when the prompt is allowed to continue
/// without user interaction. `NonInteractive` is used with `--yes` to skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptCondition {
    UserYes,
    NonInteractive,
}

/// The atomic unit of work the Executor runs. Each Step is side-effect-free
/// as data; the Executor turns it into filesystem/process actions and
/// records an artifact in the snapshot manifest before it mutates anything.
///
/// Step variants use an internally-tagged `kind` discriminator so TOML /
/// JSON round-trips look like:
/// ```toml
/// [[steps]]
/// kind = "backup_file"
/// path = "/etc/environment"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    BackupFile { path: PathBuf },
    InstallPackages { manager: PackageManager, packages: Vec<String> },
    UninstallPackages { manager: PackageManager, packages: Vec<String> },
    SetEnvVar { file: EnvFile, key: String, value: String },
    UnsetEnvVar { file: EnvFile, key: String },
    SystemctlUserEnable { unit: String },
    SystemctlUserDisable { unit: String },
    SystemctlUserStart { unit: String },
    SystemctlUserStop { unit: String },
    RunImConfig { mode: String },
    WriteFile { path: PathBuf, content: String, mode: u32 },
    Verify { check: VerifyCheck },
    Prompt { message: String, continue_if: PromptCondition },
}

impl Step {
    /// Short, stable tag used in error messages so the invariant checker can
    /// say "Step 7 (set_env_var) has no backup".
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::BackupFile { .. } => "backup_file",
            Self::InstallPackages { .. } => "install_packages",
            Self::UninstallPackages { .. } => "uninstall_packages",
            Self::SetEnvVar { .. } => "set_env_var",
            Self::UnsetEnvVar { .. } => "unset_env_var",
            Self::SystemctlUserEnable { .. } => "systemctl_user_enable",
            Self::SystemctlUserDisable { .. } => "systemctl_user_disable",
            Self::SystemctlUserStart { .. } => "systemctl_user_start",
            Self::SystemctlUserStop { .. } => "systemctl_user_stop",
            Self::RunImConfig { .. } => "run_im_config",
            Self::WriteFile { .. } => "write_file",
            Self::Verify { .. } => "verify",
            Self::Prompt { .. } => "prompt",
        }
    }
}

/// The high-level intent the user gave us. `Install` is the main path for
/// v0.1; `Uninstall` / `Switch` bodies arrive in Week 5+.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Goal {
    Install { combo: Combo },
    Uninstall { snapshot_id: Option<String> },
    Switch { from: Combo, to: Combo },
}

/// The Plan is the full wire-serializable description of an Installer run:
/// which steps will happen, in which order, and against which pre-state.
///
/// The executor writes the whole Plan into `manifest.toml` inside the
/// snapshot directory before the first mutating step runs, so a later
/// `rollback` session can replay it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// Always written as `PLAN_SCHEMA_VERSION`.
    pub schema_version: u32,
    pub id: Uuid,
    pub goal: Goal,
    pub generated_at: DateTime<Utc>,
    pub pre_state: PreState,
    pub steps: Vec<Step>,
    /// Duration in whole seconds — `std::time::Duration` isn't TOML-friendly.
    pub estimated_duration_secs: u64,
    pub requires_sudo: bool,
    pub requires_logout: bool,
}

impl Plan {
    /// Build an empty Plan skeleton with the schema version + timestamp
    /// pre-filled. The Planner populates everything else.
    #[must_use]
    pub fn new_skeleton(goal: Goal, pre_state: PreState) -> Self {
        Self {
            schema_version: PLAN_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            goal,
            generated_at: Utc::now(),
            pre_state,
            steps: Vec::new(),
            estimated_duration_secs: 0,
            requires_sudo: false,
            requires_logout: false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn fixture_pre_state() -> PreState {
        PreState::fixture_ubuntu_24_04()
    }

    #[test]
    fn combo_all_supported_has_four_unique() {
        let all = Combo::all_supported();
        assert_eq!(all.len(), 4);
        let uniq: std::collections::HashSet<_> = all.iter().copied().collect();
        assert_eq!(uniq.len(), 4, "all_supported() must not repeat");
    }

    #[test]
    fn combo_slug_display_roundtrip() {
        for c in Combo::all_supported() {
            let slug = c.slug();
            let parsed: Combo = slug.parse().expect("valid slug parses back");
            assert_eq!(parsed, c);
            assert_eq!(format!("{c}"), slug);
        }
    }

    #[test]
    fn combo_rejects_unknown_slug() {
        let err = Combo::from_str("fcitx5-telex").expect_err("should reject");
        let msg = err.to_string();
        assert!(msg.contains("fcitx5-telex"), "error mentions input: {msg}");
        assert!(msg.contains("fcitx5-bamboo"), "error lists supported: {msg}");
    }

    #[test]
    fn combo_parse_is_case_insensitive_and_trims() {
        assert_eq!(
            Combo::from_str("  FCITX5-BAMBOO  ").unwrap(),
            Combo::new(ImFramework::Fcitx5, Engine::Bamboo)
        );
    }

    #[test]
    fn plan_roundtrips_through_toml() {
        let plan = Plan {
            schema_version: PLAN_SCHEMA_VERSION,
            id: Uuid::nil(),
            goal: Goal::Install { combo: Combo::new(ImFramework::Fcitx5, Engine::Bamboo) },
            generated_at: Utc::now(),
            pre_state: fixture_pre_state(),
            steps: vec![
                Step::BackupFile { path: "/etc/environment".into() },
                Step::InstallPackages {
                    manager: PackageManager::Apt,
                    packages: vec!["fcitx5".to_owned(), "fcitx5-bamboo".to_owned()],
                },
                Step::SetEnvVar {
                    file: EnvFile::EtcEnvironment,
                    key: "GTK_IM_MODULE".to_owned(),
                    value: "fcitx".to_owned(),
                },
                Step::SystemctlUserEnable { unit: "fcitx5.service".to_owned() },
                Step::Verify {
                    check: VerifyCheck::DaemonRunning { framework: ImFramework::Fcitx5 },
                },
                Step::Prompt {
                    message: "Logout and log back in.".to_owned(),
                    continue_if: PromptCondition::UserYes,
                },
            ],
            estimated_duration_secs: 45,
            requires_sudo: true,
            requires_logout: true,
        };

        let serialized = toml::to_string(&plan).expect("toml serialize");
        let back: Plan = toml::from_str(&serialized).expect("toml deserialize");
        assert_eq!(plan, back);
    }

    #[test]
    fn plan_roundtrips_through_json() {
        let plan = Plan::new_skeleton(
            Goal::Install { combo: Combo::new(ImFramework::Ibus, Engine::Unikey) },
            fixture_pre_state(),
        );
        let s = serde_json::to_string_pretty(&plan).expect("json serialize");
        let back: Plan = serde_json::from_str(&s).expect("json deserialize");
        assert_eq!(plan, back);
    }

    #[test]
    fn env_file_resolves_paths_against_home() {
        let home = std::path::Path::new("/home/test");
        assert_eq!(
            EnvFile::EtcEnvironment.path(home),
            std::path::PathBuf::from("/etc/environment")
        );
        assert_eq!(
            EnvFile::HomeProfile.path(home),
            std::path::PathBuf::from("/home/test/.profile")
        );
        assert_eq!(
            EnvFile::ConfigEnvironmentD { filename: "90-vietime.conf".into() }.path(home),
            std::path::PathBuf::from("/home/test/.config/environment.d/90-vietime.conf")
        );
    }

    #[test]
    fn step_kind_is_stable() {
        assert_eq!(Step::BackupFile { path: "/tmp/foo".into() }.kind(), "backup_file");
        assert_eq!(Step::Verify { check: VerifyCheck::EnvConsistent }.kind(), "verify");
    }
}
