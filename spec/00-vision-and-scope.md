# VietIME Suite — Vision & Scope

> Master document. Toàn bộ spec con (01, 02, 03, 04, 05) đều tham chiếu vào đây. Đọc file này trước.

---

## 0. Một câu định nghĩa

**VietIME Suite là một bộ công cụ phụ trợ (không phải IME engine) giúp người dùng Linux Việt cài, chẩn đoán, và đo lường chất lượng gõ tiếng Việt của họ — xây dựng trên các IME engine có sẵn (ibus-bamboo, fcitx5-bamboo, fcitx5-unikey).**

Chúng ta **không** viết bộ gõ mới. Chúng ta viết 3 công cụ bao quanh các bộ gõ hiện có để giảm pain points ở tầng setup, chẩn đoán, và quality assurance.

---

## 1. Vấn đề (dẫn lại, ngắn)

Người dùng Linux Việt hiện chịu 3 lớp đau:

1. **Setup hell** — cài fcitx5-bamboo phải sửa ~5 biến môi trường, gỡ IBus, fix conflict, reboot. Không có one-click installer.
2. **Chẩn đoán mù** — khi gõ bị lỗi (mất dấu trong Electron, loạn chữ Google Docs, fake backspace vỡ undo), user không biết lỗi ở đâu, không có log, không biết báo bug ra sao.
3. **Không có dữ liệu khách quan** — "app X có gõ được không", "mode Y có bug không" đều là truyền miệng. Không có compatibility matrix.

Chi tiết pain points: tham chiếu tài liệu đánh giá gốc (`danh-gia-du-an-bo-go-tieng-viet.md`) mục §1.

---

## 2. 3 components

| # | Tên | Giải quyết đau nào | MVP timebox |
|---|---|---|---|
| 1 | **VietIME Doctor** | Chẩn đoán mù | 4–8 tuần |
| 2 | **VietIME Installer** | Setup hell | 2–4 tuần |
| 3 | **VietIME Bench** | Không có dữ liệu | 6–10 tuần |

Ba công cụ này **độc lập** — dùng riêng cũng có giá trị. Nhưng kết hợp thì tạo flywheel:

- Installer dùng Doctor để verify cài đặt thành công.
- Bench dùng Doctor để ghi nhận môi trường test.
- Doctor output có thể được share để Bench biết user đang chạy config gì khi gặp bug.

---

## 3. Non-goals — đừng lẫn lộn

**Dự án này KHÔNG làm**:

- ❌ Viết IME engine mới (Telex/VNI parsing).
- ❌ Fork `bamboo-core` hoặc `unikey-core`.
- ❌ Thay thế IBus hoặc Fcitx5.
- ❌ Patch upstream Electron/Chromium (đụng vào thì là dự án khác).
- ❌ Làm GUI config cho bộ gõ (đã có Fcitx5 GUI).
- ❌ Hỗ trợ Windows/macOS ở phase 1. Pure Linux.
- ❌ Host server, analytics tự host, backend database. Zero infrastructure ở MVP.

Non-goal là **ranh giới để tránh scope creep**. Mỗi khi đứng trước một feature, hỏi: "cái này có đẩy ta sang một trong 6 gạch trên không?" Nếu có → từ chối.

---

## 4. Users — 3 personas

**P1. "Developer mới lên Linux"** (60% user base dự kiến)
- Vừa chuyển từ Windows/macOS sang Ubuntu/Fedora.
- Biết terminal nhưng không rành IM stack.
- Pain: cài xong không gõ được, không biết hỏi ai.
- Success cho họ: chạy `vietime-installer`, 2 phút sau gõ được tiếng Việt trong VS Code.

**P2. "Developer Linux kỳ cựu, bị Electron bug"** (30%)
- Dùng Linux 3+ năm, biết cấu hình IM.
- Pain: mất dấu trong VS Code/Slack/Discord, không biết workaround nào chuẩn.
- Success cho họ: `vietime-doctor --app vscode` chỉ ra config nào đang sai và đề xuất fix.

