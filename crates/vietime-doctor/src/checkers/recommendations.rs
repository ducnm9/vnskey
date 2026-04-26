// SPDX-License-Identifier: GPL-3.0-or-later
//
// Static recommendation catalogue — VR001…VR014 (13 entries, VR012 omitted
// because VD012 is Info-only).
//
// Issues emitted by the Week 5 / Week 6 checkers reference a `VR###` id
// via `Issue::recommendation`. The orchestrator resolves each unique id
// once per report by calling `lookup(id)` and populates
// `Report.recommendations` so the renderer can group "fixes that apply"
// without duplicating the command text per issue.
//
// Spec ref: `spec/01-phase1-doctor.md` §A.5, §B.4.

use vietime_core::Recommendation;

/// Total entries in the catalogue. Sized as a named constant so the
/// `catalogue` return type, `all_has_N_entries` test, and the `lookup`
/// tests all read from a single source of truth — we don't want the type
/// and the tests to drift when Week 7 adds more.
pub const CATALOGUE_LEN: usize = 13;

/// Build the full catalogue. Called once on demand by `lookup` — the
/// recommendations hold `Vec`/`String` fields so a `static` wouldn't be
/// `const`-friendly without a lot of `&'static str` gymnastics. The cost
/// (rebuilding the entries on each `lookup` call) is negligible; the whole
/// catalogue is built once per Doctor run.
#[must_use]
#[allow(clippy::too_many_lines)]
fn catalogue() -> [Recommendation; CATALOGUE_LEN] {
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
        Recommendation {
            id: "VR009".to_owned(),
            title: "Consolidate IM env vars into a single config file".to_owned(),
            description: "GTK_IM_MODULE, QT_IM_MODULE and XMODIFIERS are set \
                 in multiple places (/etc/environment, ~/.profile, \
                 systemd --user …) with conflicting values. Pick one file \
                 and define all three keys there, identically."
                .to_owned(),
            commands: vec![
                "# Inspect every source:".to_owned(),
                "systemctl --user show-environment | grep _IM_MODULE".to_owned(),
                "grep _IM_MODULE /etc/environment ~/.profile ~/.bash_profile 2>/dev/null"
                    .to_owned(),
                "# Then remove the stray entries and keep a single authoritative file.".to_owned(),
            ],
            safe_to_run_unattended: false,
            references: vec![
                "https://wiki.archlinux.org/title/Environment_variables".to_owned(),
            ],
        },
        Recommendation {
            id: "VR010".to_owned(),
            title: "Replace the VSCode Snap with a .deb or Flatpak build".to_owned(),
            description: "Snap confinement strips IM env vars, which is why \
                 Vietnamese input works everywhere except the Snap VSCode \
                 window. Install the `.deb` from code.visualstudio.com or \
                 the com.visualstudio.code Flatpak — both honour \
                 GTK_IM_MODULE."
                .to_owned(),
            commands: vec![
                "# Remove the Snap:".to_owned(),
                "sudo snap remove code".to_owned(),
                "# Install the .deb (Debian/Ubuntu):".to_owned(),
                "sudo apt install ./code_<version>_amd64.deb".to_owned(),
                "# Or Flatpak:".to_owned(),
                "flatpak install flathub com.visualstudio.code".to_owned(),
            ],
            safe_to_run_unattended: false,
            references: vec![
                "https://code.visualstudio.com/docs/setup/linux".to_owned(),
                "https://flathub.org/apps/com.visualstudio.code".to_owned(),
            ],
        },
        Recommendation {
            id: "VR011".to_owned(),
            title: "Expose the IM socket to the Flatpak sandbox".to_owned(),
            description: "The Flatpak'd app can't see ibus-daemon or fcitx5 \
                 because its sandbox namespace hides the unix socket. Add \
                 a `flatpak override` to expose it, then relaunch the app."
                .to_owned(),
            commands: vec![
                "# IBus:".to_owned(),
                "flatpak override --user --socket=session-bus --env=GTK_IM_MODULE=ibus \
                 --env=QT_IM_MODULE=ibus --env=XMODIFIERS=@im=ibus <app-id>"
                    .to_owned(),
                "# Fcitx5:".to_owned(),
                "flatpak override --user --socket=wayland --socket=session-bus \
                 --env=GTK_IM_MODULE=fcitx --env=QT_IM_MODULE=fcitx \
                 --env=XMODIFIERS=@im=fcitx <app-id>"
                    .to_owned(),
            ],
            safe_to_run_unattended: false,
            references: vec![
                "https://docs.flatpak.org/en/latest/sandbox-permissions.html".to_owned(),
            ],
        },
        Recommendation {
            id: "VR013".to_owned(),
            title: "Enable the missing Fcitx5 session addon".to_owned(),
            description: "Fcitx5 on Wayland needs the `wayland-im` addon; on \
                 X11 it needs `xim`. Without the matching addon, keyboard \
                 events never reach the engine. Open fcitx5-configtool and \
                 enable it under `Addons`."
                .to_owned(),
            commands: vec![
                "fcitx5-configtool".to_owned(),
                "# Then `Addons → Wayland` (or `XIM`) → Enable".to_owned(),
                "# Persist by restarting the daemon:".to_owned(),
                "pkill -x fcitx5 && fcitx5 -d".to_owned(),
            ],
            safe_to_run_unattended: false,
            references: vec!["https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland".to_owned()],
        },
        Recommendation {
            id: "VR014".to_owned(),
            title: "Switch the active locale to a UTF-8 variant".to_owned(),
            description: "Vietnamese engines emit UTF-8 commit strings. A \
                 non-UTF-8 locale (C / POSIX / ISO-8859-*) mangles the \
                 output at the libc boundary. Generate a UTF-8 locale and \
                 set it as the default."
                .to_owned(),
            commands: vec![
                "# Debian/Ubuntu:".to_owned(),
                "sudo locale-gen en_US.UTF-8 vi_VN.UTF-8".to_owned(),
                "sudo update-locale LANG=en_US.UTF-8".to_owned(),
                "# Fedora/Arch (systemd):".to_owned(),
                "sudo localectl set-locale LANG=en_US.UTF-8".to_owned(),
            ],
            safe_to_run_unattended: true,
            references: vec![
                "https://wiki.archlinux.org/title/Locale".to_owned(),
            ],
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
        for id in [
            "VR001", "VR002", "VR003", "VR004", "VR005", "VR006", "VR007", "VR008", "VR009",
            "VR010", "VR011", "VR013", "VR014",
        ] {
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
    fn vr012_is_intentionally_absent() {
        // VD012 is Info-only and has no fix attached — the empty slot
        // keeps VR ids aligned with VD ids for the entries that DO have
        // a fix. Asserting here prevents a future copy-paste from
        // accidentally introducing a VR012 entry.
        assert!(lookup("VR012").is_none(), "VD012 is Info-only; VR012 must not exist");
    }

    #[test]
    fn all_has_thirteen_entries() {
        assert_eq!(all().len(), CATALOGUE_LEN);
    }

    #[test]
    fn unattended_flags_match_spec() {
        // Fixes that require picking between IBus and Fcitx5 (or running
        // config tools, editing config files, or removing a Snap) are
        // NOT safe to run unattended. Pure env-var tweaks and Ozone
        // flags ARE.
        assert!(!lookup("VR001").expect("VR001").safe_to_run_unattended);
        assert!(!lookup("VR002").expect("VR002").safe_to_run_unattended);
        assert!(!lookup("VR003").expect("VR003").safe_to_run_unattended);
        assert!(lookup("VR004").expect("VR004").safe_to_run_unattended);
        assert!(!lookup("VR005").expect("VR005").safe_to_run_unattended);
        assert!(!lookup("VR006").expect("VR006").safe_to_run_unattended);
        assert!(lookup("VR007").expect("VR007").safe_to_run_unattended);
        assert!(lookup("VR008").expect("VR008").safe_to_run_unattended);
        assert!(!lookup("VR009").expect("VR009").safe_to_run_unattended);
        assert!(!lookup("VR010").expect("VR010").safe_to_run_unattended);
        assert!(!lookup("VR011").expect("VR011").safe_to_run_unattended);
        assert!(!lookup("VR013").expect("VR013").safe_to_run_unattended);
        assert!(lookup("VR014").expect("VR014").safe_to_run_unattended);
    }
}
