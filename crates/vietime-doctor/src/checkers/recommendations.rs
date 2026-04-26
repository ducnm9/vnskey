// SPDX-License-Identifier: GPL-3.0-or-later
//
// Static recommendation catalogue — VR001…VR008.
//
// Issues emitted by the Week 5 checkers reference a `VR###` id via
// `Issue::recommendation`. The orchestrator resolves each unique id once
// per report by calling `lookup(id)` and populates `Report.recommendations`
// so the renderer can group "fixes that apply" without duplicating the
// command text per issue.
//
// Spec ref: `spec/01-phase1-doctor.md` §A.5, §B.4.

use vietime_core::Recommendation;

/// Build the full catalogue. Called once on demand by `lookup` — the
/// recommendations hold `Vec`/`String` fields so a `static` wouldn't be
/// `const`-friendly without a lot of `&'static str` gymnastics. The cost
/// (rebuilding 8 structs on each `lookup` call) is negligible; the whole
/// catalogue is built once per Doctor run.
#[must_use]
#[allow(clippy::too_many_lines)]
fn catalogue() -> [Recommendation; 8] {
    [
        Recommendation {
            id: "VR001".to_owned(),
            title: "Start an input-method daemon".to_owned(),
            description: "No IM daemon is running, but a Vietnamese engine is \
                 installed. Start either IBus or Fcitx5 (pick one — running \
                 both causes VD002)."
                .to_owned(),
            commands: vec![
                "# Option A — IBus".to_owned(),
                "ibus-daemon -drx".to_owned(),
                "# Option B — Fcitx5".to_owned(),
                "fcitx5 -d".to_owned(),
            ],
            safe_to_run_unattended: false,
            references: vec![
                "https://wiki.archlinux.org/title/IBus".to_owned(),
                "https://fcitx-im.org/wiki/Fcitx_5".to_owned(),
            ],
        },
        Recommendation {
            id: "VR002".to_owned(),
            title: "Stop one of the conflicting IM daemons".to_owned(),
            description: "Both ibus-daemon and fcitx5 are running. Pick one \
                 and stop the other — having both active confuses the GTK/Qt \
                 IM module routing."
                .to_owned(),
            commands: vec![
                "# Keep IBus, stop Fcitx5:".to_owned(),
                "pkill -x fcitx5".to_owned(),
                "# Or keep Fcitx5, stop IBus:".to_owned(),
                "pkill -x ibus-daemon".to_owned(),
            ],
            safe_to_run_unattended: false,
            references: vec![
                "https://wiki.archlinux.org/title/Input_method#Troubleshooting".to_owned(),
            ],
        },
        Recommendation {
            id: "VR003".to_owned(),
            title: "Align IM env variables with the active framework".to_owned(),
            description: "GTK_IM_MODULE, QT_IM_MODULE, and XMODIFIERS must \
                 all point at the framework you actually run. A mismatch \
                 causes some apps to silently drop Vietnamese input."
                .to_owned(),
            commands: vec![
                "# For IBus — add to ~/.profile:".to_owned(),
                "export GTK_IM_MODULE=ibus".to_owned(),
                "export QT_IM_MODULE=ibus".to_owned(),
                "export XMODIFIERS=@im=ibus".to_owned(),
                "# For Fcitx5, use `fcitx` in each value instead.".to_owned(),
            ],
            safe_to_run_unattended: false,
            references: vec![
                "https://wiki.archlinux.org/title/Input_method#Environment_variables".to_owned(),
            ],
        },
        Recommendation {
            id: "VR004".to_owned(),
            title: "Set SDL_IM_MODULE".to_owned(),
            description: "Games and SDL-based apps (Steam, Love2D, emulators) \
                 need SDL_IM_MODULE to route Vietnamese input. Without it \
                 they fall back to raw keycodes."
                .to_owned(),
            commands: vec![
                "# For IBus:".to_owned(),
                "export SDL_IM_MODULE=ibus".to_owned(),
                "# For Fcitx5:".to_owned(),
                "export SDL_IM_MODULE=fcitx".to_owned(),
            ],
            safe_to_run_unattended: true,
            references: vec!["https://wiki.libsdl.org/SDL2/FAQUsingSDL".to_owned()],
        },
        Recommendation {
            id: "VR005".to_owned(),
            title: "Register the installed engine".to_owned(),
            description: "A Vietnamese IME package is installed on disk but \
                 not registered with the active framework. Open the config \
                 tool and add it to the input-method list."
                .to_owned(),
            commands: vec![
                "# IBus:".to_owned(),
                "ibus-setup".to_owned(),
                "# Fcitx5:".to_owned(),
                "fcitx5-configtool".to_owned(),
            ],
            safe_to_run_unattended: false,
            references: vec![
                "https://github.com/BambooEngine/ibus-bamboo".to_owned(),
                "https://github.com/fcitx/fcitx5-unikey".to_owned(),
            ],
        },
        Recommendation {
            id: "VR006".to_owned(),
            title: "Consider switching to Fcitx5 on Wayland".to_owned(),
            description: "IBus on Wayland has known text-insertion bugs in \
                 several desktops. Fcitx5's `wayland-im` addon is the \
                 recommended path today. Install the Fcitx5 equivalent of \
                 your current engine and log out/in."
                .to_owned(),
            commands: vec![
                "# Debian/Ubuntu:".to_owned(),
                "sudo apt install fcitx5 fcitx5-bamboo".to_owned(),
                "# Arch:".to_owned(),
                "sudo pacman -S fcitx5 fcitx5-bamboo".to_owned(),
            ],
            safe_to_run_unattended: false,
            references: vec!["https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland".to_owned()],
        },
        Recommendation {
            id: "VR007".to_owned(),
            title: "Launch Electron apps with Ozone/Wayland flags".to_owned(),
            description: "Electron on Wayland needs --ozone-platform=wayland \
                 (or UseOzonePlatform in --enable-features) to route IM \
                 events through the Wayland text-input protocol. Without it \
                 Vietnamese input is dropped silently."
                .to_owned(),
            commands: vec![
                "# Pass on launch:".to_owned(),
                "code --ozone-platform=wayland --enable-features=UseOzonePlatform,WaylandWindowDecorations".to_owned(),
                "# Or persist in ~/.config/code-flags.conf (one flag per line).".to_owned(),
            ],
            safe_to_run_unattended: true,
            references: vec![
                "https://www.electronjs.org/docs/latest/api/command-line-switches"
                    .to_owned(),
            ],
        },
        Recommendation {
            id: "VR008".to_owned(),
            title: "Enable Chrome/Chromium Wayland Ozone backend".to_owned(),
            description: "Chrome is running with the X11 backend on a \
                 Wayland session — Vietnamese input drops through XWayland \
                 in that mode. Switch to the Ozone/Wayland backend."
                .to_owned(),
            commands: vec![
                "google-chrome --ozone-platform=wayland".to_owned(),
                "# Or persist via chrome://flags → Preferred Ozone platform → Wayland".to_owned(),
            ],
            safe_to_run_unattended: true,
            references: vec!["https://wiki.archlinux.org/title/Chromium#Native_Wayland_support"
                .to_owned()],
        },
    ]
}