**P3. "Maintainer / contributor bộ gõ"** (10%)
- Maintainer của ibus-bamboo, fcitx5-bamboo, hoặc reviewer PR.
- Pain: bug report không đủ thông tin, không có regression test.
- Success cho họ: `vietime-doctor report` → paste vào issue; `vietime-bench --mode telex --app vscode` → số liệu trước/sau fix.

---

## 5. Nguyên tắc thiết kế (Design Tenets)

Khi có 2 lựa chọn, luôn chọn theo thứ tự ưu tiên:

1. **An toàn trước tính năng** — không bao giờ sửa `/etc/environment` mà không backup. Rollback là mặc định.
2. **Read-only trước write** — Doctor đọc config, Installer mới ghi. Mọi tool phải có `--dry-run`.
3. **Plain text output trước GUI** — CLI trước, TUI sau, GUI cuối. CLI output phải paste được thẳng vào GitHub issue.
4. **Single static binary trước runtime deps** — Rust/Go, không Python/Node.
5. **Không telemetry ngầm** — opt-in tường minh, hiển thị đúng dữ liệu gửi đi trước khi gửi.
6. **Tương thích IBus và Fcitx5 ngang bằng** — không ép user chọn bên.
7. **Wayland và X11 ngang bằng** — không giả định session loại nào.
8. **Fail loud, fail early** — thà từ chối chạy còn hơn làm hỏng cấu hình user.

---

## 6. Tech stack — quyết định chung

Ba component dùng chung tech stack để giảm maintenance burden.

| Layer | Chọn | Lý do |
|---|---|---|
| Ngôn ngữ chính | **Rust** (edition 2021) | Single static binary, không runtime deps, cross-compile dễ, error handling nghiêm |
| CLI framework | `clap` v4 với derive | De-facto standard, subcommand tốt |
| Logging | `tracing` + `tracing-subscriber` | Structured log, level filter, có thể emit JSON cho report |
| Config file | TOML via `toml` + `serde` | User-readable, chuẩn Rust ecosystem |
| D-Bus | `zbus` (async) | Pure Rust, không cần libdbus dev headers |
| Async runtime | `tokio` (multi-thread, features=["full"]) | Chỉ bật trong Doctor/Bench, Installer có thể sync |
| Error type | `thiserror` (library) + `anyhow` (binary) | Pattern chuẩn Rust |
| Testing | `cargo test` + `insta` snapshot + `rstest` parameterized | |
| GUI (phase 2) | Không ở MVP. Nếu cần sau, dùng **Slint** (native, Rust-first) hoặc Tauri | |
| Packaging | **Flatpak** (ưu tiên), `.deb` (ưu tiên 2), **AUR** | Flatpak chạy mọi distro, `.deb` cho Ubuntu user base |
| License | **GPLv3** | Tương thích upstream IBus/Fcitx5 |
| Code repo layout | **Cargo workspace monorepo**: `crates/vietime-core`, `crates/vietime-doctor`, `crates/vietime-installer`, `crates/vietime-bench` | Share code (enum distro, env parsing) |

**Lý do KHÔNG chọn Go**: bamboo-core viết Go, nhưng chúng ta không wrap bamboo-core ở 3 công cụ này — chỉ detect/measure nó. Rust cho phép single static binary nhỏ hơn (<5MB) và cross-compile Wayland tooling sạch hơn.

**Lý do KHÔNG chọn Python**: Python env trên Linux là mìn. User cài xong không chạy được vì thiếu `python3-dbus` là UX thua ngay từ đầu.

---

## 7. Cấu trúc repo đề xuất

