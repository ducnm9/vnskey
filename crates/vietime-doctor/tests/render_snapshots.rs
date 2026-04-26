// SPDX-License-Identifier: GPL-3.0-or-later
//
// Week-7 DOC-61 — insta snapshot fixtures covering the five distros we
// claim to support in Phase 1 (Ubuntu 24.04, Fedora 40, Arch, Debian 12,
// openSUSE Tumbleweed). Each snapshot renders a pre-built `Report` through
// the same template the CLI uses, so any regression in the renderer,
// checker catalogue, or recommendation copy shows up as a diff.
//
// Why integration tests (not unit tests in `render.rs`)? The per-distro
// fixtures combine every subsystem's facts (system / IM / env / apps /
// issues / recommendations), which would dwarf the renderer's own unit
// tests. Keeping the big fixtures here keeps `render.rs` readable.
//
// Spec ref: `spec/01-phase1-doctor.md` §A.5, §B.15 (release fixtures).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::HashMap;

use chrono::{TimeZone, Utc};
use vietime_core::{
    desktop::DesktopEnv,
    distro::{Distro, DistroFamily},
    engine::{EngineFact, Fcitx5Facts, IbusFacts},
    session::SessionType,
    ActiveFramework, AppFacts, AppKind, EnvFacts, EnvSource, Facts, ImFacts, ImFramework, Issue,
    Recommendation, Report, Severity, SystemFacts, REPORT_SCHEMA_VERSION,
};
use vietime_doctor::render::{render, RenderOptions};

/// Build an empty `Report` with a fixed `generated_at` so snapshots are
/// deterministic. Call sites fill in `facts` / `issues` / `recommendations`.
fn base(tool_version: &str) -> Report {
    Report {
        schema_version: REPORT_SCHEMA_VERSION,
        generated_at: Utc.with_ymd_and_hms(2026, 4, 25, 10, 23, 11).unwrap(),
        tool_version: tool_version.to_owned(),
        facts: Facts::default(),
        issues: vec![],
        recommendations: vec![],
        anomalies: vec![],
    }
}

fn env_from_pairs(pairs: &[(&str, &str)], src: EnvSource) -> EnvFacts {
    let map: HashMap<String, String> =
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    EnvFacts::from_env_with_source(&map, src)
}

fn ibus_engine(name: &str, registered: bool) -> EngineFact {
    EngineFact {
        name: name.to_owned(),
        package: Some(format!("ibus-{name}")),
        version: None,
        framework: ImFramework::Ibus,
        is_vietnamese: true,
        is_registered: registered,
    }
}

fn fcitx_engine(name: &str, registered: bool) -> EngineFact {
    EngineFact {
        name: name.to_owned(),
        package: Some(format!("fcitx5-{name}")),
        version: None,
        framework: ImFramework::Fcitx5,
        is_vietnamese: true,
        is_registered: registered,
    }
}

// ─── Ubuntu 24.04 — clean IBus ───────────────────────────────────────────

fn ubuntu_24_04() -> Report {
    let mut r = base("0.1.0");
    r.facts.system = SystemFacts {
        distro: Some(Distro {
            id: "ubuntu".to_owned(),
            version_id: Some("24.04".to_owned()),
            pretty: Some("Ubuntu 24.04 LTS".to_owned()),
            family: DistroFamily::Debian,
            id_like: vec!["debian".to_owned()],
        }),
        desktop: Some(DesktopEnv::Gnome { version: Some("46".to_owned()) }),
        session: Some(SessionType::Wayland),
        kernel: Some("6.8.0-45-generic".to_owned()),
        shell: Some("zsh".to_owned()),
        locale: Some("en_US.UTF-8".to_owned()),
    };
    r.facts.im = ImFacts {
        active_framework: ActiveFramework::Ibus,
        ibus: Some(IbusFacts {
            version: Some("1.5.29".to_owned()),
            daemon_running: true,
            daemon_pid: Some(2341),
            config_dir: None,
            registered_engines: vec!["Bamboo".to_owned()],
        }),
        fcitx5: None,
        engines: vec![ibus_engine("Bamboo", true)],
    };
    r.facts.env = env_from_pairs(
        &[
            ("GTK_IM_MODULE", "ibus"),
            ("QT_IM_MODULE", "ibus"),
            ("XMODIFIERS", "@im=ibus"),
            ("SDL_IM_MODULE", "ibus"),
            ("INPUT_METHOD", "ibus"),
        ],
        EnvSource::Process,
    );
    r
}

