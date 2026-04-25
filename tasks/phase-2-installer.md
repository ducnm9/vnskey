# Phase 2 — VietIME Installer v0.1 (Tháng 4–5, 8 tuần)

> **Goal**: release Installer v0.1.0 với `.deb`/`.rpm`, test được trên 5 distro VM, rollback an toàn.
>
> **Exit criteria**: spec/02 §B.16 (acceptance checklist).
>
> **Budget**: 80–120h.

---

## Week 1 — Planner + data model

### INS-01 — CLI skeleton
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: DOC-01
- **Spec ref**: spec/02 §A.4
- **Acceptance**:
  - [ ] Subcommands: `install`, `uninstall`, `switch`, `verify`, `status`, `list`, `rollback`, `snapshots`, `doctor`, `version`.
  - [ ] Global flags: `--dry-run`, `--yes`, `--verbose`, `--log-file`.
  - [ ] `--help` hiển thị all combos (`fcitx5-bamboo`, `ibus-bamboo`, ...).

### INS-02 — `Combo`, `Goal`, `Plan`, `Step` types
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: INS-01
- **Spec ref**: spec/02 §B.2
- **Acceptance**:
  - [ ] `Combo { framework, engine }` với PartialEq/Eq/Hash.
  - [ ] `Goal::Install | Uninstall | Switch`.
  - [ ] `Step` enum 14 variants (BackupFile, InstallPackages, ...).
  - [ ] `Plan` struct serializable TOML + JSON.
  - [ ] Roundtrip test: Plan → TOML → Plan.

### INS-03 — `PreState` detector (reuse vietime-core)
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: INS-02, DOC-24
- **Spec ref**: spec/02 §B.2
- **Acceptance**:
  - [ ] `detect_pre_state() -> PreState` dùng các detector Doctor đã có.
  - [ ] Tests với fixture môi trường giả.

### INS-04 — `Planner` skeleton
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: INS-03
- **Spec ref**: spec/02 §B.3
- **Acceptance**:
  - [ ] `fn plan(pre: PreState, goal: Goal) -> Plan`.
  - [ ] Invariants check: mọi Step side-effect có BackupFile trước.
  - [ ] Unit test: Plan cho Install(fcitx5-bamboo) trên Ubuntu 24.04 fixture = golden.

---

## Week 2 — Snapshot store + env file editor

### INS-10 — Snapshot store layout
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: INS-02
- **Spec ref**: spec/02 §B.5
- **Acceptance**:
  - [ ] `~/.config/vietime/snapshots/<ts>/` directory structure.
  - [ ] `manifest.toml` serialize Plan + artifacts.
  - [ ] `latest` symlink update atomic.
  - [ ] `list_snapshots() -> Vec<SnapshotMeta>`.
  - [ ] Tests với tempdir.

### INS-11 — `BackupFile` executor
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: INS-10
- **Spec ref**: spec/02 §B.4
- **Acceptance**:
  - [ ] Copy file gốc sang snapshot với sha256 record.
  - [ ] Idempotent: nếu backup đã tồn tại cùng sha, skip.
  - [ ] Rollback: restore từ snapshot, verify sha post.
  - [ ] Handle file-not-exist (record as "did-not-exist").

### INS-12 — Env file parser & writer
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (8h)
- **Depends on**: DOC-11
- **Spec ref**: spec/02 §B.8
- **Acceptance**:
  - [ ] `EtcEnvironment` K=V format: parse, preserve comments, write section markers.
  - [ ] `HomeProfile` shell syntax: `export` lines, section markers.
  - [ ] `ConfigEnvironmentD` systemd format.
  - [ ] Section markers: `# >>> VietIME managed start >>>` ... `# <<< VietIME managed end <<<`.
  - [ ] Nếu key trùng ngoài section → comment out + note.
  - [ ] `uninstall` xóa toàn bộ section, restore commented.
  - [ ] 20 fixture test cases.

### INS-13 — `SetEnvVar` + `UnsetEnvVar` executors
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: INS-11, INS-12
- **Spec ref**: spec/02 §B.4
- **Acceptance**:
  - [ ] Apply change, record artifact.
  - [ ] Rollback: restore file từ snapshot.
  - [ ] Idempotent.

---

## Week 3 — Package manager ops

### INS-20 — `PackageOps` trait + `AptOps`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (6h)
- **Depends on**: INS-04
- **Spec ref**: spec/02 §B.7
- **Acceptance**:
  - [ ] `install(&[&str])`, `uninstall(&[&str])`, `is_installed(&str)`, `available_version(&str)`.
  - [ ] `AptOps` dùng `apt-get -y` wrapper với sudo.
  - [ ] Capture stdout/stderr vào log.
  - [ ] Test với `--dry-run` → `apt-get install --dry-run`.
  - [ ] Integration test Docker ubuntu:22.04.

### INS-21 — `InstallPackages` executor
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: INS-20
- **Spec ref**: spec/02 §B.4
- **Acceptance**:
  - [ ] Record packages "we installed this" trong snapshot.
  - [ ] Idempotent: skip if all installed.
  - [ ] Rollback: uninstall những package we installed (không touch packages đã có trước).

