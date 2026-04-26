# Phase 1 — VietIME Doctor v0.1 (Tháng 2–3, 8 tuần)

> **Goal**: release Doctor v0.1.0 Flatpak + GitHub, chẩn đoán được setup IME trên 5 distro.
>
> **Exit criteria**: spec/01 §B.14 (acceptance checklist).
>
> **Budget**: 80–120h.

---

## Week 1 — Core foundation & basic detectors

### DOC-01 — CLI skeleton với clap subcommands
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: P0-14
- **Spec ref**: spec/01 §A.4
- **Acceptance**:
  - [ ] `vietime-doctor --help` show subcommands: `(default)`, `check`, `list`, `diagnose`, `report`, `version`.
  - [ ] Global flags: `--json`, `--plain`, `--verbose`, `--no-redact`, `--app`.
  - [ ] Exit codes 0/1/2/64/70 như spec.
  - [ ] Clap derive API, `clap` v4.
  - [ ] Integration test: invoke subcommands, check exit code.

### DOC-02 — `Detector` trait + orchestrator skeleton
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (6h)
- **Depends on**: DOC-01
- **Spec ref**: spec/01 §B.1, §B.3
- **Acceptance**:
  - [ ] `async_trait Detector` với `id()`, `timeout()`, `run()`.
  - [ ] `Orchestrator::run_all()` spawn detectors qua `tokio::task::JoinSet` với per-detector timeout.
  - [ ] Detector fail không crash; collect vào `Report.anomalies`.
  - [ ] Unit test: 2 mock detector (1 ok, 1 panic) → orchestrator hoàn thành, ghi anomaly.

### DOC-03 — `DistroDetector` + `SessionDetector` + `DesktopDetector`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: DOC-02
- **Spec ref**: spec/01 §B.3 (`sys.distro`, `sys.session`, `sys.desktop`)
- **Acceptance**:
  - [ ] DistroDetector parse `/etc/os-release` cho Ubuntu 22/24, Debian, Fedora, Arch, Pop!_OS → fixture tests.
  - [ ] SessionDetector đọc `$XDG_SESSION_TYPE` → X11/Wayland/Unknown.
  - [ ] DesktopDetector đọc `$XDG_CURRENT_DESKTOP` → GNOME/KDE/XFCE + version (best-effort).
  - [ ] Cả 3 impl `Detector` trait.

### DOC-04 — `KernelDetector` + `ShellDetector`
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (2h)
- **Depends on**: DOC-02
- **Spec ref**: spec/01 §B.3
- **Acceptance**:
  - [ ] `uname -r` via `nix::sys::utsname` (không spawn subprocess).
  - [ ] Shell detect qua `$SHELL` + fallback `getent passwd`.
  - [ ] Tests.

---

## Week 2 — Env var detectors & rendering

### DOC-10 — `ProcessEnvDetector`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: DOC-02
- **Spec ref**: spec/01 §B.3 (`env.process`)
- **Acceptance**:
  - [ ] Đọc env của process hiện tại → extract keys: `GTK_IM_MODULE`, `QT_IM_MODULE`, `QT4_IM_MODULE`, `XMODIFIERS`, `INPUT_METHOD`, `SDL_IM_MODULE`, `GLFW_IM_MODULE`, `CLUTTER_IM_MODULE`.
  - [ ] Emit `EnvFacts` + `sources[key] = Process`.
  - [ ] Tests với env giả lập.

### DOC-11 — `EtcEnvironmentDetector`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: DOC-10
- **Spec ref**: spec/01 §B.3 (`env.etc_environment`); spec/02 §B.8
- **Acceptance**:
  - [ ] Parser line-based cho `/etc/environment`, preserve comments.
  - [ ] Dùng `shell-words` crate để handle quoting.
  - [ ] 20 fixture edge case (empty, double quote, single quote, escaped, comment inline).
  - [ ] Merge vào `EnvFacts` với source = `EtcEnvironment`, **không overwrite** Process source (Process có priority cao hơn).

