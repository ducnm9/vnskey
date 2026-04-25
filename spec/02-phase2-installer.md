# Phase 2 — VietIME Installer

> One-click installer cho bộ gõ tiếng Việt trên Linux. Mục tiêu: từ terminal mới mở tới gõ được tiếng Việt trong 2 phút.
> Đọc `00-vision-and-scope.md` trước. Tái sử dụng nhiều từ `vietime-core` và `vietime-doctor`.

---

## A. PRD (Product Requirements)

### A.1. Problem statement

Setup Fcitx5-bamboo trên Ubuntu hiện tại cần:

1. `sudo apt remove ibus` (hoặc `ibus-daemon --exit`)
2. `sudo apt install fcitx5 fcitx5-bamboo` (hoặc từ PPA)
3. Chỉnh `/etc/environment` hoặc `~/.profile` 5 biến.
4. `sudo update-alternatives --config gtk3-im-module ...`
5. `im-config -n fcitx5`
6. Logout/login toàn bộ session.
7. Config input method trong fcitx5-configtool.
8. Verify bằng cách gõ thử.

**Mỗi bước sai → không gõ được tiếng Anh luôn hoặc IME không activate.** User bình thường từ Windows qua không vượt qua nổi.

Installer giải quyết: `vietime-installer install fcitx5-bamboo` → tool lo tất cả, rollback được nếu fail.

### A.2. User stories

**US-1**: Là user mới cài Ubuntu 24.04, tôi chạy `vietime-installer install` → tool hỏi tôi muốn bộ gõ nào, tự chọn framework phù hợp (Fcitx5 trên Wayland), cài, set env, verify, và báo "OK, logout và login lại".

**US-2**: Là user có IBus cũ, tôi chạy `vietime-installer switch fcitx5-bamboo` → tool cảnh báo sẽ disable IBus, confirm → gỡ IBus env, cài Fcitx5, set env mới. Nếu lỗi giữa chừng, rollback về IBus.

**US-3**: Là user nâng cao, tôi chạy `vietime-installer install --dry-run` để xem tool sẽ làm gì trước.

**US-4**: Là user không phải Ubuntu (Fedora/Arch), tool detect distro và dùng đúng package manager.

**US-5**: Là user thấy lỗi, tôi chạy `vietime-installer uninstall` → trả config về trước khi cài.

### A.3. Scope

**In-scope (MVP v0.1)**:
- Install matrix: **Fcitx5 + bamboo**, **Fcitx5 + unikey**, **IBus + bamboo**, **IBus + unikey**.
- Distro support: **Ubuntu 22.04+**, **Debian 12+**, **Fedora 39+**, **Arch**, **Pop!_OS 22.04+**. Ubuntu là target #1.
- Actions: `install`, `uninstall`, `switch`, `verify`, `status`, `list`, `doctor` (shell ra `vietime-doctor`).
- Atomic với rollback: mọi thay đổi config đều snapshot trước, có thể revert.
- Idempotent: chạy `install` 2 lần không break gì.
- Dry-run mandatory: `--dry-run` hiển thị plan mà không thực thi.
- Interactive wizard (TUI đơn giản với `dialoguer`/`inquire`).
- Non-interactive mode (`--yes`) cho script/CI.
- Root privilege: hỏi user khi cần `sudo`, không tự escalate.

**Out-of-scope (MVP)**:
- GUI graphic (Qt/GTK). TUI đủ.
- NixOS (cần derivation riêng, community contribute).
- Snap package support cho IME (Canonical không support well).
- Windows/macOS.
- Custom build từ source.
- Tự động logout user (chỉ nhắc).

**In-scope v0.2+**:
- NixOS overlay.
- Slint GUI version.
- Resume từ interrupt (viết state file).
- Offline bundle (download package trước, apply sau).

### A.4. Command surface