### INS-22 — `SystemctlUserEnable/Disable/Start/Stop` executors
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: INS-10
- **Spec ref**: spec/02 §B.4
- **Acceptance**:
  - [ ] Wrap `systemctl --user <action> <unit>` (no sudo).
  - [ ] Record previous state (was-enabled, was-running).
  - [ ] Rollback reverses.

### INS-23 — `RunImConfig` executor (Ubuntu/Debian)
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: INS-22
- **Spec ref**: spec/02 §B.4
- **Acceptance**:
  - [ ] Spawn `im-config -n <mode>` (requires sudo).
  - [ ] Record previous mode for rollback.
  - [ ] Skip on Fedora/Arch (no im-config).

### INS-24 — `WriteFile` executor (Fcitx5 profile)
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (3h)
- **Depends on**: INS-11
- **Spec ref**: spec/02 §B.4
- **Acceptance**:
  - [ ] Write file với mode bits.
  - [ ] Backup trước nếu file đã tồn tại.
  - [ ] Default `~/.config/fcitx5/profile` content enable Bamboo template.

---

## Week 4 — Full flow Ubuntu + interactive wizard

### INS-30 — Executor orchestrator
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: INS-11, INS-13, INS-21, INS-22
- **Spec ref**: spec/02 §B.4
- **Acceptance**:
  - [ ] Execute Plan sequentially; record each StepArtifact before apply.
  - [ ] On failure: walk backward, call `rollback` for applied steps.
  - [ ] Dry-run mode: print plan, no side-effect.
  - [ ] Tests với mock executors.

### INS-31 — Sudo handler
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: INS-30
- **Spec ref**: spec/02 §B.6
- **Acceptance**:
  - [ ] Detect which steps need privilege.
  - [ ] Batch privileged steps, prompt user 1 lần.
  - [ ] Spawn `sudo` interactive (stdin inherit).
  - [ ] Abort if sudo denied.
  - [ ] KHÔNG cache password.

### INS-32 — Interactive wizard (TUI)
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (8h)
- **Depends on**: INS-30
- **Spec ref**: spec/02 §A.5
- **Acceptance**:
  - [ ] `dialoguer` hoặc `inquire` prompts.
  - [ ] Flow: detect → show current state → chọn combo → show plan → confirm → run → show result.
  - [ ] `--yes` bypass.
  - [ ] Test với scripted input (`expect`-style).

### INS-33 — Full Ubuntu + Fcitx5-Bamboo E2E
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (6h)
- **Depends on**: INS-32
- **Spec ref**: spec/02 §B.3 (plan rules)
- **Acceptance**:
  - [ ] `vietime-installer install fcitx5-bamboo --yes` trên Ubuntu 24.04 VM sạch.
  - [ ] Reboot → gõ tiếng Việt trong gedit.
  - [ ] Manual checklist pass `docs/testing/install-manual.md`.

---

## Week 5 — Rollback + safety

### INS-40 — `rollback` command
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: INS-30
- **Spec ref**: spec/02 §A.4, §B.5
- **Acceptance**:
  - [ ] `vietime-installer rollback` rollback latest snapshot.
  - [ ] `rollback --to <id>` rollback specific.
  - [ ] Verify sha256 files post-restore.
  - [ ] Print diff summary.

