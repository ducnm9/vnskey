<!--
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Changelog

All notable changes to the VietIME Suite are documented in this file.

The format follows [Keep a Changelog 1.1.0] and the project adheres to
[Semantic Versioning 2.0.0].

[Keep a Changelog 1.1.0]: https://keepachangelog.com/en/1.1.0/
[Semantic Versioning 2.0.0]: https://semver.org/spec/v2.0.0.html

## [Unreleased]

### Added

#### Bench CLI (`vietime-bench`) — Phase 3

- `run` subcommand: automated compatibility matrix runner across
  (engine x app x session x mode) combos with profile-driven expansion.
- `validate` subcommand: loads all test vectors and checks for duplicate
  IDs, empty fields, and Unicode NFC normalisation.
- `report` subcommand: renders results as JSON, Markdown tables, or
  colour-coded HTML dashboard.
- `compare` subcommand: diffs two runs with regression markers (>5% drop).
- `inspect` subcommand: shows failure details and reproducer commands.
- `list` subcommand: enumerates apps, sessions, modes, drivers, profiles.

#### Test vectors

- 500 Telex vectors (T001–T500) covering modifiers, tones, combined
  diacritics, common words, phrases, sentences, and edge cases.
- 25 bug regression vectors (BUG-001–BUG-025) with `known_failing_on`
  annotations referencing upstream issues.

#### Drivers & runners (9 apps, 2 sessions, 2 IM frameworks)

- Session drivers: X11 (Xvfb + openbox), Wayland (Weston headless).
- Keystroke injectors: xdotool (X11), ydotool/wtype (Wayland).
- IM drivers: IBus (ibus-daemon lifecycle, engine activation, gsettings
  mode switch), Fcitx5 (fcitx5 lifecycle, config file mode switch).
- App runners: gedit, kate, firefox, chromium, vscode, libreoffice,
  slack, discord, obsidian (Electron macro).
- Shared xdotool helper for window search, focus, select-all, clipboard
  capture.

#### Profiles

- Built-in profiles: `smoke` (3 combos), `full` (48 combos),
  `bugs` (4 combos).
- TOML-based custom profiles with Cartesian product expansion.

#### Scoring

- Per-vector exact match + Levenshtein edit distance via `strsim`.
- Aggregate accuracy percentage, weighted score, total edit distance.

#### Reliability

- Injection retry (3x with 200ms backoff).
- Capture retry (2x with 500ms delay for empty reads).
- Run result persistence: `runs/<id>/summary.json`, per-failure JSON,
  `latest` symlink.

#### CI

- Nightly bench workflow (`bench-nightly.yml`): cron 2am UTC +
  manual dispatch, 4-combo matrix, artifact upload, HTML gh-pages deploy.
- `validate-vectors` job added to main CI.
- Auto-label issue bot for component/type/engine classification.
- Bench result JSON Schema (`schemas/bench-result.v1.json`).

#### Documentation

- `docs/reproduce-locally.md`: prerequisites, quick start, Docker setup.
- `docs/user/{vi,en}/bench.md`: compatibility matrix user guide.
- `docs/user/vi/contributing-test-vectors.md`: vector format and ID rules.
- `docs/user/{vi,en}/troubleshooting.md`: 20-item FAQ for common issues.
- `docs/dev/bench-poc.md`: PoC design for headless IME testing.
- `docs/dev/bug-analysis/`: template for upstream bug root cause analysis.

## [0.1.0] — 2026-04-26

First public release. Ships `vietime-doctor`: a diagnostic-only tool
that inspects a Linux desktop for the usual causes of broken
Vietnamese typing. No automatic fixes yet (Installer lands in 0.2).

### Added

#### Doctor CLI (`vietime-doctor`)

- `report` subcommand: full Markdown / plain-text / JSON output.
- `check` subcommand: 1-line status with 0/1/2 exit codes, under
  500 ms on a warm cache (DOC-54).
- `list` subcommand: enumerates every detector (16) and checker (15)
  compiled into the binary.
