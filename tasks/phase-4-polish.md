# Phase 4 — Polish & Upstream (Tháng 9–12, 16 tuần)

> **Goal**: chuyển từ "tool của mình" sang "tool cộng đồng dùng + upstream contribution".
>
> **Exit criteria**: đạt metric 12 tháng (spec/00 §9): ≥ 1000 stars, ≥ 10k DL, ≥ 10 contributor, 100+ bug report có Doctor attachment, 500+ matrix rows, 30 blog/forum mention.
>
> **Budget**: 80–120h.

---

## Track A — User feedback loop

### POL-01 — Bug triage SLA
- **Status**: TODO
- **Priority**: P0
- **Estimate**: Ongoing
- **Depends on**: DOC-76, INS-74, BEN-93
- **Spec ref**: spec/04 §9 (review style)
- **Acceptance**:
  - [ ] Publish "respond within 72h" SLA trong CONTRIBUTING.md.
  - [ ] Auto-label issue bot (GitHub Actions) — `bug`, `compat`, `enhancement`.
  - [ ] Weekly triage: mark priority, ask for Doctor report nếu thiếu.

### POL-02 — FAQ + troubleshooting docs
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (8h)
- **Depends on**: POL-01
- **Spec ref**: spec/04 §6.1
- **Acceptance**:
  - [ ] `docs/vi/troubleshooting.md` tổng hợp 20 câu hỏi thường gặp từ feedback Phase 1–3.
  - [ ] Mirror en.
  - [ ] Link từ Doctor/Installer help output.

### POL-03 — User survey round 2
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (3h)
- **Depends on**: POL-01
- **Spec ref**: spec/05 §3 (decision gate)
- **Acceptance**:
  - [ ] Google Form + FB post: "Bạn dùng VietIME 3 tháng qua thế nào?"
  - [ ] ≥ 30 response.
  - [ ] Synthesize vào `docs/research/survey-round2.md`.

---

## Track B — Bug fixes from Bench matrix

### POL-10 — Pick top 3 bugs từ matrix
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: BEN-93
- **Spec ref**: spec/05 §2 (Phase 4)
- **Acceptance**:
  - [ ] Từ dashboard, chọn 3 bug có impact nhất (nhiều app × nhiều combo).
  - [ ] Document root cause analysis trong `docs/dev/bug-analysis/<id>.md`.
  - [ ] Coordinate với maintainer upstream trước khi viết patch.

### POL-11 — Upstream PR #1 (ibus-bamboo)
- **Status**: TODO
- **Priority**: P0
- **Estimate**: XL (20h)
- **Depends on**: POL-10
- **Spec ref**: spec/05 §2 (Phase 4)
- **Acceptance**:
  - [ ] Fork `BambooEngine/ibus-bamboo`.
  - [ ] Submit PR với test case (reuse Bench vector).
  - [ ] Address review feedback.
  - [ ] Merge HOẶC (nếu maintainer từ chối) document post-mortem.

### POL-12 — Upstream PR #2 (fcitx5-bamboo)
- **Status**: TODO
- **Priority**: P0
- **Estimate**: XL (20h)
- **Depends on**: POL-10
- **Spec ref**: spec/05 §2 (Phase 4)
- **Acceptance**:
  - [ ] Fork + PR.
  - [ ] Test case kèm.
  - [ ] Merge hoặc document.

### POL-13 — Upstream PR #3 (TBD)
- **Status**: TODO
- **Priority**: P1
- **Estimate**: L (12h)
- **Depends on**: POL-10
- **Spec ref**: spec/05 §2 (Phase 4)
- **Acceptance**:
  - [ ] Có thể là ibus-unikey / fcitx5-unikey / BambooEngine core.
  - [ ] Hoặc là Electron workaround nếu tìm được.

---

## Track C — Distro packaging

### POL-20 — Submit `fcitx5-bamboo` vào Debian official
- **Status**: TODO
- **Priority**: P1
- **Estimate**: XL (16h)
- **Depends on**: POL-11 hoặc POL-12
- **Spec ref**: spec/05 §2 (Phase 4)
- **Acceptance**:
  - [ ] Liên hệ Debian Input Method Team (dim).
  - [ ] Nếu fcitx5-bamboo chưa vào Debian: tạo ITP (Intent to Package) bug.
  - [ ] Package theo Debian guidelines.
  - [ ] Sponsor upload.
  - [ ] **Hoặc** chứng minh đã trong Debian → no-op, DROP task.

### POL-21 — Ubuntu LTS PPA
- **Status**: TODO
- **Priority**: P2
- **Estimate**: M (6h)
- **Depends on**: INS-73
- **Spec ref**: spec/05 §4 (R19 mitigation)
- **Acceptance**:
  - [ ] Launchpad PPA setup.
  - [ ] Upload `.deb` của Doctor + Installer + fcitx5-bamboo mới nhất.
  - [ ] Doc cho user `add-apt-repository`.