// ─── Fedora 40 — Fcitx5 on Wayland, clean ────────────────────────────────

fn fedora_40() -> Report {
    let mut r = base("0.1.0");
    r.facts.system = SystemFacts {
        distro: Some(Distro {
            id: "fedora".to_owned(),
            version_id: Some("40".to_owned()),
            pretty: Some("Fedora Linux 40 (Workstation Edition)".to_owned()),
            family: DistroFamily::Redhat,
            id_like: vec![],
        }),
        desktop: Some(DesktopEnv::Gnome { version: Some("46".to_owned()) }),
        session: Some(SessionType::Wayland),
        kernel: Some("6.8.9-300.fc40.x86_64".to_owned()),
        shell: Some("bash".to_owned()),
        locale: Some("en_US.UTF-8".to_owned()),
    };
    r.facts.im = ImFacts {
        active_framework: ActiveFramework::Fcitx5,
        ibus: None,
        fcitx5: Some(Fcitx5Facts {
            version: Some("5.1.12".to_owned()),
            daemon_running: true,
            daemon_pid: Some(1821),
            config_dir: None,
            addons_enabled: vec!["wayland-im".to_owned()],
            input_methods_configured: vec!["bamboo".to_owned()],
        }),
        engines: vec![fcitx_engine("bamboo", true)],
    };
    r.facts.env = env_from_pairs(
        &[
            ("GTK_IM_MODULE", "fcitx"),
            ("QT_IM_MODULE", "fcitx"),
            ("XMODIFIERS", "@im=fcitx"),
            ("SDL_IM_MODULE", "fcitx"),
            ("INPUT_METHOD", "fcitx"),
        ],
        EnvSource::EtcEnvironment,
    );
    r
}

// ─── Arch — framework conflict + env disagreement ────────────────────────

fn arch_conflict() -> Report {
    let mut r = base("0.1.0");
    r.facts.system = SystemFacts {
        distro: Some(Distro {
            id: "arch".to_owned(),
            version_id: None,
            pretty: Some("Arch Linux".to_owned()),
            family: DistroFamily::Arch,
            id_like: vec![],
        }),
        desktop: Some(DesktopEnv::Kde { version: Some("6.0".to_owned()) }),
        session: Some(SessionType::X11),
        kernel: Some("6.9.1-arch1-1".to_owned()),
        shell: Some("fish".to_owned()),
        locale: Some("en_US.UTF-8".to_owned()),
    };
    r.facts.im = ImFacts {
        active_framework: ActiveFramework::Conflict,
        ibus: Some(IbusFacts {
            version: Some("1.5.30".to_owned()),
            daemon_running: true,
            daemon_pid: Some(1111),
            config_dir: None,
            registered_engines: vec!["Bamboo".to_owned()],
        }),
        fcitx5: Some(Fcitx5Facts {
            version: Some("5.1.12".to_owned()),
            daemon_running: true,
            daemon_pid: Some(2222),
            config_dir: None,
            addons_enabled: vec!["xim".to_owned()],
            input_methods_configured: vec!["bamboo".to_owned()],
        }),
        engines: vec![ibus_engine("Bamboo", true), fcitx_engine("bamboo", true)],
    };
    // Classic VD003 trigger: env says fcitx, active is Conflict.
    let mut env = env_from_pairs(&[("GTK_IM_MODULE", "fcitx")], EnvSource::Process);
    let etc = env_from_pairs(
        &[("QT_IM_MODULE", "ibus"), ("XMODIFIERS", "@im=ibus")],
        EnvSource::EtcEnvironment,
    );
    env.merge_by_priority(&etc);
    r.facts.env = env;
    r.issues.push(Issue {
        id: "VD002".to_owned(),
        severity: Severity::Error,
        title: "Both IBus and Fcitx5 daemons are running".to_owned(),
        detail: "Only one IM framework should own the IM socket at a time.".to_owned(),
        facts_evidence: vec![
            "ibus-daemon: running (pid 1111)".to_owned(),
            "fcitx5: running (pid 2222)".to_owned(),
        ],
        recommendation: Some("VR002".to_owned()),
    });
    r.issues.push(Issue {
        id: "VD003".to_owned(),
        severity: Severity::Error,
        title: "IM env vars disagree with the active framework".to_owned(),
        detail: "GTK_IM_MODULE points at fcitx but QT_IM_MODULE / XMODIFIERS point at ibus."
            .to_owned(),
        facts_evidence: vec![
            "GTK_IM_MODULE=fcitx (process)".to_owned(),
            "QT_IM_MODULE=ibus (/etc/environment)".to_owned(),
        ],
        recommendation: Some("VR003".to_owned()),
    });
    r.recommendations.push(Recommendation {
        id: "VR002".to_owned(),
        title: "Stop the non-preferred IM daemon".to_owned(),
        description: "Pick one framework and disable the other.".to_owned(),
        commands: vec!["systemctl --user disable --now ibus".to_owned()],
        safe_to_run_unattended: false,
        references: vec![],
    });
    r.recommendations.push(Recommendation {
        id: "VR003".to_owned(),
        title: "Align IM env vars with the active framework".to_owned(),
        description: "Set GTK/QT/XMODIFIERS/SDL_IM_MODULE to one consistent value.".to_owned(),
        commands: vec!["export GTK_IM_MODULE=fcitx".to_owned()],
        safe_to_run_unattended: false,
        references: vec![],
    });
    r
}

