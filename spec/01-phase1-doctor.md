# Phase 1 — VietIME Doctor

> Công cụ chẩn đoán Vietnamese IME trên Linux. CLI-first, single static Rust binary.
> Đọc `00-vision-and-scope.md` trước.

---

## A. PRD (Product Requirements)

### A.1. Problem statement

Khi user báo "gõ tiếng Việt bị lỗi", họ và maintainer đều mù: không biết session X11/Wayland, IM framework nào đang chạy, env var có đúng không, app đang bật flag gì. Mỗi bug report là một cuộc phỏng vấn 20 câu hỏi.

**Doctor giải quyết**: một lệnh duy nhất xuất ra **báo cáo đầy đủ, copy-paste được, ẩn PII**, và đề xuất fix cụ thể cho các cấu hình sai đã biết.

### A.2. User stories

**US-1 (P1 user)**: Là developer mới lên Ubuntu, sau khi `apt install ibus-bamboo` xong không gõ được, tôi chạy `vietime-doctor` và được chỉ ra thiếu `GTK_IM_MODULE=ibus` trong `~/.profile`, kèm lệnh fix copy-paste.

**US-2 (P2 user)**: Là developer dùng VS Code, khi gõ mất dấu tôi chạy `vietime-doctor --app vscode` và được biết VS Code đang chạy X11 backend dù session là Wayland, được đề xuất thêm `--enable-features=UseOzonePlatform,WaylandWindowDecorations`.

**US-3 (P3 user)**: Là maintainer, khi nhận bug report tôi yêu cầu user dán output `vietime-doctor report --json` và tôi parse ra trạng thái môi trường trong 5 giây.

### A.3. Scope

**In-scope (MVP v0.1)**:
- Detect: distro, desktop environment, session type (X11/Wayland), IM framework đang chạy (IBus/Fcitx5/none), phiên bản IM, env vars liên quan.
- Check: env vars nhất quán, IBus/Fcitx5 daemon có đang chạy, bộ gõ tiếng Việt nào đang được đăng ký, conflict giữa IBus+Fcitx5.
- Inspect theo app (khi `--app X`): binary path, Electron flags (nếu Electron app), GTK/Qt version, IM module đang bind.
- Output: human-readable markdown (default), JSON (`--json`), plain text (`--plain`).
- Redact PII: username, hostname, email trong env.
- Offline 100%. Không gọi internet.

**Out-of-scope (MVP)**:
- Tự động fix (đó là việc của Installer).
- Theo dõi realtime (top-like view).
- Detect mode gõ hiện tại (Telex/VNI) — chỉ biết bộ gõ nào đang active, không biết user đã chọn mode nào nếu config nằm trong file của IME.
- Windows/macOS.

**In-scope v0.2+** (không làm ngay, nhưng giữ đường):
- TUI (ratatui) với live view.
- Benchmark micro-test gõ latency.
- Plugin per-app detector (Chrome, Firefox, VS Code, Slack mỗi cái một detector module).

### A.4. Command surface (UX)

```
vietime-doctor                 # default: human-readable markdown report
vietime-doctor --json          # JSON cho maintainer parse
vietime-doctor --plain         # plain text không markdown
vietime-doctor --verbose       # thêm raw detector output
vietime-doctor --app vscode    # focus vào 1 app, kèm detector riêng
vietime-doctor --app /usr/bin/code   # đường dẫn binary cũng accept
vietime-doctor check           # chỉ chạy các check, exit 0/1 cho CI
vietime-doctor list            # liệt kê detector/checker có sẵn
vietime-doctor diagnose env    # chỉ phần env var
vietime-doctor diagnose daemon # chỉ phần daemon state
vietime-doctor report --output report.md   # ghi ra file
vietime-doctor --no-redact     # tắt PII redaction (debug)
vietime-doctor version
vietime-doctor --help
```

**Convention**: mọi subcommand đều `--json` được. Mọi check exit code:
- `0`: everything OK.
- `1`: có config issue nhưng không critical (env không nhất quán nhưng daemon chạy).
- `2`: critical (daemon không chạy, không có bộ gõ Vietnamese nào).
- `64`: usage error (bad argument).
- `70`: internal error (panic hoặc bug Doctor).

### A.5. Output format — user-facing sample