### POL-22 — Flathub stabilization
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: DOC-72
- **Spec ref**: spec/04 §7
- **Acceptance**:
  - [ ] Doctor + Bench Flatpak đã approved trên Flathub.
  - [ ] Auto-update workflow khi tag release.
  - [ ] Monitor download stats.

### POL-23 — AUR maintainership
- **Status**: TODO
- **Priority**: P2
- **Estimate**: M (4h)
- **Depends on**: DOC-73, INS-72
- **Spec ref**: spec/04 §7
- **Acceptance**:
  - [ ] PKGBUILD cho cả 3 binary công cụ.
  - [ ] Co-maintainer recruit (nếu ai đó offer).

---

## Track D — Community building

### POL-30 — Recruit first 2 contributor
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (ongoing, ~10h spread)
- **Depends on**: POL-01
- **Spec ref**: spec/05 §4 (R15 mitigation)
- **Acceptance**:
  - [ ] ≥ 10 issue tagged `good-first-issue`.
  - [ ] Active mentoring cho first-time PR.
  - [ ] ≥ 2 external PR merged.

### POL-31 — Discord/Matrix community (optional)
- **Status**: TODO
- **Priority**: P3
- **Estimate**: S (3h)
- **Depends on**: POL-30
- **Spec ref**: spec/04 §15
- **Acceptance**:
  - [ ] **Nếu** GitHub Discussions quá tải → mới tạo.
  - [ ] Moderation policy rõ ràng.
  - [ ] Không tạo sớm (anti-pattern spec/04 §16).

### POL-32 — Regular blog cadence
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (~4h/quarter)
- **Depends on**: —
- **Spec ref**: spec/04 §15
- **Acceptance**:
  - [ ] ≥ 1 post/quarter.
  - [ ] Topics: release note, user story, deep dive kỹ thuật.

### POL-33 — Conference / meetup talk
- **Status**: TODO
- **Priority**: P3
- **Estimate**: L (submission + prep 10h)
- **Depends on**: POL-32
- **Spec ref**: —
- **Acceptance**:
  - [ ] Submit talk tới Vietnam Linux User Group meetup.
  - [ ] Hoặc FOSDEM Input Methods devroom (quốc tế).
  - [ ] Slides lưu trong `docs/talks/`.

---

## Track E — Quality + maintenance

### POL-40 — v0.2 planning
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (3h)
- **Depends on**: POL-03
- **Spec ref**: —
- **Acceptance**:
  - [ ] Review `backlog.md` với lens 6 tháng qua.
  - [ ] Prioritize top 10 cho v0.2.
  - [ ] Draft `spec/v0.2-roadmap.md`.

### POL-41 — Dependency upgrade pass
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (3h)
- **Depends on**: —
- **Spec ref**: spec/04 §12
- **Acceptance**:
  - [ ] `cargo update` + test.
  - [ ] `cargo audit` no critical.
  - [ ] `cargo deny` xanh.

### POL-42 — Performance regression check
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (2h)
- **Depends on**: —
- **Spec ref**: spec/04 §11
- **Acceptance**:
  - [ ] Hyperfine benchmark Doctor cold start.
  - [ ] So sánh với baseline v0.1.
  - [ ] Fail nếu regression > 20%.

### POL-43 — Security audit pass
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (5h)
- **Depends on**: —
- **Spec ref**: spec/04 §10
- **Acceptance**:
  - [ ] Review Installer privilege escalation paths.
  - [ ] Verify no shell injection.
  - [ ] Document trong `SECURITY.md`.

---

## Track F — Checkpoint & decide

### POL-50 — 12-month metrics review
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: (end of Phase 4)
- **Spec ref**: spec/00 §9, spec/05 §9
- **Acceptance**:
  - [ ] Đo metrics spec/00 §9 vs target 12-month.
  - [ ] So sánh với kill criteria spec/05 §9.
  - [ ] Quyết định: continue v0.2 / pause / archive.
  - [ ] Ghi vào `docs/dev/decisions.md` (D-Y1).
  - [ ] Viết year-1 retrospective blog.

### POL-51 — Year-2 strategy (nếu continue)
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (5h)
- **Depends on**: POL-50
- **Spec ref**: spec/05 §7 (escape hatches)
- **Acceptance**:
  - [ ] Đánh giá escape hatches: merge upstream / spin off / continue.
  - [ ] Update `spec/` với v0.2 vision.
  - [ ] Recruit co-maintainer nếu scope mở rộng.

---

## Phase 4 — Exit checklist

**Tháng 12 kết quả kỳ vọng** (spec/00 §9):

- [ ] ≥ 1000 GitHub stars.
- [ ] ≥ 10k downloads (Flatpak + deb + rpm + AUR).
- [ ] ≥ 10 contributor non-author.
- [ ] ≥ 100 bug report có Doctor attachment.
- [ ] ≥ 5 distro covered.
- [ ] ≥ 500 matrix rows.
- [ ] ≥ 30 blog/forum mention.
- [ ] ≥ 2 upstream PR merged (POL-11/12).
- [ ] Year-1 retrospective published.

Nếu < 3/9 mục đạt → kill criteria triggered, archive dự án.
