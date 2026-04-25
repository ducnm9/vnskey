// SPDX-License-Identifier: GPL-3.0-or-later
//
// `AppProfile` registry — the ten apps Doctor can diagnose when `--app <X>`
// is passed.
//
// Everything is `&'static` so profiles can be referenced from spawned
// detector tasks without any lifetime gymnastics. The list is deliberately
// short: these are the Electron / Chromium / native apps that actually
// break Vietnamese input on Linux in 2026. Additions go via a future
// `~/.config/vietime/apps.toml` overlay, not here.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.5.

use vietime_core::AppKind;

/// Static profile for one recognised app.
///
/// * `id` — canonical kebab-case identifier. Emitted verbatim in `AppFacts.app_id`.
/// * `aliases` — additional names users might type (`code`, `code-oss`, …).
/// * `display_name` — pretty-printed in the report.
/// * `kind_hint` — starting classification before the generic detector runs
///   `file(1)` / the Electron detector scans strings. The generic detector
///   can override this when `file` identifies a script, AppImage, etc.
/// * `binary_hints` — absolute paths to probe in order before falling back
///   to `$PATH` lookups. Sysroot-prefixed inside the resolver for hermetic
///   tests.
/// * `extra_detectors` — detector ids to run when this app is the target.
///   Week 4 only uses `"app.electron"`; Week 5+ may add checker-specific
///   ids like `"ime.vd010"`.
#[derive(Debug, Clone)]
pub struct AppProfile {
    pub id: &'static str,
    pub aliases: &'static [&'static str],
    pub display_name: &'static str,
    pub kind_hint: AppKind,
    pub binary_hints: &'static [&'static str],
    pub extra_detectors: &'static [&'static str],
}

/// Shortcuts so the `PROFILES` table stays readable.
const APP_ELECTRON: &[&str] = &["app.electron"];
const NO_EXTRA: &[&str] = &[];

/// The ten Week-4 profiles (spec §B.5). Order is user-visible in `list`
/// output so it stays deterministic — do NOT sort alphabetically.
///
/// `AppKind::Flatpak`/`Snap` variants carry data that must be known at
/// detection time, so they don't appear in the hint column; DOC-31/32 can
/// promote a profile's `kind_hint` to one of those runtime variants if
/// the binary path happens to point into a Flatpak / Snap sandbox.
pub const PROFILES: &[AppProfile] = &[
    AppProfile {
        id: "vscode",
        aliases: &["code", "code-oss", "vscodium", "visual studio code"],
        display_name: "Visual Studio Code",
        kind_hint: AppKind::Electron,
        binary_hints: &[
            "/usr/bin/code",
            "/usr/share/code/code",
            "/snap/bin/code",
            "/var/lib/flatpak/exports/bin/com.visualstudio.code",
        ],
        extra_detectors: APP_ELECTRON,
    },
    AppProfile {
        id: "chrome",
        aliases: &["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"],
        display_name: "Google Chrome / Chromium",
        kind_hint: AppKind::Chromium,
        binary_hints: &[
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
        ],
        extra_detectors: APP_ELECTRON,
    },
    AppProfile {
        id: "firefox",
        aliases: &["firefox-esr", "firefox-developer-edition"],
        display_name: "Mozilla Firefox",
        kind_hint: AppKind::Native,
        binary_hints: &["/usr/bin/firefox", "/usr/lib/firefox/firefox", "/snap/bin/firefox"],
        extra_detectors: NO_EXTRA,
    },
    AppProfile {
        id: "slack",
        aliases: &[],
        display_name: "Slack",
        kind_hint: AppKind::Electron,
        binary_hints: &[
            "/usr/bin/slack",
            "/snap/bin/slack",
            "/var/lib/flatpak/exports/bin/com.slack.Slack",
        ],
        extra_detectors: APP_ELECTRON,
    },
    AppProfile {
        id: "discord",
        aliases: &[],
        display_name: "Discord",
        kind_hint: AppKind::Electron,
        binary_hints: &["/usr/bin/discord", "/opt/discord/Discord", "/snap/bin/discord"],
        extra_detectors: APP_ELECTRON,
    },
    AppProfile {
        id: "obsidian",
        aliases: &[],
        display_name: "Obsidian",
        kind_hint: AppKind::Electron,
        binary_hints: &[
            "/usr/bin/obsidian",
            "/opt/Obsidian/obsidian",
            "/var/lib/flatpak/exports/bin/md.obsidian.Obsidian",
        ],
        extra_detectors: APP_ELECTRON,
    },
    AppProfile {
        id: "telegram",
        aliases: &["telegram-desktop"],
        display_name: "Telegram Desktop",
        kind_hint: AppKind::Native,
        binary_hints: &[
            "/usr/bin/telegram-desktop",
            "/var/lib/flatpak/exports/bin/org.telegram.desktop",
        ],
        extra_detectors: NO_EXTRA,
    },
    AppProfile {
        id: "libreoffice",
        aliases: &["soffice", "lowriter", "localc"],
        display_name: "LibreOffice",
        kind_hint: AppKind::Native,
        binary_hints: &["/usr/bin/libreoffice", "/usr/bin/soffice"],
        extra_detectors: NO_EXTRA,
    },
    AppProfile {
        id: "intellij",
        aliases: &["idea", "intellijidea", "idea-community"],
        display_name: "IntelliJ IDEA",
        kind_hint: AppKind::Jvm,
        binary_hints: &["/usr/bin/idea", "/usr/local/bin/idea", "/opt/idea/bin/idea.sh"],
        extra_detectors: NO_EXTRA,
    },
    AppProfile {
        id: "neovide",
        aliases: &[],
        display_name: "Neovide",
        kind_hint: AppKind::Native,
        binary_hints: &["/usr/bin/neovide", "/usr/local/bin/neovide"],
        extra_detectors: NO_EXTRA,
    },
];