### DOC-12 — `HomeProfileDetector`
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: DOC-11
- **Spec ref**: spec/01 §B.3 (`env.home_profile`)
- **Acceptance**:
  - [ ] Grep `~/.profile`, `~/.bashrc`, `~/.zshrc`, `~/.config/environment.d/*.conf` cho 8 key IM.
  - [ ] Handle shell export syntax: `export KEY=value`, `KEY=value`, `export KEY="value"`.
  - [ ] Tests.

### DOC-13 — `EtcProfileDDetector` + `SystemdEnvDetector`
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (3h)
- **Depends on**: DOC-11
- **Spec ref**: spec/01 §B.3
- **Acceptance**:
  - [ ] Grep `/etc/profile.d/*.sh`.
  - [ ] Run `systemctl --user show-environment`, parse output.
  - [ ] Merge với source tương ứng.
  - [ ] Test mock subprocess.

### DOC-14 — Markdown renderer (MVP)
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: DOC-03, DOC-10, DOC-11
- **Spec ref**: spec/01 §A.5, §B.7
- **Acceptance**:
  - [ ] `minijinja` template render `Report` → markdown giống spec §A.5.
  - [ ] `insta` snapshot test cho 3 fixture Report.
  - [ ] `--plain` strip markdown formatting.
  - [ ] `--json` serialize qua `serde_json::to_string_pretty`.

---

## Week 3 — IM framework detection

### DOC-20 — `IbusDaemonDetector` via D-Bus
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (8h)
- **Depends on**: DOC-02
- **Spec ref**: spec/01 §B.3 (`im.ibus.daemon`); spec/00 §10 (A5)
- **Acceptance**:
  - [ ] `zbus` connect session bus, query `org.freedesktop.IBus` interface existence.
  - [ ] Fallback `pgrep ibus-daemon` nếu D-Bus không có interface.
  - [ ] Extract version qua `ibus --version` subprocess (timeout 1s).
  - [ ] Emit `IbusFacts { version, daemon_running, daemon_pid, config_dir }`.
  - [ ] Test với mock D-Bus (sử dụng `zbus_macros::dbus_interface` mock server trong test).

### DOC-21 — `IbusEnginesDetector`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: DOC-20
- **Spec ref**: spec/01 §B.3 (`im.ibus.engines`)
- **Acceptance**:
  - [ ] Spawn `ibus list-engine` (timeout 2s).
  - [ ] Parse output → `Vec<EngineFact>` với `is_vietnamese` flag cho bamboo/unikey.
  - [ ] Test với fixture output.

### DOC-22 — `Fcitx5DaemonDetector`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (6h)
- **Depends on**: DOC-02
- **Spec ref**: spec/01 §B.3 (`im.fcitx5.daemon`)
- **Acceptance**:
  - [ ] `zbus` query `org.fcitx.Fcitx5`.
  - [ ] Fallback `pgrep fcitx5`.
  - [ ] Version qua `fcitx5 --version`.
  - [ ] Emit `Fcitx5Facts`.

### DOC-23 — `Fcitx5ConfigDetector`
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: DOC-22
- **Spec ref**: spec/01 §B.3 (`im.fcitx5.config`)
- **Acceptance**:
  - [ ] Parse `~/.config/fcitx5/profile` (INI-like format) → `input_methods_configured`.
  - [ ] Đọc `~/.local/share/fcitx5/addon/` hoặc `~/.config/fcitx5/conf/*.conf` → `addons_enabled`.
  - [ ] Fixture tests.

### DOC-24 — `PackageEnginesDetector`
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (5h)
- **Depends on**: DOC-03
- **Spec ref**: spec/01 §B.3 (`im.engines.packages`)
- **Acceptance**:
  - [ ] Detect package manager từ `Distro` (apt/dnf/pacman).
  - [ ] Query package list: `dpkg -l | grep`, `rpm -qa | grep`, `pacman -Q`.
  - [ ] Filter packages: ibus-bamboo, ibus-unikey, fcitx5-bamboo, fcitx5-unikey.
  - [ ] Emit `EngineFact[]` với `package`, `version`, `is_registered = false` (registered set bởi DOC-21/23).

---

## Week 4 — App-specific detection

