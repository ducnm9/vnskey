# VietIME Suite

[![CI](https://github.com/vietime/vietime/actions/workflows/ci.yml/badge.svg)](https://github.com/vietime/vietime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Rust: 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange)](Cargo.toml)

> Status: **Phase 0 — Validate & Setup**. Foundation ready, user research in progress.

---

## Specification

Đây là tài liệu spec đầy đủ cho **VietIME Suite**, một bộ công cụ phụ trợ (không phải IME engine mới) giúp người dùng Linux Việt cài, chẩn đoán, và đo lường chất lượng gõ tiếng Việt.

Dự án được chuẩn bị dựa trên đánh giá khả thi [`danh-gia-du-an-bo-go-tieng-viet.md`](../Library/Application%20Support/Claude-3p/local-agent-mode-sessions/a1951980-2c69-4f5a-a032-f4ae76d1d01f/00000000-0000-4000-8000-000000000001/local_6076a2ee-9c03-4178-8f95-3bb83ceb410a/outputs/danh-gia-du-an-bo-go-tieng-viet.md). Kết luận cốt lõi: **không viết bộ gõ mới** — thay vào đó xây 3 công cụ xung quanh hệ sinh thái hiện có (ibus-bamboo, fcitx5-bamboo, fcitx5-unikey).

---

## Cấu trúc spec

Đọc theo thứ tự:

1. **[00 — Vision & Scope](spec/00-vision-and-scope.md)** — master doc. Vision, non-goals, personas, tech stack chung, cấu trúc repo, thứ tự triển khai. **Đọc đầu tiên.**
2. **[01 — Phase 1: VietIME Doctor](spec/01-phase1-doctor.md)** — công cụ chẩn đoán. PRD + tech design chi tiết: detectors, checkers, data model, CLI surface, testing.
3. **[02 — Phase 2: VietIME Installer](spec/02-phase2-installer.md)** — one-click installer. Planner/Executor architecture, atomic snapshot rollback, package manager abstraction.
4. **[03 — Phase 3: VietIME Bench](spec/03-phase3-test-suite.md)** — compatibility matrix runner. Headless session driver, keystroke injection, dashboard.
5. **[04 — Cross-cutting](spec/04-cross-cutting.md)** — naming, license, i18n, release process, security, performance budget, anti-patterns.
6. **[05 — Roadmap & Risks](spec/05-roadmap-risks.md)** — timeline 12 tháng, risk register với mitigation, kill criteria.

---

## TL;DR

| Component | Giải quyết | MVP timebox |
|---|---|---|
| **Doctor** (Phase 1) | "Tại sao gõ của tôi bị lỗi?" — không ai biết | 8 tuần |
| **Installer** (Phase 2) | "Cài fcitx5-bamboo xong không gõ được" — setup hell | 8 tuần |
| **Bench** (Phase 3) | "Combo nào gõ VS Code được?" — không có dữ liệu | 10 tuần |

**Stack**: Rust workspace, GPLv3, zero server, Flatpak + deb/rpm/AUR distribution.

**Không làm**: viết IME engine mới, fork bamboo-core, patch Electron upstream, Windows/macOS support.

---

## Các quyết định quan trọng đã chốt trong spec

- **Ngôn ngữ**: Rust (single static binary, cross-compile dễ, không runtime deps). Không Python/Node.
- **License**: GPLv3 để tương thích upstream IBus/Fcitx5.
- **Repo**: monorepo Cargo workspace với 4 crates.
- **Phân phối**: Flatpak (Doctor, Bench) + native .deb/.rpm/AUR (Installer bắt buộc native vì cần `/etc`).
- **Telemetry**: KHÔNG ở MVP v0.1. Opt-in v0.2 nếu cần.
- **i18n**: vi + en. `fluent` i18n.
- **Priority distro**: Ubuntu 22.04+, Debian 12+, Fedora 39+, Arch, Pop!_OS. NixOS v0.2+.

---

## Task board

Chi tiết task từng phase (kèm ID, priority, estimate, acceptance criteria) ở [`tasks/`](tasks/):

- [`tasks/README.md`](tasks/README.md) — convention, format, status workflow
- [`tasks/phase-0-validate.md`](tasks/phase-0-validate.md) — 17 task (P0-##)
- [`tasks/phase-1-doctor.md`](tasks/phase-1-doctor.md) — 42 task (DOC-##)
- [`tasks/phase-2-installer.md`](tasks/phase-2-installer.md) — 36 task (INS-##)
- [`tasks/phase-3-bench.md`](tasks/phase-3-bench.md) — 37 task (BEN-##)
- [`tasks/phase-4-polish.md`](tasks/phase-4-polish.md) — 21 task (POL-##)
- [`tasks/backlog.md`](tasks/backlog.md) — 41 task v0.2+ (BL-##)

**Tổng: 194 task được lên kế hoạch** từ spec, với estimate + priority + acceptance criteria.

## Next steps

Bắt đầu từ `tasks/phase-0-validate.md` P0-01. Theo `spec/05-roadmap-risks.md` §2:

**Phase 0 (4 tuần đầu)**:
1. Build `ibus-bamboo` + `fcitx5-bamboo` local, ghi lại ≥ 20 pain points.
2. Post user research trên nhóm Facebook Linux Vietnam → 15 feedback.
3. Gửi DM maintainer upstream giới thiệu dự án.
4. Validate 5 assumptions A1–A5 (xem `00` §10).
5. Setup Cargo workspace, CI, `vietime-core` skeleton.
6. Viết blog post "Tại sao gõ tiếng Việt trên Ubuntu khó".
7. **Go/no-go checkpoint** cuối tuần 4.

---

## Quy trình phát triển

- **[docs/dev/process.md](docs/dev/process.md)** — quy trình dev/QA/release cho solo part-time. Đọc trước khi bắt đầu code.
- Decision log (khi bắt đầu code): `docs/dev/decisions.md`.
- Daily log: `docs/dev/log/YYYY-MM-DD.md`.

---

## Tác giả spec & đóng góp

- Spec draft khởi tạo: 2026-04-24.
- Đóng góp spec: mở PR vào `spec/*.md`, theo Conventional Commits format.
