# Backlog — v0.2+ ideas

> Tasks chưa assign vào phase cụ thể. Nơi parking cho: (a) ý tưởng hay nhưng sai thời điểm, (b) v0.2+ features, (c) task bị demote khỏi phase vì timebox.
>
> **Rule**: không chuyển task từ backlog vào phase đang chạy. Task ở đây chỉ được lên lịch tại checkpoint POL-50 khi plan v0.2.

---

## Cross-cutting

### BL-01 — GUI version (Slint hoặc Tauri)
- **Priority**: P2
- **Estimate**: XL (3–4 tuần)
- **Spec ref**: spec/00 §6, spec/01 §A.3 (in-scope v0.2+)
- **Note**: Đánh giá Slint (native, Rust-first) vs Tauri (webview). Cần khảo sát user P1 xem CLI đủ không trước khi invest.

### BL-02 — Telemetry opt-in (Plausible.io)
- **Priority**: P2
- **Estimate**: L (1 tuần)
- **Spec ref**: spec/04 §4
- **Note**: Chỉ làm sau khi có ≥ 1000 user.

### BL-03 — NixOS derivation + module
- **Priority**: P2
- **Estimate**: M (5d)
- **Spec ref**: spec/02 §A.3 (out-of-scope MVP)
- **Note**: Flake + NixOS module cho fcitx5-bamboo. Cần contributor Nix.

### BL-04 — Fedora Silverblue (rpm-ostree) support
- **Priority**: P3
- **Estimate**: L (1 tuần)
- **Spec ref**: spec/02 §B.15 (R ở Silverblue)
- **Note**: Immutable distro cần approach khác — layer image.