### DOC-30 — `AppProfile` registry + resolver
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: DOC-01
- **Spec ref**: spec/01 §B.5
- **Acceptance**:
  - [ ] Hardcoded registry với 10 profile (vscode, chrome, firefox, slack, discord, obsidian, telegram, libreoffice, intellij, neovide).
  - [ ] `resolve(name_or_path) -> Option<AppProfile>` handle alias + absolute path.
  - [ ] User config `~/.config/vietime/apps.toml` override (optional).

### DOC-31 — `GenericAppDetector`
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (3h)
- **Depends on**: DOC-30
- **Spec ref**: spec/01 §B.3 (`app.generic`)
- **Acceptance**:
  - [ ] Từ AppProfile, resolve binary path (which).
  - [ ] `file` command detect ELF/script.
  - [ ] Version best-effort qua `--version`.
  - [ ] Emit `AppFacts`.

### DOC-32 — `ElectronAppDetector`
- **Status**: TODO
- **Priority**: P1
- **Estimate**: L (6h)
- **Depends on**: DOC-31
- **Spec ref**: spec/01 §B.3 (`app.electron`)
- **Acceptance**:
  - [ ] Detect `.asar` hoặc `resources/app.asar`.
  - [ ] Extract Electron version từ binary strings hoặc `package.json` trong asar.
  - [ ] Detect đang chạy với process flag (`/proc/<pid>/cmdline`) tìm `--enable-features=UseOzonePlatform`.
  - [ ] `uses_wayland: Option<bool>`.

### DOC-33 — `--app <X>` CLI integration
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (3h)
- **Depends on**: DOC-30, DOC-31, DOC-32
- **Spec ref**: spec/01 §A.4, §B.5
- **Acceptance**:
  - [ ] `vietime-doctor --app vscode` chạy extra detectors.
  - [ ] Report có section "App-specific" với `AppFacts`.
  - [ ] Test E2E với VS Code installed.

---

## Week 5 — Checkers core

### DOC-40 — Checker trait + engine
- **Status**: DONE
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: DOC-24
- **Spec ref**: spec/01 §B.4
- **Acceptance**:
  - [x] `fn check(&Facts) -> Vec<Issue>` pure function.
  - [x] Registry của checkers, orchestrator chạy tất cả sau detectors.
  - [x] Severity ordering: Info < Warn < Error < Critical.

### DOC-41 — VD001 NoImFrameworkActive
- **Status**: DONE
- **Priority**: P0
- **Estimate**: S (2h)
- **Depends on**: DOC-40
- **Spec ref**: spec/01 §B.4
- **Acceptance**:
  - [x] Trigger khi không có daemon chạy + có Vietnamese engine.
  - [x] Severity Critical.
  - [x] Recommendation VR001 gợi ý start daemon.

### DOC-42 — VD002 ImFrameworkConflict
- **Status**: DONE
- **Priority**: P0
- **Estimate**: S (2h)
- **Depends on**: DOC-40
- **Spec ref**: spec/01 §B.4
- **Acceptance**:
  - [x] Trigger khi cả IBus và Fcitx5 daemon đều running.
  - [x] Severity Error.
  - [x] VR002: gợi ý stop 1 trong 2.

### DOC-43 — VD003 EnvVarMismatch
- **Status**: DONE
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: DOC-40
- **Spec ref**: spec/01 §B.4
- **Acceptance**:
  - [x] Trigger khi `GTK_IM_MODULE=ibus` nhưng active framework là Fcitx5 (hoặc ngược).
  - [x] Severity Error.
  - [x] VR003: show các env sai + gợi ý giá trị đúng.

### DOC-44 — VD004 MissingSdlImModule + VD012 LegacyImSettingEmpty
- **Status**: DONE
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: DOC-40
- **Spec ref**: spec/01 §B.4
- **Acceptance**:
  - [x] VD004 Warn khi `SDL_IM_MODULE` unset.
  - [x] VD012 Info khi `INPUT_METHOD` unset.
  - [x] VR tương ứng.

### DOC-45 — VD005 EngineInstalledNotRegistered
- **Status**: DONE
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: DOC-40
- **Spec ref**: spec/01 §B.4
- **Acceptance**:
  - [x] Trigger khi package installed nhưng không register trong IBus/Fcitx5 config.
  - [x] Severity Warn.