// ─── Debian 12 — Electron app missing Ozone ──────────────────────────────

fn debian_12_electron() -> Report {
    let mut r = base("0.1.0");
    r.facts.system = SystemFacts {
        distro: Some(Distro {
            id: "debian".to_owned(),
            version_id: Some("12".to_owned()),
            pretty: Some("Debian GNU/Linux 12 (bookworm)".to_owned()),
            family: DistroFamily::Debian,
            id_like: vec![],
        }),
        desktop: Some(DesktopEnv::Gnome { version: Some("43".to_owned()) }),
        session: Some(SessionType::Wayland),
        kernel: Some("6.1.0-20-amd64".to_owned()),
        shell: Some("bash".to_owned()),
        locale: Some("en_US.UTF-8".to_owned()),
    };
    r.facts.im = ImFacts {
        active_framework: ActiveFramework::Fcitx5,
        ibus: None,
        fcitx5: Some(Fcitx5Facts {
            version: Some("5.0.8".to_owned()),
            daemon_running: true,
            daemon_pid: Some(2114),
            config_dir: None,
            addons_enabled: vec!["wayland-im".to_owned()],
            input_methods_configured: vec!["bamboo".to_owned()],
        }),
        engines: vec![fcitx_engine("bamboo", true)],
    };
    r.facts.env = env_from_pairs(
        &[
            ("GTK_IM_MODULE", "fcitx"),
            ("QT_IM_MODULE", "fcitx"),
            ("XMODIFIERS", "@im=fcitx"),
            ("SDL_IM_MODULE", "fcitx"),
            ("INPUT_METHOD", "fcitx"),
        ],
        EnvSource::EtcEnvironment,
    );
    r.facts.apps = vec![AppFacts {
        app_id: "vscode".to_owned(),
        binary_path: std::path::PathBuf::from("/usr/bin/code"),
        version: Some("1.89.0".to_owned()),
        kind: AppKind::Electron,
        electron_version: Some("28.2.10".to_owned()),
        uses_wayland: Some(false),
        detector_notes: vec!["no --ozone-platform=wayland flag seen".to_owned()],
    }];
    r.issues.push(Issue {
        id: "VD007".to_owned(),
        severity: Severity::Error,
        title: "Electron app running without Ozone/Wayland".to_owned(),
        detail: "vscode is an Electron app but was launched without \
                 `--ozone-platform=wayland`."
            .to_owned(),
        facts_evidence: vec!["vscode: Electron 28.2.10".to_owned(), "session: wayland".to_owned()],
        recommendation: Some("VR007".to_owned()),
    });
    r.recommendations.push(Recommendation {
        id: "VR007".to_owned(),
        title: "Launch Electron apps with native Wayland input".to_owned(),
        description: "Add the Ozone/Wayland flags to your launcher.".to_owned(),
        commands: vec![
            "code --ozone-platform=wayland --enable-features=UseOzonePlatform".to_owned()
        ],
        safe_to_run_unattended: true,
        references: vec!["https://wiki.archlinux.org/title/Wayland#Electron".to_owned()],
    });
    r
}