```markdown
# VietIME Doctor Report
Generated: 2025-03-14T10:23:11+07:00
vietime-doctor v0.1.0

## Environment
- Distro: Ubuntu 24.04 LTS (noble)
- Desktop: GNOME 46
- Session: Wayland
- Shell: zsh 5.9

## IM Framework
- Active: IBus 1.5.29 (via systemd user service, pid 2341)
- Fcitx5: not installed
- Bamboo engine: ibus-bamboo 0.8.2 (installed, registered)
- Unikey engine: ibus-unikey 0.6.1 (installed, NOT registered)

## Environment Variables
| Var | Value | Status |
|---|---|---|
| GTK_IM_MODULE | ibus | ✅ ok |
| QT_IM_MODULE | ibus | ✅ ok |
| XMODIFIERS | @im=ibus | ✅ ok |
| SDL_IM_MODULE | <unset> | ⚠️ missing (recommend `ibus`) |
| GLFW_IM_MODULE | <unset> | ⚠️ missing (optional) |

## Checks
- [✅] IBus daemon running
- [✅] ibus-bamboo registered in `ibus list-engine`
- [⚠️] SDL_IM_MODULE not set — SDL apps (some games, Zoom old) will not receive Vietnamese input
- [❌] Electron apps likely to drop characters on Wayland — see fix below

## App-specific
(not requested)

## Recommendations
1. Add to `~/.profile`:
   ```
   export SDL_IM_MODULE=ibus
   ```
2. For Electron apps (VS Code, Slack, Discord, Notion), run with:
   ```
   --enable-features=UseOzonePlatform,WaylandWindowDecorations --ozone-platform-hint=auto
   ```
   Example wrapper in `~/.local/share/applications/code.desktop`.

## Paste this to a bug report
<paste full report above>
```

### A.6. Success criteria (Phase 1)

- Chạy `vietime-doctor` trên 5 distro (Ubuntu 22.04 + 24.04, Fedora 39, Pop!_OS, Arch) → cả 5 ra report hợp lý.
- Maintainer ibus-bamboo xác nhận report đủ thông tin để debug 80% bug report.
- Single binary <8 MB sau strip.
- `vietime-doctor check` < 500ms trên máy đã cài IM.
- Zero runtime dependency ngoài libc.

---

## B. Technical Design

### B.1. Architecture overview

```
┌──────────────────────────────────────────────┐
│                  CLI (clap)                  │
│   subcommand: doctor | check | diagnose ...  │
└───────────────────┬──────────────────────────┘
                    │
      ┌─────────────┴──────────────┐
      │        Orchestrator        │
      │  - schedules detectors     │
      │  - collects Facts          │
      │  - runs Checkers           │
      │  - builds Report           │
      └───────┬──────────┬─────────┘
              │          │
    ┌─────────▼──┐   ┌───▼────────────┐
    │ Detectors  │   │   Checkers     │
    │ (read-only)│   │ (Facts → Issue)│
    └─────┬──────┘   └────────────────┘
          │
  ┌───────┴─────────────────────────────┐
  │ distro | session | im_framework |   │
  │ env | engines | daemon | apps       │
  └─────────────────────────────────────┘
```

**Separation of concerns**:
- **Detector**: đọc state hệ thống → emit `Fact`. Pure read-only. Mỗi detector có thể fail độc lập.
- **Checker**: nhận `Vec<Fact>` → emit `Vec<Issue>` với severity và gợi ý fix. Pure function.
- **Orchestrator**: điều phối, xử lý timeout, aggregate, render.
- **Renderer**: `Report` → Markdown / JSON / plain.

Quy tắc: **detector không gọi detector khác**. Nếu cần data từ detector khác, lấy qua `Facts` sau khi tất cả detector chạy xong (2-pass).

### B.2. Data model (Rust types)

