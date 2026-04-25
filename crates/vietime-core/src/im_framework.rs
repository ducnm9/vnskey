// SPDX-License-Identifier: GPL-3.0-or-later
//
// Input method framework enum.
//
// VietIME Suite speaks of 3 frameworks — IBus, Fcitx5, and "none" (either no
// IM is set up, or it's an older framework we don't support yet like Fcitx4
// or SCIM).
//
// Spec ref: `spec/00-vision-and-scope.md` §6, `spec/01-phase1-doctor.md` §B.2.
//
// Engine detection (Bamboo vs Unikey) lives in the Doctor crate because it
// requires D-Bus queries; the framework enum is just the tag.

use serde::{Deserialize, Serialize};

/// Input method framework.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImFramework {
    Ibus,
    Fcitx5,
    /// Any older or unsupported framework (fcitx4, scim, ...) or none.
    None,
}

impl ImFramework {
    /// Short lowercase label used in IM env variables
    /// (`GTK_IM_MODULE=ibus` / `fcitx` / etc.).
    ///
    /// Note that Fcitx5 uses the legacy string `fcitx` in env vars — upstream
    /// kept the value stable for GTK/Qt compatibility when moving 4→5.
    #[must_use]
    pub fn env_value(self) -> Option<&'static str> {
        match self {
            Self::Ibus => Some("ibus"),
            Self::Fcitx5 => Some("fcitx"),
            Self::None => None,
        }
    }

    /// D-Bus well-known name for the framework daemon.
    #[must_use]
    pub fn dbus_name(self) -> Option<&'static str> {
        match self {
            Self::Ibus => Some("org.freedesktop.IBus"),
            Self::Fcitx5 => Some("org.fcitx.Fcitx5"),
            Self::None => None,
        }
    }

    /// Human-readable name for CLI output.
    #[must_use]
    pub fn display(self) -> &'static str {
        match self {
            Self::Ibus => "IBus",
            Self::Fcitx5 => "Fcitx5",
            Self::None => "None",
        }
    }
}

/// Parse a `GTK_IM_MODULE` / `QT_IM_MODULE` / `XMODIFIERS` value.
///
/// Returns `ImFramework::None` for empty, missing, or unrecognized values.
#[must_use]
pub fn parse_im_module_value(value: &str) -> ImFramework {
    // XMODIFIERS has the form `@im=<name>`; strip the prefix if present.
    let trimmed = value.trim();
    let name = trimmed.strip_prefix("@im=").unwrap_or(trimmed);
    match name.to_ascii_lowercase().as_str() {
        "ibus" => ImFramework::Ibus,
        // Fcitx5 kept `fcitx` as the env value for backward compatibility.
        "fcitx" | "fcitx5" => ImFramework::Fcitx5,
        _ => ImFramework::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn env_values() {
        assert_eq!(ImFramework::Ibus.env_value(), Some("ibus"));
        assert_eq!(ImFramework::Fcitx5.env_value(), Some("fcitx"));
        assert_eq!(ImFramework::None.env_value(), None);
    }

    #[test]
    fn dbus_names() {
        assert_eq!(ImFramework::Ibus.dbus_name(), Some("org.freedesktop.IBus"));
        assert_eq!(ImFramework::Fcitx5.dbus_name(), Some("org.fcitx.Fcitx5"));
        assert_eq!(ImFramework::None.dbus_name(), None);
    }

    #[test]
    fn display_labels() {
        assert_eq!(ImFramework::Ibus.display(), "IBus");
        assert_eq!(ImFramework::Fcitx5.display(), "Fcitx5");
        assert_eq!(ImFramework::None.display(), "None");
    }

    #[rstest]
    #[case("ibus", ImFramework::Ibus)]
    #[case("IBUS", ImFramework::Ibus)]
    #[case("fcitx", ImFramework::Fcitx5)]
    #[case("fcitx5", ImFramework::Fcitx5)]
    #[case("FCITX", ImFramework::Fcitx5)]
    #[case("@im=ibus", ImFramework::Ibus)]
    #[case("@im=fcitx", ImFramework::Fcitx5)]
    #[case("  @im=fcitx  ", ImFramework::Fcitx5)]
    #[case("", ImFramework::None)]
    #[case("scim", ImFramework::None)]
    #[case("uim", ImFramework::None)]
    #[case("@im=", ImFramework::None)]
    fn parses_im_module_value(#[case] input: &str, #[case] expected: ImFramework) {
        assert_eq!(parse_im_module_value(input), expected);
    }
}