### DOC-46 — VD006 + VD007 + VD008 (Wayland/Electron/Chrome)
- **Status**: DONE
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: DOC-32, DOC-40
- **Spec ref**: spec/01 §B.4
- **Acceptance**:
  - [x] VD006: Wayland + IBus → Warn, gợi ý Fcitx5.
  - [x] VD007: Electron app no Ozone flags → Error (chỉ chạy khi `--app`).
  - [x] VD008: Chrome chạy X11 backend trên Wayland → Warn.

---

## Week 6 — Polish checkers + JSON schema + PII

### DOC-50 — VD009 → VD015 còn lại
- **Status**: TODO
- **Priority**: P2
- **Estimate**: M (5h)
- **Depends on**: DOC-40
- **Spec ref**: spec/01 §B.4
- **Acceptance**:
  - [ ] VD009 EnvConflictBetweenFiles.
  - [ ] VD010 VsCodeSnapDetected.
  - [ ] VD011 FlatpakAppNoImPortal.
  - [ ] VD013 FcitxAddonDisabled.
  - [ ] VD014 UnicodeLocaleMissing.
  - [ ] VD015 NoVietnameseEngineInstalled.
  - [ ] Mỗi checker có test.

### DOC-51 — PII redactor
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: DOC-14
- **Spec ref**: spec/01 §B.6
- **Acceptance**:
  - [ ] Redact: username, hostname, machine-id UUID, IPs, `*_TOKEN`/`*_KEY` values.
  - [ ] `--no-redact` opt-out.
  - [ ] Fuzz test: feed random data, verify no unreadcted username.
  - [ ] Apply trên Markdown + JSON + plain.

### DOC-52 — JSON schema v1 publish
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (3h)
- **Depends on**: DOC-14
- **Spec ref**: spec/01 §B.14
- **Acceptance**:
  - [ ] `schemas/report.v1.json` JSON Schema file.
  - [ ] CI step validate golden JSON reports against schema.
  - [ ] Docs page linking schema.

