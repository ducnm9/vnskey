# Phase 0 — Validate & Setup (Tháng 1, 4 tuần)

> **Goal**: validate 5 assumptions, thu thập 15+ user feedback, setup foundation. **Không code production**.
>
> **Exit criteria** (spec/05 §2): `vietime-core` compile + test pass, 1 blog post live, go/no-go quyết định cuối tuần 4.
>
> **Budget**: 40–60h.

---

## Track A — User Research

### P0-01 — Build ibus-bamboo locally, thử 5 app
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (6h)
- **Depends on**: —
- **Spec ref**: spec/05 §2 (Phase 0 W1)
- **Acceptance**:
  - [ ] Ubuntu 22.04 VM hoặc máy thật build thành công từ source.
  - [ ] Gõ Telex trong ≥ 5 app: gedit, Firefox, Chrome, VS Code, Slack.
  - [ ] Ghi log 10+ pain point cụ thể trong `docs/research/pain-points-ibus.md`.

### P0-02 — Build fcitx5-bamboo locally, thử 5 app
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (6h)
- **Depends on**: —
- **Spec ref**: spec/05 §2 (Phase 0 W1)
- **Acceptance**:
  - [ ] Build thành công (fcitx5 trước, sau đó fcitx5-bamboo addon).
  - [ ] Gõ Telex trong cùng 5 app như P0-01.
  - [ ] Ghi log pain point trong `docs/research/pain-points-fcitx5.md`.

### P0-03 — So sánh IBus vs Fcitx5 Wayland behavior
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (3h)
- **Depends on**: P0-01, P0-02
- **Spec ref**: spec/00 §10 (validate A1)
- **Acceptance**:
  - [ ] Test cả 2 trên Ubuntu 24.04 Wayland + X11.
  - [ ] Kết luận A1 (Fcitx5 future-proof hơn) pass hay fail, ghi vào `docs/dev/decisions.md` (D-A1).

### P0-04 — Post user research trên cộng đồng Việt
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (2h)
- **Depends on**: —
- **Spec ref**: spec/05 §2 (Phase 0 W1)
- **Acceptance**:
  - [ ] Post "Tôi đang khảo sát pain points gõ tiếng Việt Linux..." trên:
    - [ ] Nhóm Facebook "Linux Việt Nam" / "Ubuntu Việt Nam"
    - [ ] r/vietnam hoặc subreddit Việt khác
    - [ ] 1 Discord/Telegram group dev Việt
  - [ ] Ít nhất 15 phản hồi/comment thu thập được trong 2 tuần.
  - [ ] Tổng hợp vào `docs/research/user-feedback.md`.

### P0-05 — DM maintainer upstream (ibus-bamboo + fcitx5-bamboo)
- **Status**: TODO
- **Priority**: P0
- **Estimate**: XS (1h)
- **Depends on**: P0-01, P0-02
- **Spec ref**: spec/05 §2 (Phase 0 W2); spec/05 §4 (R3 mitigation)
- **Acceptance**:
  - [ ] Gửi email/GitHub Discussion message cho maintainer ibus-bamboo.
  - [ ] Gửi cho maintainer fcitx5-bamboo.
  - [ ] Framing: "tool bổ trợ bên ngoài, không cạnh tranh", hỏi pain point ưu tiên.
  - [ ] Ghi phản hồi (nếu có) vào `docs/research/upstream-contact.md`.

### P0-06 — Validate A3 — user Việt chịu Flatpak?
- **Status**: TODO
- **Priority**: P1
- **Estimate**: XS (1h)
- **Depends on**: P0-04
- **Spec ref**: spec/00 §10 (A3)
- **Acceptance**:
  - [ ] Hỏi 10 người trong nhóm Linux VN: "Bạn có dùng Flatpak/đang cài được không?"
  - [ ] Nếu < 5 có: spec Phase 2 phải ưu tiên `.deb`/`.rpm` lớn hơn Flatpak.
  - [ ] Ghi vào `docs/dev/decisions.md` (D-A3).

---

## Track B — Foundation Setup

