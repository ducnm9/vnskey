// SPDX-License-Identifier: GPL-3.0-or-later
//
// Session type detection.
//
// Linux desktop can run under X11, Wayland, or a TTY. We detect which by
// reading a small set of environment variables the login manager sets.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.2.
//
// The caller passes a pre-built `HashMap<String, String>` of env vars rather
// than us touching `std::env` directly — this keeps the function testable
// and side-effect-free.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Display/session kind the user's desktop is running on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    /// `XDG_SESSION_TYPE=x11` or `DISPLAY` is set.
    X11,
    /// `XDG_SESSION_TYPE=wayland` or `WAYLAND_DISPLAY` is set.
    Wayland,
    /// `XDG_SESSION_TYPE=tty`.
    Tty,
    /// We have no signal either way — includes headless SSH sessions where
    /// no display is set up at all.
    Unknown,
}

impl SessionType {
    /// Short lowercase label suitable for logs and JSON output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X11 => "x11",
            Self::Wayland => "wayland",
            Self::Tty => "tty",
            Self::Unknown => "unknown",
        }
    }
}

/// Classify a session given the environment.
///
/// Order of precedence (explicit signals beat implicit ones):
/// 1. `XDG_SESSION_TYPE` set to `wayland`, `x11`, or `tty` — authoritative.
/// 2. `WAYLAND_DISPLAY` non-empty → Wayland.
/// 3. `DISPLAY` non-empty → X11.
/// 4. Otherwise → Unknown.
///
/// This mirrors the detection order used by `systemd-logind` and most IM
/// framework autostart scripts. See spec/01 §B.2.
#[must_use]
pub fn detect_session_from_env(env: &HashMap<String, String>) -> SessionType {
    if let Some(explicit) = env.get("XDG_SESSION_TYPE") {
        match explicit.trim().to_ascii_lowercase().as_str() {
            "wayland" => return SessionType::Wayland,
            "x11" => return SessionType::X11,
            "tty" => return SessionType::Tty,
            // Fall through for "unspecified" or unexpected values.
            _ => {}
        }
    }

    if non_empty(env, "WAYLAND_DISPLAY") {
        return SessionType::Wayland;
    }
    if non_empty(env, "DISPLAY") {
        return SessionType::X11;
    }

    SessionType::Unknown
}

fn non_empty(env: &HashMap<String, String>, key: &str) -> bool {
    env.get(key).is_some_and(|v| !v.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect()
    }

    #[test]
    fn xdg_session_type_wayland_wins() {
        let e = env(&[("XDG_SESSION_TYPE", "wayland"), ("DISPLAY", ":0")]);
        assert_eq!(detect_session_from_env(&e), SessionType::Wayland);
    }

    #[test]
    fn xdg_session_type_x11_wins_over_wayland_display() {
        let e = env(&[("XDG_SESSION_TYPE", "x11"), ("WAYLAND_DISPLAY", "wayland-0")]);
        assert_eq!(detect_session_from_env(&e), SessionType::X11);
    }

    #[test]
    fn wayland_display_fallback() {
        let e = env(&[("WAYLAND_DISPLAY", "wayland-0")]);
        assert_eq!(detect_session_from_env(&e), SessionType::Wayland);
    }

    #[test]
    fn display_fallback() {
        let e = env(&[("DISPLAY", ":0")]);
        assert_eq!(detect_session_from_env(&e), SessionType::X11);
    }

    #[test]
    fn empty_display_ignored() {
        let e = env(&[("DISPLAY", "")]);
        assert_eq!(detect_session_from_env(&e), SessionType::Unknown);
    }

    #[test]
    fn tty_session() {
        let e = env(&[("XDG_SESSION_TYPE", "tty")]);
        assert_eq!(detect_session_from_env(&e), SessionType::Tty);
    }

    #[test]
    fn unknown_when_no_signals() {
        let e = env(&[]);
        assert_eq!(detect_session_from_env(&e), SessionType::Unknown);
    }

    #[test]
    fn unknown_when_xdg_value_garbage() {
        let e = env(&[("XDG_SESSION_TYPE", "banana")]);
        assert_eq!(detect_session_from_env(&e), SessionType::Unknown);
    }

    #[test]
    fn case_insensitive_xdg_value() {
        let e = env(&[("XDG_SESSION_TYPE", "WAYLAND")]);
        assert_eq!(detect_session_from_env(&e), SessionType::Wayland);
    }

    #[test]
    fn as_str_labels() {
        assert_eq!(SessionType::X11.as_str(), "x11");
        assert_eq!(SessionType::Wayland.as_str(), "wayland");
        assert_eq!(SessionType::Tty.as_str(), "tty");
        assert_eq!(SessionType::Unknown.as_str(), "unknown");
    }
}