### DOC-53 — Recommendation engine
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (3h)
- **Depends on**: DOC-41..DOC-46, DOC-50
- **Spec ref**: spec/01 §A.5, §B.4
- **Acceptance**:
  - [ ] Aggregate issues → unique recommendations (VR###).
  - [ ] Render trong Markdown section "Recommendations" với copy-paste commands.

### DOC-54 — `check` subcommand CI-friendly
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: DOC-53
- **Spec ref**: spec/01 §A.4
- **Acceptance**:
  - [ ] `vietime-doctor check` prints 1-line status, exit code 0/1/2.
  - [ ] Benchmark: < 500ms trên healthy system.

---

## Week 7 — Testing, docs, polish

### DOC-60 — CI matrix integration tests
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (8h)
- **Depends on**: DOC-54
- **Spec ref**: spec/01 §B.10 (#3)
- **Acceptance**:
  - [ ] GH Actions matrix: ubuntu-22.04, ubuntu-24.04, fedora:40 container, archlinux:latest container.
  - [ ] Run `vietime-doctor --json` → validate schema.
  - [ ] Assert no panic, reasonable exit code.

### DOC-61 — Snapshot tests (insta) 5 fixture distros
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (5h)
- **Depends on**: DOC-14
- **Spec ref**: spec/01 §B.10 (#2)
- **Acceptance**:
  - [ ] 5 fixture `tests/fixtures/<distro>/` chứa fake `/etc/os-release`, `/etc/environment`, env dump.
  - [ ] Golden markdown + JSON.
  - [ ] `cargo insta review` workflow.

### DOC-62 — Fuzz test env parser
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (3h)
- **Depends on**: DOC-11
- **Spec ref**: spec/01 §B.10 (#4)
- **Acceptance**:
  - [ ] `cargo-fuzz` target cho `parse_etc_environment`.
  - [ ] Run 60s trong CI, no panic.

### DOC-63 — User docs vi + en
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (6h)
- **Depends on**: DOC-54
- **Spec ref**: spec/04 §6
- **Acceptance**:
  - [ ] `docs/vi/doctor.md`: quickstart, ví dụ report, troubleshooting.
  - [ ] `docs/en/doctor.md`: mirror.
  - [ ] `docs/vi/glossary.md`: IBus, Fcitx5, GTK_IM_MODULE giải thích.
  - [ ] README link.

### DOC-64 — Dry run với maintainer
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: DOC-63
- **Spec ref**: spec/01 §B.14 (#7)
- **Acceptance**:
  - [ ] Gửi binary preview cho maintainer ibus-bamboo / fcitx5-bamboo.
  - [ ] Xin feedback trên 3 bug report thật trong backlog của họ.
  - [ ] Iterate dựa trên feedback.

---

## Week 8 — Release

### DOC-70 — Binary strip + size check
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: DOC-60
- **Spec ref**: spec/01 §B.14
- **Acceptance**:
  - [ ] Release build < 8 MB sau strip (cross-compile x86_64 + aarch64).
  - [ ] Profile `release` với `lto = "thin"`, `strip = true`, `panic = "abort"`.
  - [ ] CI assert binary size.

### DOC-71 — `cargo-deb` package
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: DOC-70
- **Spec ref**: spec/01 §B.11
- **Acceptance**:
  - [ ] `.deb` build via `cargo-deb` trong CI release workflow.
  - [ ] Install + `vietime-doctor` run trên Ubuntu 22/24 VM sạch.
  - [ ] Uninstall clean.

### DOC-72 — Flatpak manifest + Flathub submit
- **Status**: TODO
- **Priority**: P1
- **Estimate**: L (8h)
- **Depends on**: DOC-70
- **Spec ref**: spec/01 §B.11; spec/04 §10.3
- **Acceptance**:
  - [ ] `packaging/flatpak/io.github.vietime.Doctor.yaml` manifest.
  - [ ] Build local với `flatpak-builder`, run → report đúng.
  - [ ] PR submit tới `flathub/flathub` repo.
  - [ ] Chờ review: document status trong `docs/dev/flathub-status.md`.

### DOC-73 — AUR PKGBUILD
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (2h)
- **Depends on**: DOC-70
- **Spec ref**: spec/01 §B.11
- **Acceptance**:
  - [ ] `packaging/aur/PKGBUILD-bin` (prebuilt binary).
  - [ ] `packaging/aur/PKGBUILD-git` (build from source).
  - [ ] Test trên Arch container.

### DOC-74 — GH Release workflow
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: DOC-71
- **Spec ref**: spec/04 §7.3, §7.4
- **Acceptance**:
  - [ ] `.github/workflows/release.yml` trigger on tag `v*`.
  - [ ] Build binary x86_64 + aarch64.
  - [ ] Upload: `.tar.gz`, `.deb`, `SHA256SUMS`, GPG signature.
  - [ ] Draft release notes từ CHANGELOG.

### DOC-75 — CHANGELOG v0.1.0 + version bump
- **Status**: TODO
- **Priority**: P0
- **Estimate**: XS (1h)
- **Depends on**: DOC-70
- **Spec ref**: spec/04 §7.3
- **Acceptance**:
  - [ ] `CHANGELOG.md` Keep-a-Changelog format.
  - [ ] Version `0.1.0` trong `crates/vietime-doctor/Cargo.toml`.
  - [ ] Tag `v0.1.0-doctor`.

### DOC-76 — Blog post #2 + social launch
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: DOC-74
- **Spec ref**: spec/05 §2 (Phase 1 release)
- **Acceptance**:
  - [ ] Blog post "Giới thiệu VietIME Doctor — chẩn đoán gõ tiếng Việt trên Linux".
  - [ ] Demo gif/screenshot.
  - [ ] Post vào Facebook nhóm + Reddit.
  - [ ] Track downloads trong 7 ngày.

---

## Phase 1 — Exit checklist (spec/01 §B.14)

- [ ] DOC-01 → DOC-33 (detectors + app-specific) ✅
- [ ] DOC-40 → DOC-53 (checkers + PII + JSON schema) ✅
- [ ] DOC-60 → DOC-64 (testing + docs + dry run) ✅
- [ ] DOC-70 → DOC-76 (release + social) ✅
- [ ] Acceptance spec/01 §B.14 đạt tất cả mục.

**Timebox rule**: cuối W6 nếu > 3 checker VD chưa xong → demote DOC-50 sang v0.2.