```rust
// crates/vietime-core/src/report.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,         // = 1 for v0.1
    pub generated_at: DateTime<Utc>,
    pub tool_version: String,
    pub facts: Facts,
    pub issues: Vec<Issue>,
    pub recommendations: Vec<Recommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Facts {
    pub system: SystemFacts,
    pub im: ImFacts,
    pub env: EnvFacts,
    pub apps: Vec<AppFacts>,    // only populated if --app used
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemFacts {
    pub distro: Option<Distro>,
    pub desktop: Option<DesktopEnv>,
    pub session: Option<SessionType>,
    pub kernel: Option<String>,
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Distro {
    Ubuntu { version: String, codename: String },
    Debian { version: String },
    Fedora { version: String },
    Arch,
    PopOs { version: String },
    Mint { version: String },
    Other { id: String, version: Option<String> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionType { X11, Wayland, Tty, Unknown }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DesktopEnv {
    Gnome { version: Option<String> },
    Kde { version: Option<String> },
    Xfce, Cinnamon, Mate, Budgie, Sway, Hyprland,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImFacts {
    pub active_framework: ImFramework,      // IBus | Fcitx5 | None | Conflict
    pub ibus: Option<IbusFacts>,
    pub fcitx5: Option<Fcitx5Facts>,
    pub engines: Vec<EngineFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbusFacts {
    pub version: String,
    pub daemon_running: bool,
    pub daemon_pid: Option<u32>,
    pub config_dir: PathBuf,
    pub registered_engines: Vec<String>,   // from `ibus list-engine`
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fcitx5Facts {
    pub version: String,
    pub daemon_running: bool,
    pub daemon_pid: Option<u32>,
    pub config_dir: PathBuf,
    pub addons_enabled: Vec<String>,
    pub input_methods_configured: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineFact {
    pub name: String,              // "bamboo", "Bamboo", "Unikey", "vietnamese-telex"
    pub package: Option<String>,   // "ibus-bamboo", "fcitx5-bamboo"
    pub version: Option<String>,
    pub framework: ImFramework,
    pub is_vietnamese: bool,
    pub is_registered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvFacts {
    pub gtk_im_module: Option<String>,
    pub qt_im_module: Option<String>,
    pub qt4_im_module: Option<String>,
    pub xmodifiers: Option<String>,
    pub input_method: Option<String>,
    pub sdl_im_module: Option<String>,
    pub glfw_im_module: Option<String>,
    pub clutter_im_module: Option<String>,
    pub sources: HashMap<String, EnvSource>,   // var → đọc từ đâu
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EnvSource {
    Process,            // /proc/self/environ
    EtcEnvironment,     // /etc/environment
    EtcProfileD,        // /etc/profile.d/*.sh (grep-based)
    HomeProfile,        // ~/.profile, ~/.bashrc, ~/.zshrc
    SystemdUserEnv,     // `systemctl --user show-environment`
    Pam,                // /etc/pam.d/*
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppFacts {
    pub app_id: String,             // "vscode", "chrome", "slack"
    pub binary_path: PathBuf,
    pub version: Option<String>,
    pub kind: AppKind,
    pub electron_version: Option<String>,
    pub uses_wayland: Option<bool>,
    pub runtime_env_snapshot: HashMap<String, String>,  // if process running, /proc/<pid>/environ
    pub detector_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppKind {
    Native,         // pure GTK/Qt
    Electron,
    Chromium,
    Jvm,            // IntelliJ etc.
    Flatpak { sandbox_id: String },
    Snap { name: String },
    AppImage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,              // "VD001", "VD002"... stable
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    pub facts_evidence: Vec<String>,   // human-readable citations
    pub recommendation: Option<String>,// id of Recommendation
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity { Info, Warn, Error, Critical }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub id: String,              // "VR001"
    pub title: String,
    pub description: String,
    pub commands: Vec<String>,   // shell commands to run, copy-pastable
    pub safe_to_run_unattended: bool,
    pub references: Vec<String>, // URLs to docs
}
```

### B.3. Detectors — danh sách cụ thể

Mỗi detector là một struct implement trait:

```rust
#[async_trait]
pub trait Detector: Send + Sync {
    fn id(&self) -> &'static str;
    fn depends_on(&self) -> &'static [&'static str] { &[] }
    async fn run(&self, ctx: &DetectorContext) -> Result<DetectorOutput>;
    fn timeout(&self) -> Duration { Duration::from_secs(3) }
}

pub struct DetectorOutput {
    pub partial_facts: PartialFacts,   // merge vào Facts
    pub notes: Vec<String>,
    pub duration: Duration,
}
```

**Detector list (v0.1)**:

