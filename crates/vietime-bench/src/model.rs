// SPDX-License-Identifier: GPL-3.0-or-later
//
// Bench data model — Week 1 subset.
//
// Only `InputMode` lives here for now; `list` and the future `run` stub both
// need to print and parse it. The bigger `Profile` / `TestVector` /
// `ComboResult` / `RunResult` structs from `spec/03-phase3-test-suite.md` §B.9
// arrive with BEN-12/13/14 in Week 2 — keeping them out until we have a real
// test-vector loader means we don't freeze a wire format we haven't
// exercised yet.
//
// Spec ref: `spec/03-phase3-test-suite.md` §A.3 (modes) + §B.9 (full model).

use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Vietnamese input method (the *typing layout*, not the engine). Four modes
/// are in scope for the MVP matrix:
///
/// * `Telex` — the default for most engines; the big one.
/// * `Vni` — digit-based layout, common among power users.
/// * `Viqr` — ASCII-transliteration style, mostly academic but widely
///   referenced by test suites.
/// * `SimpleTelex` — a subset of Telex that disables auto-tone repositioning;
///   Bamboo ships with this as a toggle and our test vectors cover it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputMode {
    Telex,
    Vni,
    Viqr,
    SimpleTelex,
}

impl InputMode {
    /// Short kebab-case label used on the CLI (`--mode`) and in run manifests.
    /// Intentionally distinct from the serde snake_case wire format so the
    /// CLI can accept `simple-telex` which reads more naturally than
    /// `simple_telex`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Telex => "telex",
            Self::Vni => "vni",
            Self::Viqr => "viqr",
            Self::SimpleTelex => "simple-telex",
        }
    }

    /// Stable enumeration for `list` output and matrix iteration. The order
    /// matches the `InputMode` declaration — don't reorder without updating
    /// goldens.
    #[must_use]
    pub fn all() -> [Self; 4] {
        [Self::Telex, Self::Vni, Self::Viqr, Self::SimpleTelex]
    }
}

impl std::fmt::Display for InputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when `--mode <x>` doesn't match any supported mode.
#[derive(Debug, thiserror::Error)]
#[error("unknown input mode `{0}` — expected one of: {1}")]
pub struct ParseInputModeError(pub String, pub String);

impl FromStr for InputMode {
    type Err = ParseInputModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Normalise both the kebab-case CLI spelling and the snake_case wire
        // format: underscores and hyphens are interchangeable, leading/trailing
        // whitespace is dropped, and matching is case-insensitive.
        let normalised = s.trim().to_ascii_lowercase().replace('_', "-");
        for m in Self::all() {
            if m.as_str() == normalised {
                return Ok(m);
            }
        }
        let supported = Self::all().iter().map(|m| m.as_str()).collect::<Vec<_>>().join(", ");
        Err(ParseInputModeError(s.to_owned(), supported))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn as_str_is_stable() {
        assert_eq!(InputMode::Telex.as_str(), "telex");
        assert_eq!(InputMode::Vni.as_str(), "vni");
        assert_eq!(InputMode::Viqr.as_str(), "viqr");
        assert_eq!(InputMode::SimpleTelex.as_str(), "simple-telex");
    }

    #[test]
    fn all_yields_four_unique_modes() {
        let all = InputMode::all();
        assert_eq!(all.len(), 4);
        let uniq: std::collections::HashSet<_> = all.iter().copied().collect();
        assert_eq!(uniq.len(), 4, "all() must not repeat modes");
    }

    #[test]
    fn from_str_accepts_kebab_and_snake_and_case() {
        assert_eq!(InputMode::from_str("telex").unwrap(), InputMode::Telex);
        assert_eq!(InputMode::from_str("VNI").unwrap(), InputMode::Vni);
        assert_eq!(InputMode::from_str("simple-telex").unwrap(), InputMode::SimpleTelex);
        assert_eq!(InputMode::from_str("simple_telex").unwrap(), InputMode::SimpleTelex);
        assert_eq!(InputMode::from_str("  VIQR  ").unwrap(), InputMode::Viqr);
    }

    #[test]
    fn from_str_rejects_unknown_and_lists_supported() {
        let err = InputMode::from_str("hex").expect_err("should reject `hex`");
        let msg = err.to_string();
        assert!(msg.contains("hex"), "error mentions input: {msg}");
        assert!(msg.contains("telex"), "error lists supported modes: {msg}");
    }

    #[test]
    fn display_matches_as_str() {
        for m in InputMode::all() {
            assert_eq!(format!("{m}"), m.as_str());
        }
    }

    #[test]
    fn serde_round_trip_uses_snake_case() {
        // Wire format is snake_case (not kebab-case) so that TOML/JSON
        // manifests use a consistent convention across the workspace.
        let toml_s = toml::to_string(&Wrapper { mode: InputMode::SimpleTelex }).unwrap();
        assert!(toml_s.contains("simple_telex"), "toml uses snake_case: {toml_s}");
        let back: Wrapper = toml::from_str(&toml_s).unwrap();
        assert_eq!(back.mode, InputMode::SimpleTelex);
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct Wrapper {
        mode: InputMode,
    }
}