// ─── openSUSE Tumbleweed — missing SDL_IM_MODULE ─────────────────────────

fn opensuse_tumbleweed() -> Report {
    let mut r = base("0.1.0");
    r.facts.system = SystemFacts {
        distro: Some(Distro {
            id: "opensuse-tumbleweed".to_owned(),
            version_id: Some("20260420".to_owned()),
            pretty: Some("openSUSE Tumbleweed".to_owned()),
            family: DistroFamily::Suse,
            id_like: vec!["opensuse".to_owned()],
        }),
        desktop: Some(DesktopEnv::Kde { version: Some("6.0".to_owned()) }),
        session: Some(SessionType::Wayland),
        kernel: Some("6.9.1-1-default".to_owned()),
        shell: Some("bash".to_owned()),
        locale: Some("en_US.UTF-8".to_owned()),
    };
    r.facts.im = ImFacts {
        active_framework: ActiveFramework::Fcitx5,
        ibus: None,
        fcitx5: Some(Fcitx5Facts {
            version: Some("5.1.12".to_owned()),
            daemon_running: true,
            daemon_pid: Some(3310),
            config_dir: None,
            addons_enabled: vec!["wayland-im".to_owned()],
            input_methods_configured: vec!["bamboo".to_owned()],
        }),
        engines: vec![fcitx_engine("bamboo", true)],
    };
    // No SDL_IM_MODULE in env — VD004 fodder.
    r.facts.env = env_from_pairs(
        &[("GTK_IM_MODULE", "fcitx"), ("QT_IM_MODULE", "fcitx"), ("XMODIFIERS", "@im=fcitx")],
        EnvSource::Process,
    );
    r.issues.push(Issue {
        id: "VD004".to_owned(),
        severity: Severity::Warn,
        title: "SDL_IM_MODULE is not set".to_owned(),
        detail: "Games and SDL-based apps won't route IME events without this variable.".to_owned(),
        facts_evidence: vec!["SDL_IM_MODULE: unset".to_owned(), "active: fcitx5".to_owned()],
        recommendation: Some("VR004".to_owned()),
    });
    r.recommendations.push(Recommendation {
        id: "VR004".to_owned(),
        title: "Export SDL_IM_MODULE".to_owned(),
        description: "Persist SDL_IM_MODULE alongside the other IM env vars.".to_owned(),
        commands: vec!["export SDL_IM_MODULE=fcitx".to_owned()],
        safe_to_run_unattended: true,
        references: vec![],
    });
    r
}

#[test]
fn snapshot_distro_ubuntu_24_04_ibus_clean() {
    let out = render(&ubuntu_24_04(), &RenderOptions::default()).expect("render");
    insta::assert_snapshot!("distro__ubuntu_24_04_ibus_clean", out);
}

#[test]
fn snapshot_distro_fedora_40_fcitx5_clean() {
    let out = render(&fedora_40(), &RenderOptions::default()).expect("render");
    insta::assert_snapshot!("distro__fedora_40_fcitx5_clean", out);
}

#[test]
fn snapshot_distro_arch_framework_conflict() {
    let out = render(&arch_conflict(), &RenderOptions::default()).expect("render");
    insta::assert_snapshot!("distro__arch_framework_conflict", out);
}

#[test]
fn snapshot_distro_debian_12_electron_no_ozone() {
    let out = render(&debian_12_electron(), &RenderOptions::default()).expect("render");
    insta::assert_snapshot!("distro__debian_12_electron_no_ozone", out);
}

#[test]
fn snapshot_distro_opensuse_tumbleweed_missing_sdl() {
    let out = render(&opensuse_tumbleweed(), &RenderOptions::default()).expect("render");
    insta::assert_snapshot!("distro__opensuse_tumbleweed_missing_sdl", out);
}

#[test]
fn plain_variant_stays_deterministic_for_ubuntu() {
    // One plain-text snapshot is enough — it confirms the `strip_markdown`
    // branch stays in sync with the template. The other four distros keep
    // markdown snapshots so we don't bloat the fixture footprint.
    let out =
        render(&ubuntu_24_04(), &RenderOptions { plain: true, verbose: false }).expect("render");
    insta::assert_snapshot!("distro__ubuntu_24_04_ibus_clean_plain", out);
}