### P0-10 — Tạo GitHub repo + README gốc
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (2h)
- **Depends on**: —
- **Spec ref**: spec/00 §7, spec/04 §8.1
- **Acceptance**:
  - [ ] Repo public tại `github.com/<user>/vietime`.
  - [ ] README giới thiệu vision + link tới spec/.
  - [ ] Copy spec/*.md + README.md vào repo.
  - [ ] LICENSE GPL-3.0.
  - [ ] CODE_OF_CONDUCT.md (Contributor Covenant).
  - [ ] SECURITY.md placeholder.

### P0-11 — Cargo workspace skeleton
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: P0-10
- **Spec ref**: spec/00 §7
- **Acceptance**:
  - [ ] `Cargo.toml` workspace root với 4 members: `vietime-core`, `vietime-doctor`, `vietime-installer`, `vietime-bench`.
  - [ ] Mỗi crate có `src/lib.rs` hoặc `src/main.rs` với `pub fn hello() -> &'static str`.
  - [ ] `cargo build --workspace` xanh.
  - [ ] `cargo test --workspace` xanh (1 dummy test).
  - [ ] Cargo edition 2021.

### P0-12 — Lint/format/deny config
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (2h)
- **Depends on**: P0-11
- **Spec ref**: spec/04 §12
- **Acceptance**:
  - [ ] `rustfmt.toml` checked in (default + `edition = "2021"`).
  - [ ] Workspace `lints.clippy` in `Cargo.toml`: `unwrap_used`, `expect_used`, `panic` = deny.
  - [ ] `cargo-deny` config `deny.toml` với license allowlist (MIT/Apache-2.0/BSD/GPL-compat).
  - [ ] `cargo fmt --check` và `cargo clippy -D warnings` pass.

### P0-13 — CI workflow (GitHub Actions)
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: P0-11, P0-12
- **Spec ref**: spec/04 §8.4
- **Acceptance**:
  - [ ] `.github/workflows/ci.yml` run: fmt check, clippy, test, cargo-deny check.
  - [ ] Matrix: ubuntu-22.04, ubuntu-24.04.
  - [ ] Badge vào README.
  - [ ] Fail trên PR khi bất kỳ step fail.
  - [ ] Run time < 5 phút.

### P0-14 — `vietime-core` initial types
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (6h)
- **Depends on**: P0-13
- **Spec ref**: spec/01 §B.2
- **Acceptance**:
  - [ ] `src/distro.rs`: `Distro` enum + `detect_from_os_release(&str) -> Option<Distro>` + test 5 fixture (Ubuntu 22/24, Debian 12, Fedora 40, Arch).
  - [ ] `src/session.rs`: `SessionType` enum + `detect_from_env(&HashMap) -> SessionType` + tests.
  - [ ] `src/im_framework.rs`: `ImFramework` enum.
  - [ ] `src/env.rs`: `EnvFacts` struct + parser `parse_etc_environment(&str) -> HashMap`.
  - [ ] Coverage ≥ 70% cho `vietime-core`.

### P0-15 — SPDX headers + license compliance
- **Status**: TODO
- **Priority**: P2
- **Estimate**: XS (1h)
- **Depends on**: P0-14
- **Spec ref**: spec/04 §3
- **Acceptance**:
  - [ ] Mọi file `.rs` có header `// SPDX-License-Identifier: GPL-3.0-or-later`.
  - [ ] Script `scripts/check-spdx.sh` thêm vào CI.

### P0-16 — Repo hygiene files
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (2h)
- **Depends on**: P0-10
- **Spec ref**: spec/04 §9
- **Acceptance**:
  - [ ] `.gitignore` với `target/`, `.DS_Store`, `*.log`, `node_modules/`.
  - [ ] `.editorconfig` (4-space Rust, LF, trim whitespace).
  - [ ] Issue templates: `bug.yml`, `feature.yml`, `compat-matrix-entry.yml` trong `.github/ISSUE_TEMPLATE/`.
  - [ ] PR template `.github/pull_request_template.md`.

---

## Track C — Outbound

### P0-20 — Blog post #1 "Tại sao gõ tiếng Việt trên Ubuntu khó"
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (8h)
- **Depends on**: P0-01, P0-02, P0-04
- **Spec ref**: spec/05 §2 (Phase 0 W4)
- **Acceptance**:
  - [ ] Bài đăng Medium/dev.to/Hashnode hoặc `docs/blog/001-...md`.
  - [ ] 1200–2000 từ tiếng Việt.
  - [ ] Có 3+ đoạn code/config thật; screenshot minh họa.
  - [ ] Dẫn link về repo + spec.
  - [ ] Share lên 3 kênh (FB group, Reddit, dev friends).
  - [ ] Track 50+ views trong 2 tuần.

### P0-21 — Decision log bootstrapping
- **Status**: TODO
- **Priority**: P1
- **Estimate**: XS (1h)
- **Depends on**: P0-10
- **Spec ref**: spec/05 §5
- **Acceptance**:
  - [ ] `docs/dev/decisions.md` tạo với 5 seed decisions (D1–D5 trong spec/05 §5).
  - [ ] Format template ghi trong file.

### P0-22 — Weekly log bootstrapping
- **Status**: TODO
- **Priority**: P2
- **Estimate**: XS (0.5h)
- **Depends on**: P0-10
- **Spec ref**: spec/05 §8
- **Acceptance**:
  - [ ] `docs/dev/weekly-log.md` tạo với format entry mẫu.
  - [ ] Tuần 1 đã có entry đầu tiên.

---

## Track D — Checkpoint

### P0-30 — Go/No-Go checkpoint cuối tuần 4
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (2h)
- **Depends on**: P0-01, P0-02, P0-03, P0-04, P0-05, P0-14, P0-20
- **Spec ref**: spec/05 §2 (Phase 0 W4)
- **Acceptance**:
  - [ ] Tự review 5 assumption A1–A5, mỗi cái: PASS / FAIL / ADJUSTED.
  - [ ] Review feedback user (target ≥ 15 responses): **có** đau thật hay không?
  - [ ] Review maintainer response: tích cực / trung tính / tiêu cực.
  - [ ] Quyết định: tiếp Phase 1 / điều chỉnh spec / STOP.
  - [ ] Ghi quyết định + reasoning vào `docs/dev/decisions.md` (D-GO-1).

---

## Phase 0 — Exit checklist

Trước khi chuyển sang Phase 1, **tất cả** phải DONE:

- [ ] P0-01, P0-02, P0-03 (đã thử cả 2 engine)
- [ ] P0-04, P0-05, P0-06 (user + maintainer research xong)
- [ ] P0-10 → P0-14 (repo + CI + core skeleton xanh)
- [ ] P0-20 (blog live)
- [ ] P0-30 (go/no-go = GO)

Nếu P0-30 = STOP hoặc ADJUSTED: **không** bắt đầu Phase 1. Rewrite spec hoặc archive.
