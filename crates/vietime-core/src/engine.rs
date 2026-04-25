// SPDX-License-Identifier: GPL-3.0-or-later
//
// Input-method engine types.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.2.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::im_framework::ImFramework;

/// A single IME engine that the system can enumerate.
///
/// One `EngineFact` = one (framework, engine-name) pair. The same engine
/// may appear twice if it is registered in both IBus and Fcitx5 (uncommon
/// but legal); consumers should treat the list as a set keyed on
/// `(framework, name)`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineFact {
    /// Name as reported by the framework (e.g. `"Bamboo"`, `"bamboo"`,
    /// `"vietnamese-telex"`). Case preserved from the source because
    /// IBus is case-sensitive here.
    pub name: String,
    /// Debian / RPM / Arch package name if we were able to map it.
    /// Absent when the engine ships via a tarball install or is unregistered.
    pub package: Option<String>,
    /// Engine version if the package manager reported one.
    pub version: Option<String>,
    /// Which IM framework this entry belongs to.
    pub framework: ImFramework,
    /// `true` if the engine supports Vietnamese input. We populate this by
    /// matching against a small hardcoded allow-list (`bamboo*`, `unikey*`,
    /// `vietnamese-*`) in the detector.
    pub is_vietnamese: bool,
    /// `true` if the engine appears in `ibus list-engine` / Fcitx5 profile.
    /// An installed-but-not-registered engine is a classic user pitfall
    /// (see checker VD005).
    pub is_registered: bool,
}

/// Facts collected about IBus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IbusFacts {
    pub version: Option<String>,
    pub daemon_running: bool,
    pub daemon_pid: Option<u32>,
    /// Typically `~/.config/ibus/`. `None` when the detector can't resolve
    /// `$HOME`.
    pub config_dir: Option<PathBuf>,
    /// Engines listed by `ibus list-engine`.
    pub registered_engines: Vec<String>,
}

/// Facts collected about Fcitx5.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fcitx5Facts {
    pub version: Option<String>,
    pub daemon_running: bool,
    pub daemon_pid: Option<u32>,
    /// Typically `~/.config/fcitx5/`.
    pub config_dir: Option<PathBuf>,
    /// Enabled addons discovered from `~/.local/share/fcitx5/addon/` or the
    /// per-addon `.conf` files.
    pub addons_enabled: Vec<String>,
    /// Input methods from the `profile` file (e.g. `["keyboard-us", "bamboo"]`).
    pub input_methods_configured: Vec<String>,
}

/// Classifies how a given application is packaged, since this drives how
/// (and whether) it receives IM input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppKind {
    Native,
    Electron,
    Chromium,
    Jvm,
    Flatpak { sandbox_id: String },
    Snap { name: String },
    AppImage,
}

/// App-specific diagnostic facts, filled in only when `--app <X>` is passed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppFacts {
    pub app_id: String,
    pub binary_path: PathBuf,
    pub version: Option<String>,
    pub kind: AppKind,
    pub electron_version: Option<String>,
    pub uses_wayland: Option<bool>,
    /// Free-form notes captured during detection (e.g. "detected Ozone flags").
    pub detector_notes: Vec<String>,
}