/// Resolve a user-provided app name or path to a static profile.
///
/// Lookup is case-insensitive and goes id → aliases → basename of the path
/// (if the input contains `/`). Returns `None` on miss — callers should
/// render the unknown id as a note rather than an anomaly, since a typo on
/// `--app` is a user error, not a Doctor bug.
#[must_use]
pub fn resolve_app(input: &str) -> Option<&'static AppProfile> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();

    // Step 1: try id / aliases on the raw-ish input.
    if let Some(p) = lookup_exact(&lower) {
        return Some(p);
    }

    // Step 2: if the input looks like a path, retry with its basename.
    if trimmed.contains('/') {
        if let Some(base) = std::path::Path::new(trimmed).file_name().and_then(|s| s.to_str()) {
            let base_lower = base.to_ascii_lowercase();
            if let Some(p) = lookup_exact(&base_lower) {
                return Some(p);
            }
        }
    }

    None
}

fn lookup_exact(needle: &str) -> Option<&'static AppProfile> {
    for profile in PROFILES {
        if profile.id.eq_ignore_ascii_case(needle) {
            return Some(profile);
        }
        for alias in profile.aliases {
            if alias.eq_ignore_ascii_case(needle) {
                return Some(profile);
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn resolves_by_canonical_id() {
        let p = resolve_app("vscode").expect("vscode profile");
        assert_eq!(p.id, "vscode");
        assert_eq!(p.display_name, "Visual Studio Code");
    }

    #[test]
    fn resolves_by_alias_case_insensitive() {
        // Both the alias lookup and the case-insensitivity exercised in one test.
        assert_eq!(resolve_app("code").expect("alias").id, "vscode");
        assert_eq!(resolve_app("CODE").expect("upper").id, "vscode");
        assert_eq!(resolve_app("Code-OSS").expect("mixed").id, "vscode");
    }

    #[test]
    fn resolves_full_path_via_basename() {
        let p = resolve_app("/usr/bin/code").expect("path");
        assert_eq!(p.id, "vscode");
        let p = resolve_app("/snap/bin/slack").expect("snap path");
        assert_eq!(p.id, "slack");
    }

    #[test]
    fn unknown_input_returns_none() {
        assert!(resolve_app("notepad++").is_none());
        assert!(resolve_app("").is_none());
        assert!(resolve_app("   ").is_none());
    }

    #[test]
    fn every_profile_has_a_unique_id() {
        // The registry is a closed set — drifting this list should be a loud
        // failure, not a silent shadowing.
        let mut ids: Vec<&str> = PROFILES.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate profile id in registry");
        // And we promised exactly ten.
        assert_eq!(PROFILES.len(), 10);
    }

    #[test]
    fn electron_profiles_have_app_electron_in_extra_detectors() {
        for p in PROFILES {
            if matches!(p.kind_hint, AppKind::Electron | AppKind::Chromium) {
                assert!(
                    p.extra_detectors.contains(&"app.electron"),
                    "{} should list app.electron in extra_detectors",
                    p.id
                );
            }
        }
    }
}