| ID | Tên | Input | Output |
|---|---|---|---|
| `sys.distro` | DistroDetector | `/etc/os-release`, `lsb_release` | `SystemFacts.distro` |
| `sys.desktop` | DesktopDetector | `$XDG_CURRENT_DESKTOP`, `$DESKTOP_SESSION`, gnome-shell `--version` | `SystemFacts.desktop` |
| `sys.session` | SessionDetector | `$XDG_SESSION_TYPE`, `$WAYLAND_DISPLAY`, `$DISPLAY` | `SystemFacts.session` |
| `sys.kernel` | KernelDetector | `uname -r` | `SystemFacts.kernel` |
| `sys.shell` | ShellDetector | `$SHELL`, `getent passwd $(id -u)` | `SystemFacts.shell` |
| `im.ibus.daemon` | IbusDaemonDetector | D-Bus `org.freedesktop.IBus`, `pgrep ibus-daemon` | `ImFacts.ibus` |
| `im.ibus.engines` | IbusEnginesDetector | `ibus list-engine` output | `ImFacts.engines` (filtered) |
| `im.fcitx5.daemon` | Fcitx5DaemonDetector | D-Bus `org.fcitx.Fcitx5`, `pgrep fcitx5` | `ImFacts.fcitx5` |
| `im.fcitx5.config` | Fcitx5ConfigDetector | `~/.config/fcitx5/profile`, `~/.local/share/fcitx5/` | Đ `addons_enabled`, `input_methods_configured` |
| `im.engines.packages` | PackageEnginesDetector | `dpkg -l`, `rpm -q`, `pacman -Q` (fallback to `which`) | `EngineFact[]` |
| `env.process` | ProcessEnvDetector | `/proc/self/environ` (chính là env của chính Doctor) | `EnvFacts.*`, `sources = Process` |
| `env.etc_environment` | EtcEnvironmentDetector | parse `/etc/environment` | merge `EnvFacts` với source `EtcEnvironment` |
| `env.etc_profile_d` | EtcProfileDDetector | grep `/etc/profile.d/*.sh` cho các key IM | merge với source `EtcProfileD` |
| `env.home_profile` | HomeProfileDetector | grep `~/.profile`, `~/.bashrc`, `~/.zshrc`, `~/.config/environment.d/*.conf` | merge |
| `env.systemd` | SystemdEnvDetector | `systemctl --user show-environment` | merge |
| `app.generic` | GenericAppDetector | `which <app>`, đọc binary | `AppFacts` |
| `app.electron` | ElectronAppDetector | đọc `.asar` metadata, detect `--electron-version` | Electron-specific fields |
| `app.flatpak` | FlatpakAppDetector | `flatpak info <id>` | sandbox env |

**Rules**:
- Mỗi detector **phải** có timeout (default 3s, override khi cần).
- Detector fail không crash cả process; ghi note vào report.
- Không detector nào write file. Không detector nào gọi sudo.
- D-Bus call qua `zbus` session bus, không system bus (không cần privilege).

### B.4. Checkers — danh sách cụ thể

Checker chạy sau khi `Facts` đầy đủ, thuần sync pure function `(&Facts) -> Vec<Issue>`.

| ID | Tên | Severity | Trigger |
|---|---|---|---|
| `VD001` | NoImFrameworkActive | Critical | Không có ibus-daemon/fcitx5 chạy + có engine Vietnamese đã cài |
| `VD002` | ImFrameworkConflict | Error | Cả ibus-daemon và fcitx5 đều chạy |
| `VD003` | EnvVarMismatch | Error | GTK_IM_MODULE=ibus nhưng active là fcitx5 (hoặc ngược) |
| `VD004` | MissingSdlImModule | Warn | SDL_IM_MODULE không set |
| `VD005` | EngineInstalledNotRegistered | Warn | ibus-bamboo installed nhưng không trong `ibus list-engine` (IBus) hoặc input-methods-configured (Fcitx5) |
| `VD006` | WaylandSessionIbus | Warn | Session là Wayland và đang dùng IBus (gợi ý Fcitx5 nếu có bug cụ thể) |
| `VD007` | ElectronWaylandNoOzone | Error | App Electron đang chạy nhưng không có Ozone flags (chỉ chạy khi `--app`) |
| `VD008` | ChromeX11OnWayland | Warn | Chrome/Chromium đang chạy X11 backend dù session Wayland |
| `VD009` | EnvConflictBetweenFiles | Warn | `~/.profile` đặt `GTK_IM_MODULE=ibus` nhưng `/etc/environment` đặt `fcitx` |
| `VD010` | VsCodeSnapDetected | Warn | VS Code snap — IM thường không chạy qua snap sandbox |
| `VD011` | FlatpakAppNoImPortal | Warn | App Flatpak nhưng `xdg-desktop-portal` không expose input method |
| `VD012` | LegacyImSettingEmpty | Info | `INPUT_METHOD` env không set (legacy, khuyến nghị có) |
| `VD013` | FcitxAddonDisabled | Warn | Active là Fcitx5 nhưng addon `wayland-im` hoặc `xim` disabled |
| `VD014` | UnicodeLocaleMissing | Error | Locale không phải UTF-8 |
| `VD015` | NoVietnameseEngineInstalled | Info | Không có bất kỳ package Vietnamese IME nào |