/// Return the recommendation with the given VR id, or `None` if it isn't
/// one of the 8 Week 5 catalogue entries.
#[must_use]
pub fn lookup(id: &str) -> Option<Recommendation> {
    catalogue().into_iter().find(|r| r.id == id)
}

/// Return the full Week 5 catalogue.
#[must_use]
pub fn all() -> Vec<Recommendation> {
    catalogue().to_vec()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn lookup_finds_each_known_id() {
        for id in ["VR001", "VR002", "VR003", "VR004", "VR005", "VR006", "VR007", "VR008"] {
            let r = lookup(id).expect("recommendation must exist");
            assert_eq!(r.id, id);
            assert!(!r.title.is_empty());
            assert!(!r.description.is_empty());
        }
    }

    #[test]
    fn lookup_returns_none_for_unknown_id() {
        assert!(lookup("VR999").is_none());
        assert!(lookup("").is_none());
    }

    #[test]
    fn all_has_eight_entries() {
        assert_eq!(all().len(), 8);
    }

    #[test]
    fn unattended_flags_match_spec() {
        // Fixes that require picking between IBus and Fcitx5 (or running
        // config tools) are NOT safe to run unattended. Env-var tweaks
        // and Ozone flags ARE.
        assert!(!lookup("VR001").expect("VR001").safe_to_run_unattended);
        assert!(!lookup("VR002").expect("VR002").safe_to_run_unattended);
        assert!(!lookup("VR003").expect("VR003").safe_to_run_unattended);
        assert!(lookup("VR004").expect("VR004").safe_to_run_unattended);
        assert!(!lookup("VR005").expect("VR005").safe_to_run_unattended);
        assert!(!lookup("VR006").expect("VR006").safe_to_run_unattended);
        assert!(lookup("VR007").expect("VR007").safe_to_run_unattended);
        assert!(lookup("VR008").expect("VR008").safe_to_run_unattended);
    }
}
