# VietIME Suite — Task Board

Task list kế hoạch, chia theo phase của `spec/05-roadmap-risks.md`. Tất cả task được format để track tiến độ và assign estimate/priority.

---

## Cấu trúc file

| File | Scope | Timebox |
|---|---|---|
| [`phase-0-validate.md`](phase-0-validate.md) | Validate + setup foundation | Tháng 1 (4 tuần) |
| [`phase-1-doctor.md`](phase-1-doctor.md) | VietIME Doctor v0.1 | Tháng 2–3 (8 tuần) |
| [`phase-2-installer.md`](phase-2-installer.md) | VietIME Installer v0.1 | Tháng 4–5 (8 tuần) |
| [`phase-3-bench.md`](phase-3-bench.md) | VietIME Bench v0.1 + dashboard | Tháng 6–8 (10 tuần) |
| [`phase-4-polish.md`](phase-4-polish.md) | Polish, upstream contribution | Tháng 9–12 (16 tuần) |
| [`backlog.md`](backlog.md) | v0.2+ features, nice-to-have | — |

---

## Format mỗi task

```
### TASK-ID — Title
- **Status**: TODO | IN-PROGRESS | DONE | BLOCKED | DROPPED
- **Priority**: P0 (blocker) | P1 (must) | P2 (should) | P3 (nice)
- **Estimate**: Xh hoặc Xd
- **Depends on**: TASK-ID, TASK-ID
- **Spec ref**: spec/XX-name.md §Y.Z
- **Owner**: (solo MVP = author)
- **Acceptance**:
  - [ ] Criterion 1
  - [ ] Criterion 2
```

**Task ID scheme**:
- `P0-##` Phase 0
- `DOC-##` Phase 1 (Doctor)
- `INS-##` Phase 2 (Installer)
- `BEN-##` Phase 3 (Bench)
- `POL-##` Phase 4 (Polish/upstream)
- `BL-##` Backlog

IDs **không tái dùng** khi task bị drop.

---

## Status workflow

```
TODO ──→ IN-PROGRESS ──→ DONE
  │           │
  │           └──→ BLOCKED ──→ IN-PROGRESS (unblock)
  │
  └──→ DROPPED (với lý do, gạch chéo task)
```

**Rule**: chỉ 1 task IN-PROGRESS cùng lúc per track (có thể có nhiều track song song, mỗi phase 1 track chính).

---

## Weekly cadence

Chủ nhật tối:
1. Đánh dấu DONE các task hoàn thành tuần qua.
2. Review BLOCKED — unblock được chưa?
3. Chọn 3–5 task cho tuần tới, đặt IN-PROGRESS (queue).
4. Ghi vào `docs/dev/weekly-log.md` (mỗi tuần ≤ 10 dòng).

---

## Priority guide

- **P0** — blocker. Không làm thì phase không kết thúc được. Max 3 P0 cùng lúc.
- **P1** — must. Là phần cốt lõi của acceptance criteria của phase.
- **P2** — should. Cải thiện chất lượng nhưng có thể defer.
- **P3** — nice. Move sang `backlog.md` nếu không kịp.

**Cắt scope rule**: nếu timebox cuối phase còn 1 tuần mà > 3 P1 chưa xong → promote 2 P1 thành P0, demote số còn lại sang phase sau/backlog. Không kéo dài timebox.

---

## Estimate calibration

Estimate thô:
- **XS** = < 2h — quickie fix hoặc script ngắn.
- **S** = 2–4h — 1 buổi tối.
- **M** = 0.5–1d — 1 ngày cuối tuần.
- **L** = 1–2d — 2–3 buổi tối + 1 cuối tuần.
- **XL** = 3–5d — >1 tuần part-time.
- **XXL** = > 1 tuần — **cấm**. Break nhỏ lại.

Part-time baseline: 10–15h/tuần. Một phase 8 tuần ≈ 80–120h effort.

---

## Metrics to track (tự monitor)

Mỗi tháng check:
- Tasks DONE / Tasks pending.
- Tasks trong DROPPED / total (> 20% = scope đoán sai).
- BLOCKED age trung bình (> 7 ngày = alert).
- Estimate accuracy (actual h / estimate h, target 0.8–1.3).