Mỗi checker map 1-1 tới một Recommendation `VR###` (trừ INFO không cần fix).

### B.5. App-specific detector plugins

Khi user chạy `vietime-doctor --app <X>`, Orchestrator:

1. Resolve `<X>` → AppProfile (từ registry built-in hoặc user-supplied path).
2. Chạy `GenericAppDetector` + tất cả detector được `AppProfile.extra_detectors` khai báo.
3. Chạy additional checkers gắn tag `app:<X>`.

**AppProfile registry (hardcoded v0.1)**:

```rust
// crates/vietime-doctor/src/apps/registry.rs
pub const PROFILES: &[AppProfile] = &[
    AppProfile {
        id: "vscode",
        aliases: &["code", "visual-studio-code"],
        binary_hints: &["/usr/bin/code", "/usr/share/code/code", "/var/lib/flatpak/app/com.visualstudio.code"],
        kind_hint: AppKind::Electron,
        extra_detectors: &["app.electron", "app.electron.ozone_flags"],
        tags: &["electron", "chromium-based"],
    },
    AppProfile {
        id: "chrome",
        binary_hints: &["/usr/bin/google-chrome", "/opt/google/chrome/chrome"],
        kind_hint: AppKind::Chromium,
        extra_detectors: &["app.chromium_flags"],
        tags: &["chromium-based"],
    },
    AppProfile { id: "firefox", /* ... */ },
    AppProfile { id: "slack", kind_hint: AppKind::Electron, /* ... */ },
    AppProfile { id: "discord", kind_hint: AppKind::Electron, /* ... */ },
    AppProfile { id: "obsidian", kind_hint: AppKind::Electron, /* ... */ },
    AppProfile { id: "telegram", kind_hint: AppKind::Native, /* ... */ },
    AppProfile { id: "libreoffice", kind_hint: AppKind::Native, /* ... */ },
    AppProfile { id: "intellij", kind_hint: AppKind::Jvm, /* ... */ },
    AppProfile { id: "neovide", kind_hint: AppKind::Native, /* ... */ },
];
```

User có thể mở rộng qua `~/.config/vietime/apps.toml`.

### B.6. PII redaction

Mặc định redact **trước khi** render report. Scope redact:

| Field | Xử lý |
|---|---|
| `$HOME` | replace bằng `/home/$USER` constant → `/home/<user>` |
| username | từ `getuid` → replace mọi occurrence bằng `<user>` |
| hostname | từ `gethostname` → `<host>` |
| uuid-like strings (machine-id) | regex → `<uuid>` |
| IPs trong env (hiếm) | regex IPv4/IPv6 → `<ip>` |
| SSH keys, tokens, `*_TOKEN`, `*_KEY` env | remove value, giữ name `<redacted>` |
| Paths chứa repo private (không detect được) | không redact, docs warn |

`--no-redact` tắt redaction (cảnh báo rõ).

### B.7. Rendering

- **Markdown**: default, 1 template cho mỗi section. Dùng `minijinja` hoặc `handlebars` — chọn minijinja (nhỏ hơn).
- **JSON**: serialize trực tiếp `Report` với `serde_json::to_string_pretty`. Schema version stable (v1).
- **Plain**: tương tự markdown nhưng strip formatting, cho dán vào chat.

### B.8. Error handling policy

- `anyhow::Result` ở boundary (main, orchestrator).
- `thiserror` typed error trong library (`vietime-core`).
- **Không bao giờ panic**. `#[deny(clippy::unwrap_used, clippy::panic, clippy::expect_used)]` ở workspace root.
- Detector fail → `Fact` thiếu nhưng report vẫn render. Checker tương ứng emit `Info: "cannot determine X because Y"`.

