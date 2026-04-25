// SPDX-License-Identifier: GPL-3.0-or-later
//
// Desktop-environment detection.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.2 (`DesktopEnv`), §B.3 (`sys.desktop`).
//
// Only the `$XDG_CURRENT_DESKTOP` / `$DESKTOP_SESSION` portion is implemented
// here; version extraction (e.g. `gnome-shell --version`) lives in the Doctor
// detector because it spawns subprocesses.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A best-effort classification of the user's desktop environment.
///
/// The enum is intentionally open-ended with `Other(String)` so that unknown
/// desktops (e.g. `"LXQt"`) round-trip through JSON without losing the raw
/// label reported by the system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DesktopEnv {
    Gnome { version: Option<String> },
    Kde { version: Option<String> },
    Xfce,
    Cinnamon,
    Mate,
    Budgie,
    Sway,
    Hyprland,
    Lxqt,
    Lxde,
    Pantheon,
    Unity,
    Other(String),
}

impl DesktopEnv {
    /// Human-readable label for Markdown rendering.
    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Gnome { version: Some(v) } => format!("GNOME {v}"),
            Self::Gnome { version: None } => "GNOME".to_owned(),
            Self::Kde { version: Some(v) } => format!("KDE Plasma {v}"),
            Self::Kde { version: None } => "KDE Plasma".to_owned(),
            Self::Xfce => "XFCE".to_owned(),
            Self::Cinnamon => "Cinnamon".to_owned(),
            Self::Mate => "MATE".to_owned(),
            Self::Budgie => "Budgie".to_owned(),
            Self::Sway => "Sway".to_owned(),
            Self::Hyprland => "Hyprland".to_owned(),
            Self::Lxqt => "LXQt".to_owned(),
            Self::Lxde => "LXDE".to_owned(),
            Self::Pantheon => "Pantheon".to_owned(),
            Self::Unity => "Unity".to_owned(),
            Self::Other(label) => label.clone(),
        }
    }
}

/// Detect the user's desktop environment from environment variables.
///
/// Reads `XDG_CURRENT_DESKTOP` first (the standard), falling back to
/// `DESKTOP_SESSION` if the former is missing. Returns `None` when neither
/// variable is present — the caller should treat that as "unknown" rather
/// than inventing a default.
///
/// Version fields are always `None` here; the Doctor fills them in later by
/// running `gnome-shell --version` etc. Keeping it pure means this function
/// is side-effect-free and trivially unit-testable.
#[must_use]
pub fn detect_desktop_from_env(env: &HashMap<String, String>) -> Option<DesktopEnv> {
    let raw = env.get("XDG_CURRENT_DESKTOP").or_else(|| env.get("DESKTOP_SESSION"))?;
    if raw.trim().is_empty() {
        return None;
    }

    // `XDG_CURRENT_DESKTOP` is colon-separated per the spec (e.g.
    // `"ubuntu:GNOME"`, `"pop:GNOME"`). We take the last segment because the
    // vendor prefix before the colon is just branding — `ubuntu:GNOME` is
    // still GNOME for our purposes.
    let candidate =
        raw.split(':').map(str::trim).filter(|s| !s.is_empty()).next_back().unwrap_or(raw);

    Some(classify_desktop(candidate))
}

fn classify_desktop(raw: &str) -> DesktopEnv {
    let lower = raw.to_ascii_lowercase();
    match lower.as_str() {
        "gnome" | "gnome-classic" | "gnome-xorg" | "gnome-wayland" => {
            DesktopEnv::Gnome { version: None }
        }
        "kde" | "plasma" => DesktopEnv::Kde { version: None },
        "xfce" | "xubuntu" => DesktopEnv::Xfce,
        "x-cinnamon" | "cinnamon" => DesktopEnv::Cinnamon,
        "mate" => DesktopEnv::Mate,
        "budgie" | "budgie:gnome" => DesktopEnv::Budgie,
        "sway" => DesktopEnv::Sway,
        "hyprland" => DesktopEnv::Hyprland,
        "lxqt" => DesktopEnv::Lxqt,
        "lxde" => DesktopEnv::Lxde,
        "pantheon" => DesktopEnv::Pantheon,
        "unity" | "unity:unity7:ubuntu" => DesktopEnv::Unity,
        _ => DesktopEnv::Other(raw.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn env_with(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[rstest]
    #[case("GNOME", DesktopEnv::Gnome { version: None })]
    #[case("ubuntu:GNOME", DesktopEnv::Gnome { version: None })]
    #[case("pop:GNOME", DesktopEnv::Gnome { version: None })]
    #[case("KDE", DesktopEnv::Kde { version: None })]
    #[case("plasma", DesktopEnv::Kde { version: None })]
    #[case("XFCE", DesktopEnv::Xfce)]
    #[case("X-Cinnamon", DesktopEnv::Cinnamon)]
    #[case("MATE", DesktopEnv::Mate)]
    #[case("sway", DesktopEnv::Sway)]
    #[case("Hyprland", DesktopEnv::Hyprland)]
    fn detects_common_desktops(#[case] raw: &str, #[case] expected: DesktopEnv) {
        let env = env_with(&[("XDG_CURRENT_DESKTOP", raw)]);
        assert_eq!(detect_desktop_from_env(&env), Some(expected));
    }

    #[test]
    fn falls_back_to_desktop_session_when_xdg_missing() {
        let env = env_with(&[("DESKTOP_SESSION", "gnome")]);
        assert_eq!(detect_desktop_from_env(&env), Some(DesktopEnv::Gnome { version: None }));
    }

    #[test]
    fn returns_none_when_no_env_set() {
        let env = HashMap::new();
        assert_eq!(detect_desktop_from_env(&env), None);
    }

    #[test]
    fn returns_none_when_values_blank() {
        let env = env_with(&[("XDG_CURRENT_DESKTOP", "   ")]);
        assert_eq!(detect_desktop_from_env(&env), None);
    }

    #[test]
    fn unknown_desktop_preserves_raw_label() {
        let env = env_with(&[("XDG_CURRENT_DESKTOP", "Enlightenment")]);
        assert_eq!(
            detect_desktop_from_env(&env),
            Some(DesktopEnv::Other("Enlightenment".to_owned()))
        );
    }

    #[test]
    fn xdg_takes_priority_over_desktop_session() {
        let env = env_with(&[("XDG_CURRENT_DESKTOP", "KDE"), ("DESKTOP_SESSION", "gnome")]);
        assert_eq!(detect_desktop_from_env(&env), Some(DesktopEnv::Kde { version: None }));
    }

    #[test]
    fn display_name_includes_version_when_known() {
        let gnome = DesktopEnv::Gnome { version: Some("46".to_owned()) };
        assert_eq!(gnome.display_name(), "GNOME 46");
        let no_ver = DesktopEnv::Gnome { version: None };
        assert_eq!(no_ver.display_name(), "GNOME");
    }
}
