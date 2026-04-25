# Architectural Decision Log

> Mỗi quyết định lớn được ghi ở đây theo format ADR. Xem `docs/dev/process.md` §10
> để biết khi nào cần entry mới.

Format mỗi entry:

```markdown
## D-YYYY-MM-DD-NN — <Title>

**Status**: proposed | accepted | superseded by D-...
**Context**: <what forced the decision>
**Decision**: <what we chose>
**Alternatives considered**: <brief list>
**Consequences**: <positive + negative>
```

---

## D-2026-04-24-01 — Không viết bộ gõ mới, làm 3 công cụ phụ trợ

**Status**: accepted
**Context**: Sau khi đánh giá hệ sinh thái hiện tại (spec gốc
`danh-gia-du-an-bo-go-tieng-viet.md`), kết luận là thị phần gõ tiếng Việt trên
Linux đã bị **bamboo-core + ibus-bamboo/fcitx5-bamboo** lấp đầy ở tầng engine.
Viết engine mới không giải quyết pain point của user; pain nằm ở tầng setup,
diagnosis, và QA.

**Decision**: Dự án VietIME Suite gồm 3 tool (**Doctor**, **Installer**, **Bench**)
chạy xung quanh bộ gõ hiện có, không fork / không rewrite engine.

**Alternatives considered**:
- (a) Viết IME engine mới bằng Rust — bị reject: ≥ 12 tháng effort, không có
  pain user nào được giải quyết mà bamboo-core chưa giải.
- (b) Fork bamboo-core + patch Electron — quá tốn công upstream, không sustain.
- (c) Không làm gì — bỏ lỡ cơ hội cải thiện UX setup/diagnosis.

**Consequences**:
- (+) Scope rõ ràng, 4–5 tháng cho MVP, có thể do 1 người part-time.
- (+) Flywheel giữa 3 tool + giữa tool với upstream.
- (−) Phụ thuộc vào health của upstream; nếu bamboo-core die → một phần project vô dụng.
- (−) Người hiểu nhầm sẽ tưởng đây là "bộ gõ mới" — cần messaging rõ ràng.

---

## D-2026-04-24-02 — Chọn Rust làm ngôn ngữ chính

**Status**: accepted
**Context**: Cần single static binary, cross-compile Linux × archs dễ, error
handling nghiêm, không runtime deps.

**Decision**: Rust edition 2021, MSRV 1.75. Cargo workspace với 4 crate.

**Alternatives considered**:
- **Go**: binary nhỏ tương đương, nhưng error handling yếu và Wayland tooling
  Rust tốt hơn (zbus, smithay).
- **Python**: loại ngay vì env Python trên Linux là mìn với user cuối.
- **Node/TS**: cần runtime → loại.
- **C/C++**: phức tạp build, không có cargo-deny equivalent.

**Consequences**:
- (+) Binary < 8 MB mục tiêu khả thi.
- (+) `cargo-deny` / `cargo-audit` ecosystem mạnh.
- (−) Learning curve với contributor mới. Cần `CONTRIBUTING.md` có "first PR in 30min".
- (−) Async trong `zbus` = phải học tokio. Đổi lại có stream API.

---

## D-2026-04-24-03 — License GPL-3.0-or-later

**Status**: accepted
**Context**: Chúng ta tương tác với IBus (LGPL-2.1) và Fcitx5 (LGPL-2.1 /
GPL-2.0). Cần license tương thích cho phép upstream adopt code nếu maintainer
muốn.

**Decision**: GPL-3.0-or-later cho toàn workspace. Header SPDX tại đầu mọi file
`.rs`. `deny.toml` allow-list với GPLv3-compat licenses.

**Alternatives considered**:
- **Apache-2.0**: permissive nhưng không force upstream share lại.
- **MIT**: quá permissive cho 1 tool chạm `/etc/` và sudo.
- **AGPL-3.0**: overkill, chỉ cần khi có service network.

**Consequences**:
- (+) Upstream IBus/Fcitx5-Bamboo có thể cherry-pick code.
- (+) Ngăn commercial fork privatize.
- (−) Một số distro enterprise khó adopt GPLv3 binary (chấp nhận được — chúng ta
  phục vụ end-user, không enterprise).

---

## D-2026-04-24-04 — Cargo monorepo 4 crate thay vì split repo

**Status**: accepted
**Context**: 3 tool share code detect distro / env / IM framework.

**Decision**: Monorepo `vietime/`, 4 crate trong `crates/`:
- `vietime-core` — shared types, zero optional deps.
- `vietime-doctor` — Phase 1.
- `vietime-installer` — Phase 2.
- `vietime-bench` — Phase 3.

**Alternatives considered**:
- Split repo per tool — tốn công sync version + duplicate CI.
- Single crate 3 binary — không rõ ranh giới, khó package Flatpak riêng.

**Consequences**:
- (+) Refactor `vietime-core` 1 lần → cả 3 tool nhận update.
- (+) CI chạy 1 workflow cover cả 3.
- (−) Contributor phải hiểu workspace. Giảm bớt bằng docs/dev/architecture.md (TODO).

---

## D-2026-04-24-05 — Phase order: Doctor → Installer → Bench

**Status**: accepted
**Context**: 3 tool có phụ thuộc. Doctor có giá trị đứng một mình, Installer cần
Doctor để verify, Bench cần Doctor để ghi môi trường test.

**Decision**:
1. **Phase 1** = Doctor (đứng một mình, giá trị cao, độ phức tạp trung bình).
2. **Phase 2** = Installer (tái sử dụng `vietime-core` + Doctor verify).
3. **Phase 3** = Bench (tái sử dụng cả 2).

Mỗi phase release độc lập (SemVer per crate) trước khi phase kế tiếp bắt đầu.

**Alternatives considered**:
- Installer trước: tempting vì giải pain point rõ ràng nhất, nhưng Installer mà
  không có verify thì không an toàn → phải code Doctor partial trước để verify.
- Song song 3 phase: solo part-time không thể đồng thời cover.

**Consequences**:
- (+) Core stabilize dần, feedback từ Doctor sẽ shape `vietime-core` trước khi
  Installer bắt đầu ghi vào `/etc/`.
- (+) User thấy tiến triển liên tục, không phải chờ 12 tháng.
- (−) Installer phải chờ 2–3 tháng, có thể competitor xuất hiện. Rủi ro chấp
  nhận được (không ai khác đang làm hướng này).

---

<!-- Các assumption decision (D-A1..A5) và go/no-go (D-GO-1) sẽ thêm trong Phase 0. -->