### BL-05 — Self-hosted analytics option
- **Priority**: P3
- **Estimate**: L
- **Spec ref**: spec/00 §5 (nguyên tắc #4)
- **Note**: Chỉ làm nếu Plausible không đáp ứng.

### BL-06 — i18n: thêm fr, zh-CN
- **Priority**: P3
- **Estimate**: M per locale
- **Spec ref**: spec/04 §5
- **Note**: Chỉ khi có contributor volunteer.

---

## Doctor v0.2+

### BL-10 — TUI live view (ratatui)
- **Priority**: P2
- **Estimate**: L (1 tuần)
- **Spec ref**: spec/01 §A.3
- **Note**: `vietime-doctor watch` — real-time env/daemon change detection.

### BL-11 — Latency micro-benchmark integration
- **Priority**: P2
- **Estimate**: L
- **Spec ref**: spec/01 §A.3
- **Note**: Đo latency từ keystroke → hiển thị. Tích hợp Bench.

### BL-12 — App-specific detector plugin framework
- **Priority**: P2
- **Estimate**: XL
- **Spec ref**: spec/01 §A.3
- **Note**: Dynamic loadable detector qua `.so` hoặc WASM plugin. Chỉ khi registry hardcode > 30 app.

### BL-13 — Auto-fix mode (merge Doctor + Installer)
- **Priority**: P3
- **Estimate**: M
- **Spec ref**: spec/01 §A.3 (out-of-scope)
- **Note**: `vietime-doctor fix --apply` shell ra Installer commands. Cần an toàn.

### BL-14 — Electron/Chromium process scanning
- **Priority**: P2
- **Estimate**: M
- **Note**: Scan `/proc/*/cmdline` tìm Electron đang chạy, kiểm Ozone flags.

### BL-15 — `GLFW_IM_MODULE` chuyên sâu
- **Priority**: P3
- **Estimate**: S
- **Note**: GLFW hiện chỉ hiểu `ibus`. Detect game engine (Unity/Godot) có GLFW bundled.

---

## Installer v0.2+

### BL-20 — Slint/Tauri GUI installer
- **Priority**: P2
- **Estimate**: XL
- **Spec ref**: spec/02 §A.3
- **Note**: Cho P1 persona thực sự không thoải mái CLI/TUI.

### BL-21 — Resume interrupted install
- **Priority**: P2
- **Estimate**: M
- **Spec ref**: spec/02 §A.3, §B.10 (#5)
- **Note**: Robustness vs SIGKILL. Yêu cầu state machine kỹ hơn.

### BL-22 — Offline bundle (download once, apply later)
- **Priority**: P2
- **Estimate**: L
- **Spec ref**: spec/02 §A.3
- **Note**: Cho user có mạng chập chờn.

### BL-23 — Snap package cho VS Code warning + deb redirect
- **Priority**: P1
- **Estimate**: S
- **Spec ref**: spec/05 §4 (R14)
- **Note**: Detect VS Code snap → khuyến khích .deb. Có thể auto-swap nếu user confirm.

### BL-24 — Fcitx5-Bamboo build from source fallback
- **Priority**: P1
- **Estimate**: L
- **Spec ref**: spec/02 §B.15 (fallback fcitx5-bamboo thiếu trên Fedora)
- **Note**: Clone repo, build deps, build, install. Slow nhưng reliable.

### BL-25 — `pkexec` thay `sudo`
- **Priority**: P3
- **Estimate**: M
- **Spec ref**: spec/02 §B.6
- **Note**: PolicyKit integration, GUI prompt, không lộ password qua stdin.

### BL-26 — Ubuntu LTS version detection + warnings
- **Priority**: P2
- **Estimate**: S
- **Spec ref**: spec/05 §4 (R19)
- **Note**: Cảnh báo user Ubuntu 20.04 (EOL) nên upgrade.

---

## Bench v0.2+

### BL-30 — Latency measurement per keystroke
- **Priority**: P1
- **Estimate**: L
- **Spec ref**: spec/03 §A.3
- **Note**: Đo ms từ `xdotool key` → window text change. Rất giá trị cho regression test.

### BL-31 — Undo history test
- **Priority**: P2
- **Estimate**: M
- **Spec ref**: spec/03 §A.3
- **Note**: Gõ → Ctrl+Z → verify. Fake backspace phá undo là pain #1.

### BL-32 — Fake-backspace counting
- **Priority**: P2
- **Estimate**: M
- **Spec ref**: spec/03 §A.3
- **Note**: Count số BS thật được gửi vs mong đợi.

### BL-33 — Chromium autocomplete interference test
- **Priority**: P2
- **Estimate**: L
- **Spec ref**: spec/03 §A.3
- **Note**: Trigger intellisense + gõ Telex cùng lúc. Reproduce Electron pain.

### BL-34 — Additional app runners
- **Priority**: P2
- **Estimate**: M each
- **Note**: Notion, Figma, Zoom, Telegram Desktop (not Flatpak), Thunderbird, Joplin, Zed editor.

### BL-35 — Web-based test vector editor
- **Priority**: P3
- **Estimate**: L
- **Spec ref**: spec/03 §A.3
- **Note**: CMS cho vector. Chỉ khi có contributor đóng góp thường xuyên.

### BL-36 — Distributed runner
- **Priority**: P3
- **Estimate**: XL
- **Spec ref**: spec/03 §A.3
- **Note**: Chạy combo trên nhiều VM song song. Chỉ khi `full` profile vượt 2h.

### BL-37 — VNI mode test vectors
- **Priority**: P2
- **Estimate**: L
- **Note**: Hiện MVP chỉ Telex. VNI (gõ số) cần bộ 500 vector riêng.

### BL-38 — VIQR mode test vectors
- **Priority**: P3
- **Estimate**: L
- **Note**: VIQR ít user hơn, priority thấp.

### BL-39 — Dashboard historical charts
- **Priority**: P2
- **Estimate**: M
- **Note**: Accuracy over time (engine version axis). Chart.js đủ.

### BL-40 — Bench cho Wayland gnome-shell native apps
- **Priority**: P1
- **Estimate**: L
- **Note**: AT-SPI trong Weston headless có thể không work. Cần rig khác.

---

## Research / experimental

### BL-50 — Electron Ozone patch contribution
- **Priority**: P3
- **Estimate**: XL (tháng)
- **Spec ref**: tài liệu gốc §2 khe hở 5
- **Note**: Rất khó, có thể không bao giờ làm. Keep trong backlog như "nếu rảnh".

### BL-51 — text-input-v3 protocol experimentation
- **Priority**: P3
- **Estimate**: XL
- **Spec ref**: tài liệu gốc §4 ngoại lệ
- **Note**: PhD-level. Có thể viết blog post thôi.

### BL-52 — IBus vs Fcitx5 head-to-head whitepaper
- **Priority**: P3
- **Estimate**: L
- **Note**: Dùng Bench data để write authoritative comparison. Giá trị community cao.

### BL-53 — Wayland text-input-v3 conformance test
- **Priority**: P3
- **Estimate**: XL
- **Note**: Test compositor compliance. Upstream value nhưng không phải Việt-specific.

---

## Documentation

### BL-60 — Video tutorial "Cài gõ tiếng Việt Ubuntu trong 5 phút"
- **Priority**: P1
- **Estimate**: M (1 tuần production)
- **Spec ref**: spec/05 §4 (R9 mitigation)
- **Note**: YouTube. High leverage cho P1 persona.

### BL-61 — Interactive troubleshooter website
- **Priority**: P2
- **Estimate**: L
- **Note**: "Click bug loại nào bạn gặp → show fix" trên site.

### BL-62 — Integrated mdBook site với search
- **Priority**: P2
- **Estimate**: M
- **Spec ref**: spec/04 §6.3
- **Note**: Elasticsearch-lite hoặc `mdbook-search`.

### BL-63 — English FOSDEM lightning talk
- **Priority**: P3
- **Estimate**: M
- **Note**: Nếu dự án có traction quốc tế.

---

## Operational

### BL-70 — Sponsorship / GitHub Sponsors setup
- **Priority**: P3
- **Estimate**: S
- **Note**: Chỉ khi có user base thật. Tránh premature monetization.

### BL-71 — Co-maintainer recruitment & handover
- **Priority**: P1 (khi cần)
- **Estimate**: Ongoing
- **Spec ref**: spec/05 §5 (R10 mitigation)
- **Note**: Để tránh bus factor = 1.

### BL-72 — Release automation: release-plz
- **Priority**: P2
- **Estimate**: M
- **Note**: Auto-bump version, CHANGELOG, tag. Rust ecosystem standard.