- `--no-redact` flag: opt out of PII scrubbing for maintainer debugging.
- `--app <id>` flag: wires in per-app detectors for VS Code, Chrome,
  and other Electron/Chromium targets.

#### Detectors (16)

- `sys.distro`, `sys.session`, `sys.locale`, `sys.desktop`
- `env.process`, `env.etc_environment`, `env.home_profile`,
  `env.etc_profile_d`, `env.systemd`
- `im.ibus.daemon`, `im.ibus.engines`, `im.fcitx5.daemon`,
  `im.fcitx5.config`, `im.engines.packages`
- `app.generic`, `app.electron` (both gated on `--app`)

#### Checkers (15: VD001 – VD015)

Full catalogue with stable `VD###` / `VR###` identifiers, evidence
strings, and shell-ready remediation commands. See
`docs/user/en/CHECKS.md` for the reference sheet.

- VD001 `NoImFrameworkActive` (Critical) → VR001
- VD002 `ImFrameworkConflict` (Error) → VR002
- VD003 `EnvVarMismatch` (Error) → VR003
- VD004 `MissingSdlImModule` (Warn) → VR004
- VD005 `EngineInstalledNotRegistered` (Warn) → VR005
- VD006 `WaylandSessionIbus` (Warn) → VR006
- VD007 `ElectronWaylandNoOzone` (Error, `--app`-gated) → VR007
- VD008 `ChromeX11OnWayland` (Warn, `--app`-gated) → VR008
- VD009 `EnvConflictBetweenFiles` (Warn) → VR009
- VD010 `VsCodeSnapDetected` (Warn, `--app vscode`) → VR010
- VD011 `FlatpakAppNoImPortal` (Warn) → VR011
- VD012 `LegacyImSettingEmpty` (Info)
- VD013 `FcitxAddonDisabled` (Warn) → VR013
- VD014 `UnicodeLocaleMissing` (Warn) → VR014
- VD015 `NoVietnameseEngineInstalled` (Info)

#### Report schema

- Stable JSON schema v1 in `schemas/report.v1.json`, validated in CI
  against the `check-jsonschema` metaschema.
- Report includes: `schema_version`, `generated_at`, `tool_version`,
  structured `facts` (system / IM / env / apps), `issues`,
  `recommendations`, and detector `anomalies`.

#### Quality bars

- `cargo-fuzz` harnesses for `parse_etc_environment` and
  `detect_from_os_release` (`fuzz/`). Stable-Rust soak tests mirror
  them in `crates/vietime-core/tests/parser_soak.rs`.
- insta snapshot fixtures for Ubuntu 24.04, Fedora 40, Arch,
  Debian 12, and openSUSE Tumbleweed (`tests/render_snapshots.rs`).
- Distro integration matrix in CI: `ubuntu-22.04`, `ubuntu-24.04`,
  plus `fedora:40`, `archlinux:latest`, `debian:12`,
  `opensuse/tumbleweed:latest` containers.
- PII redactor scrubs `$USER`, hostname, and home-dir paths by
  default; `--no-redact` guarded with a stderr warning.
- Release binary budget: 8 MiB (current: ~4.5 MiB), enforced by
  `scripts/check-binary-size.sh`.

#### Documentation

- English + Vietnamese user guides (`docs/user/{en,vi}/README.md`).
- Per-language glossary and full VD001-VD015 reference sheet.
- Maintainer dry-run script (`scripts/maintainer-dry-run.sh`)
  covering every pre-release gate in one command.

### Known limitations

- No automatic fixes — Doctor is report-only until the Installer
  lands in 0.2.
- `diagnose <topic>` subcommand is a stub in 0.1; full subset-runner
  lands in 0.2.
- `--app` support currently covers vscode and chrome; generic Electron
  detection works but per-app polish (Discord, Slack, Obsidian) is a
  0.1.x patch target.
- Snap and Flatpak detection is heuristic (binary-path + desktop
  metadata). Full sandbox-introspection lands in 0.2 alongside the
  Installer's portal-management work.