```
vietime-installer install                        # interactive wizard
vietime-installer install fcitx5-bamboo          # install specific combo
vietime-installer install ibus-bamboo --yes      # non-interactive
vietime-installer install --dry-run              # print plan only
vietime-installer switch fcitx5-bamboo           # from IBus to Fcitx5 (auto uninstall IBus env)
vietime-installer uninstall                      # remove VietIME-added config, restore backup
vietime-installer verify                         # shell ra `vietime-doctor check`
vietime-installer status                         # short status (1 line per check)
vietime-installer list                           # available combos on this distro
vietime-installer rollback                       # revert last operation
vietime-installer rollback --to <snapshot-id>    # revert to specific snapshot
vietime-installer snapshots                      # list snapshots
vietime-installer doctor                         # embed vietime-doctor call
vietime-installer --verbose
vietime-installer --log-file /tmp/vietime.log
vietime-installer version
```

### A.5. UX — interactive wizard flow

```
$ vietime-installer install

╭────────────────────────────────────────╮
│   VietIME Installer v0.1.0             │
╰────────────────────────────────────────╯

→ Detecting your system...
  Distro: Ubuntu 24.04 (noble)
  Session: Wayland
  Current IM: IBus 1.5.29

? Chọn bộ gõ bạn muốn cài:
  ▸ Fcitx5 + Bamboo (khuyến nghị cho Wayland)
    Fcitx5 + Unikey
    IBus + Bamboo (giữ IBus hiện tại)
    IBus + Unikey
    Tôi đã cài rồi, chỉ config env
  
? Hiện bạn đang dùng IBus. Tiếp tục sẽ chuyển sang Fcitx5 (IBus sẽ bị tắt nhưng không gỡ package). Đồng ý?
  > Yes / No

→ Plan:
  1. Backup:
     - /etc/environment → ~/.config/vietime/snapshots/2026-03-14-10-23/
     - ~/.profile
     - ~/.config/fcitx5/  (nếu có)
  2. apt install -y fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt5 fcitx5-bamboo
  3. im-config -n fcitx5
  4. Update /etc/environment: GTK_IM_MODULE=fcitx, QT_IM_MODULE=fcitx, XMODIFIERS=@im=fcitx, SDL_IM_MODULE=fcitx, GLFW_IM_MODULE=ibus
  5. Enable systemd user service: fcitx5.service
  6. Disable ibus user service
  7. Verify via vietime-doctor check

? Confirm? [Y/n]

→ Running...
  [1/7] Backup ✅
  [2/7] Installing packages (sudo apt)... ✅
  [3/7] im-config -n fcitx5 ✅
  [4/7] Updating /etc/environment ✅
  [5/7] systemctl --user enable fcitx5.service ✅
  [6/7] systemctl --user disable ibus.service ✅
  [7/7] Running vietime-doctor check... ✅ all green

✅ Cài đặt thành công!

⚠️  Bạn cần logout và login lại để env var có hiệu lực.
    Sau khi login, bấm Ctrl+Space để kích hoạt bộ gõ.

📋 Snapshot ID: 2026-03-14-10-23
    Rollback bất cứ lúc nào: vietime-installer rollback

📚 Hướng dẫn gõ Telex: https://vietime.io/docs/telex
```

### A.6. Success criteria (Phase 2)

- Trên Ubuntu sạch (VM), `vietime-installer install --yes` + reboot → gõ được tiếng Việt trong VS Code.
- Rollback phục hồi `/etc/environment` byte-for-byte (so với snapshot).
- Fail-path test: giết process giữa chừng → không leave hệ thống broken (partial install recovered trên lần chạy tiếp).
- 5 distro matrix pass.

---

## B. Technical Design

### B.1. Architecture

```
┌──────────────────────────────────────────────┐
│                  CLI (clap)                  │
└──────────────┬───────────────────────────────┘
               │
     ┌─────────▼─────────┐
     │     Planner       │  (produces Plan from current state + goal)
     └─────────┬─────────┘
               │
     ┌─────────▼─────────┐
     │     Executor      │  (runs Plan Steps, writes Snapshot)
     └─────────┬─────────┘
               │
  ┌────────────┼─────────────────┐
  │            │                 │
  ▼            ▼                 ▼
PackageOps  EnvOps          ServiceOps    ← executors
(apt/dnf/pacman)  (/etc/environment edit, dotfile)  (systemctl --user)
  │            │                 │
  └────────────┴────────┬────────┘
                        │
                  ┌─────▼────────┐
                  │  Snapshot    │ ~/.config/vietime/snapshots/<ts>/
                  │  Store       │  (files + manifest.toml)
                  └──────────────┘
```

