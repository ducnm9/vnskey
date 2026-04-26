// SPDX-License-Identifier: GPL-3.0-or-later
//
// Profile definitions (BEN-42) + matrix orchestrator (BEN-41).
//
// A profile describes which combos (engine × app × session × mode) to run
// and optionally filters test vectors by tag.
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.6, §B.9.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::InputMode;
use crate::runner::RunCombo;
use crate::session::SessionType;

/// A bench profile loaded from `profiles/<name>.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub engines: Vec<String>,
    pub apps: Vec<String>,
    pub sessions: Vec<String>,
    #[serde(default = "default_modes")]
    pub modes: Vec<String>,
    #[serde(default)]
    pub vector_tags: Vec<String>,
}

fn default_modes() -> Vec<String> {
    vec!["telex".to_owned()]
}

impl Profile {
    /// Expand a profile into a list of `RunCombo`s (the Cartesian product).
    #[must_use]
    pub fn expand_combos(&self) -> Vec<RunCombo> {
        let mut combos = Vec::new();
        for engine in &self.engines {
            for app in &self.apps {
                for session_str in &self.sessions {
                    let session_type = match session_str.as_str() {
                        "wayland" => SessionType::Wayland,
                        _ => SessionType::X11,
                    };
                    for mode_str in &self.modes {
                        let mode = mode_str.parse::<InputMode>().unwrap_or(InputMode::Telex);
                        combos.push(RunCombo {
                            engine: engine.clone(),
                            app_id: app.clone(),
                            session_type,
                            mode,
                        });
                    }
                }
            }
        }
        combos
    }
}

/// Load a profile from a TOML file.
pub fn load_profile(path: &Path) -> Result<Profile, ProfileError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ProfileError::Io { path: path.display().to_string(), source: e })?;
    let profile: Profile = toml::from_str(&content)
        .map_err(|e| ProfileError::Parse { path: path.display().to_string(), source: e })?;
    Ok(profile)
}

/// Built-in profiles that don't require files on disk.
#[must_use]
pub fn builtin_smoke() -> Profile {
    Profile {
        name: "smoke".to_owned(),
        description: Some("Quick smoke test: 3 apps × 1 engine × X11 × Telex".to_owned()),
        engines: vec!["ibus-bamboo".to_owned()],
        apps: vec!["gedit".to_owned(), "kate".to_owned(), "firefox".to_owned()],
        sessions: vec!["x11".to_owned()],
        modes: vec!["telex".to_owned()],
        vector_tags: vec![],
    }
}

#[must_use]
pub fn builtin_full() -> Profile {
    Profile {
        name: "full".to_owned(),
        description: Some("Full matrix: all apps × all engines × X11+Wayland × Telex".to_owned()),
        engines: vec![
            "ibus-bamboo".to_owned(),
            "ibus-unikey".to_owned(),
            "fcitx5-bamboo".to_owned(),
            "fcitx5-unikey".to_owned(),
        ],
        apps: vec![
            "gedit".to_owned(),
            "kate".to_owned(),
            "firefox".to_owned(),
            "chromium".to_owned(),
            "vscode".to_owned(),
            "libreoffice".to_owned(),
        ],
        sessions: vec!["x11".to_owned(), "wayland".to_owned()],
        modes: vec!["telex".to_owned()],
        vector_tags: vec![],
    }
}

#[must_use]
pub fn builtin_bugs() -> Profile {
    Profile {
        name: "bugs".to_owned(),
        description: Some("Regression vectors only".to_owned()),
        engines: vec!["ibus-bamboo".to_owned(), "fcitx5-bamboo".to_owned()],
        apps: vec!["gedit".to_owned(), "vscode".to_owned()],
        sessions: vec!["x11".to_owned()],
        modes: vec!["telex".to_owned()],
        vector_tags: vec!["regression".to_owned()],
    }
}

/// Resolve a profile by name: first check `profiles/<name>.toml` on disk,
/// then fall back to builtins.
pub fn resolve_profile(name: &str, profiles_dir: &Path) -> Result<Profile, ProfileError> {
    let path = profiles_dir.join(format!("{name}.toml"));
    if path.is_file() {
        return load_profile(&path);
    }
    match name {
        "smoke" => Ok(builtin_smoke()),
        "full" => Ok(builtin_full()),
        "bugs" => Ok(builtin_bugs()),
        _ => Err(ProfileError::NotFound(name.to_owned())),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("i/o error reading `{path}`: {source}")]
    Io { path: String, source: std::io::Error },

    #[error("TOML parse error in `{path}`: {source}")]
    Parse { path: String, source: toml::de::Error },

    #[error("profile `{0}` not found (checked profiles/ dir and builtins)")]
    NotFound(String),
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn smoke_profile_expands_to_3_combos() {
        let p = builtin_smoke();
        let combos = p.expand_combos();
        // 1 engine × 3 apps × 1 session × 1 mode = 3
        assert_eq!(combos.len(), 3);
    }

    #[test]
    fn full_profile_expands_correctly() {
        let p = builtin_full();
        let combos = p.expand_combos();
        // 4 engines × 6 apps × 2 sessions × 1 mode = 48
        assert_eq!(combos.len(), 48);
    }

    #[test]
    fn profile_toml_parse() {
        let toml = r#"
name = "custom"
description = "Test"
engines = ["ibus-bamboo"]
apps = ["gedit"]
sessions = ["x11"]
modes = ["telex", "vni"]
"#;
        let p: Profile = toml::from_str(toml).unwrap();
        assert_eq!(p.name, "custom");
        let combos = p.expand_combos();
        assert_eq!(combos.len(), 2); // 1×1×1×2
    }

    #[test]
    fn resolve_builtin_smoke() {
        let dir = tempfile::tempdir().unwrap();
        let p = resolve_profile("smoke", dir.path()).unwrap();
        assert_eq!(p.name, "smoke");
    }

    #[test]
    fn resolve_unknown_profile_errors() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve_profile("nonexistent", dir.path()).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn resolve_from_disk_overrides_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("smoke.toml");
        std::fs::write(
            &path,
            r#"
name = "smoke-custom"
engines = ["fcitx5-bamboo"]
apps = ["kate"]
sessions = ["wayland"]
"#,
        )
        .unwrap();
        let p = resolve_profile("smoke", dir.path()).unwrap();
        assert_eq!(p.name, "smoke-custom");
    }
}