### B.9. Concurrency

- Detector chạy song song qua `tokio::task::JoinSet`.
- Độc lập (không shared mutable state), kết quả merge sau.
- Timeout per-detector + timeout tổng (default 10s, override `--timeout`).

### B.10. Testing strategy

1. **Unit test**: mỗi detector test với fixture (file giả `/etc/os-release`, file giả `/etc/environment`).
2. **Golden test**: `insta` snapshot cho report markdown của 5 fixture distro.
3. **Integration test**: chạy thật trên CI matrix (GitHub Actions):
   - ubuntu-22.04 (X11 headless)
   - ubuntu-24.04 (Wayland headless via `weston`)
   - fedora-40 container
   - archlinux container
   Test kỳ vọng: exit code, không panic, JSON schema valid.
4. **Fuzz test**: feed random bytes vào env parser (clusterfuzz-lite style, optional).
5. **Manual test matrix**: checklist `docs/testing/manual-matrix.md` — 5 distro × 2 DE × 2 session.

Coverage target: 70% line coverage cho `vietime-core`, 50% cho `vietime-doctor` (CLI code).

### B.11. Build & distribution

- `cargo build --release` → `target/release/vietime-doctor` (~6MB sau strip).
- Cross-compile cho x86_64 + aarch64 (Apple Silicon VM, ARM Chromebook).
- Distribution:
  - **GitHub Release**: tar.gz chứa binary + LICENSE + README.
  - **Flatpak**: manifest `io.github.vietime.Doctor.yaml`, submit Flathub.
  - **`.deb`**: `cargo-deb`, ship on release.
  - **AUR**: `vietime-doctor-bin` + `vietime-doctor-git`.
  - **Cargo install**: `cargo install vietime-doctor` cho dev.

### B.12. Roadmap cụ thể Phase 1

| Tuần | Milestone |
|---|---|
| 1 | Workspace setup, `vietime-core` skeleton, distro/session/desktop detector + tests |
| 2 | Env detectors (process, /etc/environment, home dotfiles), render plain markdown |
| 3 | IBus detector (D-Bus via zbus), Fcitx5 detector, engine registry |
| 4 | App detectors (generic + electron), `--app` flag, AppProfile registry |
| 5 | Checkers VD001–VD008, Recommendation engine |
| 6 | Checkers VD009–VD015, JSON schema v1, PII redaction |
| 7 | Polish: docs vi+en, CI matrix, snapshot tests, fuzz env parser |
| 8 | Flatpak + deb packaging, v0.1.0 release, blog post + FB post |

Timebox cứng: **8 tuần**. Nếu tuần 6 chưa xong checker VD001–VD008, cắt các check còn lại sang v0.2.

### B.13. Risks (Phase 1)

| Risk | Mitigation |
|---|---|
| D-Bus API IBus/Fcitx5 không ổn định giữa version | Feature-detect interface, gracefully degrade |
| Parse `/etc/environment` shell quoting rắc rối | Dùng `shell-words` crate, có test với 20 edge case |
| Detect Electron version không reliable | Best-effort, đừng block report nếu fail |
| User dùng distro lạ (Void, NixOS, Alpine) | "Other" variant của enum, report vẫn ra, không crash |
| Report rò rỉ path chứa tên user trong `~/.config` | Aggressive redaction, `--no-redact` opt-in khi share offline |

### B.14. Acceptance criteria (checklist release v0.1.0)

- [ ] Chạy trên Ubuntu 22.04/24.04, Fedora 40, Arch, Pop!_OS — exit 0 khi mọi thứ OK, đúng issue khi broken.
- [ ] `--json` output validate được bằng JSON Schema (publish schema `schemas/report.v1.json`).
- [ ] Binary ≤ 8MB sau strip.
- [ ] `vietime-doctor check` < 500ms trên máy normal.
- [ ] README + USAGE vi + en đầy đủ.
- [ ] Flatpak submit sent to Flathub.
- [ ] 3 bug report thật đã được debug với report dán vào (dry run với maintainer).
- [ ] Không warning `clippy::pedantic` ngoài allowlist.
- [ ] License GPL-3.0, `SPDX-License-Identifier` header mọi file.