**Principles**:
- Plan là pure data (serialize được). Có thể ghi ra file, execute sau.
- Executor chạy Step theo thứ tự, ghi Snapshot trước **mỗi** Step có side-effect.
- Rollback = walk snapshot manifest ngược lại.

### B.2. Data model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: Uuid,
    pub goal: Goal,
    pub generated_at: DateTime<Utc>,
    pub pre_state: PreState,
    pub steps: Vec<Step>,
    pub estimated_duration: Duration,
    pub requires_sudo: bool,
    pub requires_logout: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Goal {
    Install { combo: Combo },
    Uninstall { snapshot_id: Option<String> },
    Switch { from: Combo, to: Combo },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Combo {
    pub framework: ImFramework,   // IBus, Fcitx5
    pub engine: Engine,           // Bamboo, Unikey
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreState {
    pub distro: Distro,
    pub session: SessionType,
    pub active_framework: Option<ImFramework>,
    pub installed_packages: Vec<String>,
    pub env: EnvFacts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Step {
    BackupFile { path: PathBuf },
    InstallPackages { manager: PackageManager, packages: Vec<String> },
    UninstallPackages { manager: PackageManager, packages: Vec<String> },
    SetEnvVar { file: EnvFile, key: String, value: String },
    UnsetEnvVar { file: EnvFile, key: String },
    SystemctlUserEnable { unit: String },
    SystemctlUserDisable { unit: String },
    SystemctlUserStart { unit: String },
    SystemctlUserStop { unit: String },
    RunImConfig { mode: String },
    WriteFile { path: PathBuf, content: String, mode: u32 },
    Verify { check: VerifyCheck },
    Prompt { message: String, continue_if: PromptCondition },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PackageManager { Apt, Dnf, Pacman, Zypper, Xbps, Emerge }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnvFile {
    EtcEnvironment,
    HomeProfile,
    ConfigEnvironmentD { filename: String },  // ~/.config/environment.d/10-vietime.conf
    SystemdUserEnv,
    Custom(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerifyCheck {
    DaemonRunning { framework: ImFramework },
    EngineRegistered { name: String },
    EnvConsistent,
    DoctorCheckPasses,
}
```

### B.3. Planner — quyết định thứ tự Step

Input: `PreState`, `Goal`.
Output: `Plan` thỏa mãn invariants:
1. Mọi Step có side-effect phải có BackupFile tương ứng trước đó trong Plan.
2. Order: `Backup* → InstallPackages → SetEnvVar → Systemctl* → Verify`.
3. `Switch` luôn lồng `Uninstall(old) + Install(new)` với barrier Verify ở giữa.
4. Không step nào assume sudo đã có; `requires_sudo=true` → executor sẽ prompt.

**Planner rules cho Ubuntu + Fcitx5-Bamboo**:

```
Step 1: BackupFile(/etc/environment)
Step 2: BackupFile(~/.profile)
Step 3: BackupFile(~/.config/fcitx5/profile)   [if exists]
Step 4: InstallPackages(apt, [fcitx5, fcitx5-frontend-gtk3, fcitx5-frontend-gtk4,
                              fcitx5-frontend-qt5, fcitx5-module-xorg, fcitx5-bamboo])
Step 5: RunImConfig(fcitx5)
Step 6: SetEnvVar(EtcEnvironment, GTK_IM_MODULE, fcitx)
Step 7: SetEnvVar(EtcEnvironment, QT_IM_MODULE, fcitx)
Step 8: SetEnvVar(EtcEnvironment, XMODIFIERS, "@im=fcitx")
Step 9: SetEnvVar(EtcEnvironment, SDL_IM_MODULE, fcitx)
Step 10: SetEnvVar(EtcEnvironment, GLFW_IM_MODULE, ibus)  [note: GLFW chỉ hiểu ibus hoặc không]
Step 11: WriteFile(~/.config/fcitx5/profile, <default profile enable Bamboo>, 0o644)
Step 12: SystemctlUserEnable(fcitx5.service)
Step 13: SystemctlUserDisable(ibus.service)  [if IBus trước đó là active]
Step 14: SystemctlUserStart(fcitx5.service)
Step 15: Verify(DaemonRunning(Fcitx5))
Step 16: Verify(EngineRegistered("bamboo"))
Step 17: Verify(EnvConsistent)
Step 18: Prompt("Logout và login lại, sau đó chạy vietime-installer verify")
```

### B.4. Executor

```rust
#[async_trait]
pub trait StepExecutor: Send + Sync {
    async fn execute(&self, step: &Step, ctx: &mut ExecContext) -> Result<StepOutcome>;
    async fn rollback(&self, step: &Step, ctx: &mut ExecContext) -> Result<()>;
}

pub struct ExecContext<'a> {
    pub snapshot: &'a mut Snapshot,
    pub dry_run: bool,
    pub log: &'a tracing::Span,
    pub sudo: &'a SudoProvider,
}

pub struct StepOutcome {
    pub applied: bool,           // false nếu dry-run hoặc idempotent no-op
    pub duration: Duration,
    pub notes: Vec<String>,
}
```

**Critical rules**:
- Mọi `execute` phải idempotent (gọi 2 lần không broken).
- Mọi `execute` có side-effect phải ghi `snapshot.record(StepArtifact)` TRƯỚC khi thực thi.
- Nếu một Step fail, Executor gọi `rollback` cho tất cả Step đã apply theo thứ tự **ngược**.
- Executor không swallow error — wrap và rethrow với context.

### B.5. Snapshot store

Layout:

```
~/.config/vietime/snapshots/
├── 2026-03-14T10-23-45Z/
│   ├── manifest.toml              # serialize `Plan` + `SnapshotMeta`
│   ├── files/
│   │   ├── etc_environment.bak     # original content, với path gốc ghi trong manifest
│   │   ├── home_profile.bak
│   │   └── fcitx5_profile.bak
│   ├── packages-installed.txt     # danh sách package mình đã cài (để uninstall)
│   └── services-changed.json      # unit đã enable/disable
└── latest → 2026-03-14T10-23-45Z
```

`manifest.toml`:
```toml
schema_version = 1
id = "..."
goal = { type = "Install", combo = { framework = "Fcitx5", engine = "Bamboo" } }
generated_at = "..."
pre_state_json = "<serialized PreState>"

[[step_artifacts]]
step_index = 1
kind = "BackupFile"
original_path = "/etc/environment"
backup_path = "files/etc_environment.bak"
sha256 = "abc..."

[[step_artifacts]]
step_index = 4
kind = "InstallPackages"
manager = "Apt"
packages = ["fcitx5", "fcitx5-bamboo", ...]
```

**Rollback**: đọc `manifest.toml`, walk `step_artifacts` ngược, gọi `rollback` từng Step. Verify sau cùng: snapshot files match sha256 hiện tại.

### B.6. Sudo handling

- Không tự `sudo` command. Thay vào đó:
  1. Detect Step nào cần privilege (InstallPackages, WriteFile tới /etc).
  2. Nhóm Step lại thành `PrivilegedBatch`.
  3. Prompt user 1 lần: "Sẽ chạy các lệnh sau với sudo. Tiếp tục?"
  4. Spawn `sudo` process tương tác với các lệnh trong batch.
  5. Nếu user không có sudo → abort với thông báo rõ.

- KHÔNG cache password. KHÔNG dùng `pkexec` ở MVP (v0.2).

### B.7. Package manager abstraction

```rust
#[async_trait]
pub trait PackageOps: Send + Sync {
    async fn install(&self, packages: &[String]) -> Result<()>;
    async fn uninstall(&self, packages: &[String]) -> Result<()>;
    async fn is_installed(&self, package: &str) -> Result<bool>;
    async fn available_version(&self, package: &str) -> Result<Option<String>>;
}

// Implementations: AptOps, DnfOps, PacmanOps, ZypperOps
```

Package name map per distro:

| Combo | Ubuntu/Debian | Fedora | Arch |
|---|---|---|---|
| Fcitx5 core | `fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt5 fcitx5-module-xorg` | `fcitx5 fcitx5-gtk fcitx5-qt` | `fcitx5 fcitx5-gtk fcitx5-qt` |
| Fcitx5-Bamboo | `fcitx5-bamboo` (24.04+, hoặc PPA BambooEngine) | `fcitx5-bamboo` (RPM Fusion? nếu thiếu, fallback build from source) | `fcitx5-bamboo` (AUR) |
| Fcitx5-Unikey | `fcitx5-unikey` | `fcitx5-unikey` | `fcitx5-unikey` |
| IBus core | `ibus ibus-gtk ibus-gtk3 ibus-gtk4` | `ibus` | `ibus` |
| IBus-Bamboo | `ibus-bamboo` | (manual/PPA) | `ibus-bamboo-git` (AUR) |
| IBus-Unikey | `ibus-unikey` | `ibus-unikey` | `ibus-unikey` |

**Fallback path** khi distro không có package: Planner detect và đề xuất "Flatpak bundle" option (future v0.2).

### B.8. Env var file edit — semantics

`/etc/environment` là K=V, không phải shell script. Parser:
- Line-based.
- Preserve comments và blank lines khi rewrite.
- Section markers VietIME quản lý:
  ```
  # >>> VietIME managed start >>>
  GTK_IM_MODULE=fcitx
  QT_IM_MODULE=fcitx
  XMODIFIERS=@im=fcitx
  SDL_IM_MODULE=fcitx
  # <<< VietIME managed end <<<
  ```
- Nếu key đã tồn tại ngoài section → comment out, add note, move vào section.
- `uninstall` xóa toàn bộ section, restore comment.

`~/.profile` là shell script, dùng format:
```
# >>> VietIME managed start >>>
export GTK_IM_MODULE=fcitx
...
# <<< VietIME managed end <<<
```

**Ưu tiên** file theo distro:

| Distro | File chính | Lý do |
|---|---|---|
| Ubuntu/Debian | `/etc/environment` | Global, mọi session pick up, IM setup chuẩn |
| Fedora | `~/.config/environment.d/90-vietime.conf` | systemd-style, GNOME/KDE pick up |
| Arch | `~/.config/environment.d/90-vietime.conf` | Cùng lý do |

Parser trong `vietime-core::env` phải handle cả 3 format.

### B.9. Verify step

`Verify(DoctorCheckPasses)` shell out tới `vietime-doctor check` (cùng binary bundle hoặc detect `$PATH`). Nếu `vietime-doctor` không có trong `$PATH`, Installer fail-gracefully và chỉ thực hiện basic verify (daemon running + engine registered).

### B.10. Error handling & recovery

**Scenarios**:

1. **User Ctrl+C giữa install**:
   - SIGINT trap → chạy rollback của tất cả Step đã apply.
   - Print: "Rollback hoàn tất. Hệ thống trở lại trạng thái trước khi chạy."

2. **Package manager fail** (apt returns non-zero):
   - Log stderr.
   - Rollback snapshot.
   - Exit 2.

3. **Sudo bị từ chối**:
   - Không apply gì, exit 64.

4. **Disk full khi ghi snapshot**:
   - Detect trước: check `statvfs` trước khi bắt đầu.
   - Nếu detect muộn: abort, print fatal, require manual cleanup (không tự xóa gì vì có thể chứa data user).

5. **SIGKILL giữa Step**:
   - Không protect được. State có thể inconsistent.
   - Lần chạy tiếp theo: detect snapshot "incomplete" flag → prompt user chọn "resume" hoặc "force rollback".

### B.11. Logging

- `--log-file` ghi structured JSON, mặc định `~/.cache/vietime/installer.log` (rolling).
- Levels: INFO cho user action, DEBUG cho chi tiết, WARN cho recoverable, ERROR cho fatal.
- Log luôn redact (reuse `vietime-doctor` redaction).

### B.12. Testing strategy

1. **Unit tests**: Planner với fixture PreState → assert Plan giống golden.
2. **Step executor mocks**: PackageOps mock, verify gọi đúng command.
3. **Integration tests trong Docker**:
   - Image `ubuntu:22.04`, `ubuntu:24.04`, `debian:12`, `fedora:40`, `archlinux:latest`.
   - Script: `install` → `verify` → `uninstall` → verify trạng thái sạch.
   - Kỳ vọng: exit 0, diff config file với pre-install == empty.
4. **VM test (manual)**: thật sự login session GNOME/KDE, gõ tiếng Việt trong gedit. Checklist `docs/testing/install-manual.md`.
5. **Chaos test**: ngẫu nhiên inject SIGTERM giữa Step → verify snapshot cleanup.
6. **Dry-run test**: every integration test chạy với `--dry-run` trước → assert không có side-effect.

### B.13. Build & distribution

- Cùng Cargo workspace với Doctor.
- `cargo-deb` → `.deb` có `Depends: sudo, coreutils`.
- Flatpak khả năng **không phù hợp** (Flatpak sandbox không reach `/etc/environment`). Installer phải là **native package** hoặc standalone binary. Ship:
  - `.deb` cho Ubuntu/Debian
  - `.rpm` cho Fedora
  - AUR cho Arch
  - Plain tar.gz với `install.sh` fallback
- **Không** Flatpak cho Installer (chỉ Doctor và Bench Flatpak được).

### B.14. Roadmap Phase 2

| Tuần | Milestone |
|---|---|
| 1 | Planner skeleton, Combo/Goal types, Plan serialization |
| 2 | Snapshot store + BackupFile executor, env file parser |
| 3 | PackageOps (Apt first), SetEnvVar executor, SystemctlUserEnable |
| 4 | Full Plan cho Ubuntu+Fcitx5-Bamboo, interactive wizard, dry-run |
| 5 | Rollback path, Ctrl+C handler, snapshot listing |
| 6 | Dnf + Pacman, Fedora + Arch matrix |
| 7 | Switch combo, verify integration với Doctor, docs vi+en |
| 8 | Docker integration test CI, deb/rpm packaging, v0.1.0 release |

Timebox **8 tuần**. Nếu tuần 5 chưa có rollback work → cắt Arch/Fedora sang v0.2.

### B.15. Risks

| Risk | Mitigation |
|---|---|
| `/etc/environment` edit gây user không gõ được tiếng Anh | Dry-run mandatory, snapshot mandatory, section markers rõ ràng |
| Race condition khi user có env đã set ở ~/.bashrc và `/etc/environment` | Detector trong Doctor phát hiện, Installer cảnh báo trước khi edit |
| im-config không có trên Fedora/Arch | Planner không dùng im-config cho non-Debian, set env thẳng |
| fcitx5-bamboo không có trong repo Fedora | Fallback: build from source (v0.2) hoặc error gracefully |
| User Flatpak-only (Pop!_OS Immutable) | Detect Silverblue/Kinoite, refuse với hướng dẫn rpm-ostree |
| SIGKILL giữa chừng → state inconsistent | Incomplete flag, resume prompt, force rollback option |

### B.16. Acceptance criteria

- [ ] `vietime-installer install fcitx5-bamboo --yes` trên Ubuntu 24.04 VM sạch → reboot → gõ được.
- [ ] `vietime-installer uninstall` → diff `/etc/environment` và `~/.profile` = empty so với pre.
- [ ] `--dry-run` không tạo file/package/service nào.
- [ ] Docker integration test 5 distro pass.
- [ ] SIGINT test: rollback sạch.
- [ ] README vi+en.
- [ ] `.deb` install trên Ubuntu 22.04 clean, không missing dep.