```
vietime/                             # repo gốc
├── Cargo.toml                       # workspace root
├── Cargo.lock
├── crates/
│   ├── vietime-core/                # shared: distro detect, env parser, IM enum
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── distro.rs            # enum Distro, parse /etc/os-release
│   │   │   ├── session.rs           # enum Session (X11/Wayland), detect
│   │   │   ├── im_framework.rs      # enum ImFramework (IBus/Fcitx5/None)
│   │   │   ├── env.rs               # parse GTK_IM_MODULE etc.
│   │   │   └── report.rs            # Report struct, JSON/Markdown render
│   │   └── Cargo.toml
│   ├── vietime-doctor/              # Phase 1 — spec 01
│   ├── vietime-installer/           # Phase 2 — spec 02
│   └── vietime-bench/               # Phase 3 — spec 03
├── docs/                            # user-facing docs (vi + en)
│   ├── vi/
│   └── en/
├── packaging/
│   ├── flatpak/
│   ├── deb/
│   └── aur/
├── spec/                            # copy của spec này để user nhìn thấy
└── README.md
```

---

## 8. Thứ tự triển khai và quyết định phụ thuộc

```
Phase 0 (Tuần 1–4)
   └─ Setup repo, workspace, CI, `vietime-core` skeleton
        │
        ▼
Phase 1 — Doctor  ◄──── spec 01
   └─ Phát hành v0.1.0 trên Flatpak + blog post
        │
        ▼
Phase 2 — Installer  ◄──── spec 02
   └─ Reuse `vietime-core`. Phát hành v0.1.0.
        │
        ▼
Phase 3 — Bench  ◄──── spec 03
   └─ Reuse `vietime-core` + một phần Doctor
```

**Quan trọng**: `vietime-core` phải stabilize trước khi Phase 2 bắt đầu. Nếu giữa Phase 1 phát hiện core API sai, refactor ngay, đừng đẩy nợ kỹ thuật sang Phase 2.

---

## 9. Success metrics (toàn project)

MVP thành công khi đạt **tất cả** những điều dưới đây trong 12 tháng đầu:

| Metric | Mục tiêu 6 tháng | Mục tiêu 12 tháng |
|---|---|---|
| GitHub stars | 200 | 1000 |
| Downloads (Flatpak + deb) | 2k | 10k |
| Contributors (non-author) | 3 | 10 |
| Bug reports có dán Doctor report | 10 | 100 |
| Distro covered (Ubuntu, Fedora, Arch, Debian, Pop!_OS) | 3 | 5 |
| Compat Matrix (app × mode) rows | 100 | 500 |
| Đề cập trong blog/forum Việt | 5 | 30 |

**Anti-metrics** (cảnh báo đi lạc):
- Số dòng code engine > 0 (nghĩa là đã viết IME, đi sai hướng).
- Commit/tuần < 2 trong 4 tuần liên tiếp (motivation drain).
- Tỷ lệ issue đóng trong 30 ngày < 30% (burnout sắp tới).

---

## 10. Các giả định (Assumptions) cần validate sớm

Mỗi assumption dưới đây nếu sai sẽ làm đổi hướng dự án. Validate ngay trong Phase 0.

- **A1**: Fcitx5 là IM framework future-proof hơn IBus cho Wayland → cần kiểm qua bug tracker và Ubuntu 24.04 behavior.
- **A2**: Maintainer ibus-bamboo/fcitx5-bamboo không phản đối có tool phụ trợ bên ngoài → gửi message, chờ phản hồi.
- **A3**: User Việt chịu cài Flatpak (ít nhất Ubuntu user) → hỏi 10 user trong nhóm FB Linux Vietnam.
- **A4**: `ydotool`/`xdotool` đủ để automate gõ trong headless session cho Bench → PoC 1 tuần.
- **A5**: D-Bus introspection đủ để query trạng thái IBus/Fcitx5 mà không cần patch chúng → PoC 3 ngày.

Nếu A1/A2/A4/A5 fail: điều chỉnh. A3 fail: thêm `.deb` path.

---

## 11. Đọc tiếp

- Phase 1 Doctor → `spec/01-phase1-doctor.md`
- Phase 2 Installer → `spec/02-phase2-installer.md`
- Phase 3 Bench → `spec/03-phase3-test-suite.md`
- Cross-cutting (naming, telemetry, release, license) → `spec/04-cross-cutting.md`
- Roadmap + risk register → `spec/05-roadmap-risks.md`