### INS-41 — SIGINT handler + incomplete flag
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: INS-30
- **Spec ref**: spec/02 §B.10 (#1, #5)
- **Acceptance**:
  - [ ] SIGINT trap → auto rollback applied steps.
  - [ ] Nếu SIGKILL: lần chạy sau phát hiện `incomplete=true` flag trong manifest.
  - [ ] Prompt user "resume" / "force rollback".

### INS-42 — Disk space pre-check
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: INS-10
- **Spec ref**: spec/02 §B.10 (#4)
- **Acceptance**:
  - [ ] `statvfs` check trên snapshot dir trước khi bắt đầu.
  - [ ] Need ≥ 10 MB free; else abort.

### INS-43 — `snapshots` listing command
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (2h)
- **Depends on**: INS-10
- **Spec ref**: spec/02 §A.4
- **Acceptance**:
  - [ ] Table output: ID, date, goal, status.
  - [ ] `--json`.

### INS-44 — `uninstall` command
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: INS-40
- **Spec ref**: spec/02 §A.4
- **Acceptance**:
  - [ ] Find latest "install" snapshot, rollback toàn bộ.
  - [ ] `--keep-packages` flag để giữ packages (chỉ gỡ config).
  - [ ] Verify diff `/etc/environment` vs pre-install = empty.

---

## Week 6 — Dnf + Pacman

### INS-50 — `DnfOps` (Fedora)
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: INS-20
- **Spec ref**: spec/02 §B.7
- **Acceptance**:
  - [ ] `dnf install -y`, `dnf remove -y`, `rpm -q`.
  - [ ] Handle package name map Fedora.
  - [ ] Docker integration test fedora:40.

### INS-51 — `PacmanOps` (Arch)
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: INS-20
- **Spec ref**: spec/02 §B.7
- **Acceptance**:
  - [ ] `pacman -S --noconfirm`, `pacman -R`, `pacman -Q`.
  - [ ] AUR fallback (document, không auto-install AUR trong MVP).
  - [ ] Docker integration test archlinux:latest.

### INS-52 — Package name mapping table
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: INS-50, INS-51
- **Spec ref**: spec/02 §B.7
- **Acceptance**:
  - [ ] `PackageMap` table trong code: Combo × Distro → Vec<package>.
  - [ ] Test exhaustive: 4 combos × 5 distros.
  - [ ] Handle missing package: Planner error rõ ràng.

### INS-53 — Planner rules cho Fedora + Arch
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: INS-52
- **Spec ref**: spec/02 §B.3, §B.8
- **Acceptance**:
  - [ ] Fedora: dùng `~/.config/environment.d/90-vietime.conf` thay `/etc/environment`.
  - [ ] Arch: tương tự Fedora.
  - [ ] Không gọi `im-config` trên non-Debian.
  - [ ] Golden plan tests cho cả 2 distro.

---

## Week 7 — Switch + verify + docs

### INS-60 — `switch` command
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: INS-40, INS-44
- **Spec ref**: spec/02 §A.4
- **Acceptance**:
  - [ ] `switch <new-combo>` = Uninstall(old) → Install(new) với barrier Verify.
  - [ ] Rollback nếu new install fail.
  - [ ] E2E test IBus → Fcitx5.

### INS-61 — `verify` integration với Doctor
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: DOC-54
- **Spec ref**: spec/02 §B.9
- **Acceptance**:
  - [ ] `vietime-installer verify` shell ra `vietime-doctor check`.
  - [ ] Fallback basic verify nếu Doctor not in PATH.

### INS-62 — `status` command
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (2h)
- **Depends on**: INS-61
- **Spec ref**: spec/02 §A.4
- **Acceptance**:
  - [ ] 1-line status per check (framework running, env OK, engine registered).
  - [ ] Exit code 0/1/2.

### INS-63 — `list` command
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (2h)
- **Depends on**: INS-52
- **Spec ref**: spec/02 §A.4
- **Acceptance**:
  - [ ] Print available combos cho distro hiện tại.
  - [ ] Annotate "installed" / "available".

### INS-64 — User docs vi + en
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: INS-33
- **Spec ref**: spec/04 §6
- **Acceptance**:
  - [ ] `docs/vi/installer.md`: quickstart, troubleshooting, rollback guide.
  - [ ] `docs/en/installer.md` mirror.
  - [ ] Video hoặc asciinema demo embed.

---

## Week 8 — Release

### INS-70 — Docker integration test CI
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (6h)
- **Depends on**: INS-33, INS-50, INS-51
- **Spec ref**: spec/02 §B.12 (#3)
- **Acceptance**:
  - [ ] `.github/workflows/installer-integration.yml` matrix: ubuntu:22.04, ubuntu:24.04, debian:12, fedora:40, archlinux:latest.
  - [ ] Flow: install → verify → uninstall → diff assert empty.
  - [ ] Chaos test: SIGTERM giữa chừng → verify clean state.

### INS-71 — `cargo-deb` + `.rpm` build
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: INS-70
- **Spec ref**: spec/02 §B.13
- **Acceptance**:
  - [ ] `.deb` với `Depends: sudo, coreutils`.
  - [ ] `.rpm` via `cargo-generate-rpm`.
  - [ ] Test install trên clean VM.

### INS-72 — AUR PKGBUILD
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (2h)
- **Depends on**: INS-71
- **Spec ref**: spec/02 §B.13
- **Acceptance**:
  - [ ] `vietime-installer-bin` AUR.
  - [ ] Test trên Arch container.

### INS-73 — Release workflow + CHANGELOG
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (2h)
- **Depends on**: INS-71
- **Spec ref**: spec/04 §7.3
- **Acceptance**:
  - [ ] Tag `v0.1.0-installer`.
  - [ ] CHANGELOG.md updated.
  - [ ] GH Release với `.deb`, `.rpm`, `.tar.gz`, GPG sig.

### INS-74 — Blog post #3 + launch
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: INS-73
- **Spec ref**: spec/05 §2 (Phase 2 release)
- **Acceptance**:
  - [ ] Blog "One-click cài gõ tiếng Việt trên Linux" với asciinema.
  - [ ] Post FB + Reddit.
  - [ ] Track downloads.

---

## Phase 2 — Exit checklist (spec/02 §B.16)

- [ ] Install + verify trên Ubuntu 24.04 VM sạch.
- [ ] Uninstall → diff = empty.
- [ ] Dry-run không tạo file/package/service nào.
- [ ] Docker integration 5 distro pass.
- [ ] SIGINT rollback sạch.
- [ ] README vi + en.
- [ ] `.deb` install trên Ubuntu 22.04 không missing dep.

**Timebox rule**: cuối W5 nếu rollback chưa xong → cắt Arch/Fedora sang v0.2, giữ Ubuntu + Debian only.
