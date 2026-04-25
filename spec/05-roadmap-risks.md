# Roadmap & Risk Register

> Lịch triển khai tổng thể + danh sách rủi ro + chiến lược đối phó. Tham chiếu timebox cụ thể trong spec từng phase.

---

## 1. Overall timeline

```
Tháng 1:      Phase 0 (Validate + setup)      ──┐
Tháng 2–3:    Phase 1 (Doctor)                  ├── Q1 deliverable: Doctor v0.1 + blog
Tháng 4–5:    Phase 2 (Installer)              ──┤   Q2 deliverable: Installer v0.1 + deb/rpm
Tháng 6–8:    Phase 3 (Bench)                  ──┤   Q3 deliverable: Bench v0.1 + dashboard
Tháng 9–10:   Polish, bug fix, docs, community ──┤
Tháng 11–12:  Upstream contribution            ──┘   Q4 deliverable: first upstream PRs merged
```

Commit: **10–15h/tuần part-time**.

**Hard rule**: cuối mỗi tháng phải có release hoặc PR public. Không cho phép "âm thầm code 3 tháng rồi công bố". Release nhỏ là cơ chế chống motivation drain.

---

## 2. Phase-by-phase milestones & exit criteria

### Phase 0 (Tháng 1)

Mục tiêu: **không code production, chỉ validate assumptions và setup foundation**.

- [ ] W1: Build `ibus-bamboo` + `fcitx5-bamboo` local; ghi lại ≥ 20 pain points cụ thể.
- [ ] W1: Post user research trên 2 nhóm Facebook + 1 subreddit → 15 feedback.
- [ ] W2: Gửi email/DM maintainer `ibus-bamboo` và `fcitx5-bamboo`, giới thiệu dự án, hỏi pain point ưu tiên.
- [ ] W2: Đánh giá A1–A5 (assumptions) trong `00-vision-and-scope.md` §10. Cập nhật spec nếu assumption sai.
- [ ] W3: Setup repo, Cargo workspace, CI (`ci.yml`), pre-commit, `cargo-deny` config.
- [ ] W3: Code `vietime-core` skeleton: `Distro`, `SessionType`, `ImFramework`, `Facts`.
- [ ] W4: Viết 1 blog post "Tại sao gõ tiếng Việt trên Ubuntu khó" → publish.
- [ ] W4: Go/no-go checkpoint. Nếu feedback ≤ 5 hoặc maintainer rõ ràng không quan tâm → rethink.

**Exit criteria**: `vietime-core` compile, test pass, 1 blog post live, go/no-go quyết định.

### Phase 1 (Tháng 2–3, 8 tuần)

Xem `01-phase1-doctor.md` §B.12.

**Exit criteria**: Doctor v0.1.0 released trên Flatpak + GitHub + blog post.

### Phase 2 (Tháng 4–5, 8 tuần)

Xem `02-phase2-installer.md` §B.14.

**Exit criteria**: Installer v0.1.0 released `.deb`/`.rpm`, tested 5 distro VM.

### Phase 3 (Tháng 6–8, 10 tuần)

Xem `03-phase3-test-suite.md` §B.14.

**Exit criteria**: Bench v0.1.0 + public dashboard live, nightly CI xanh 14 ngày liên tiếp.

### Phase 4 — Polish & Upstream (Tháng 9–12)

Mục tiêu: chuyển từ "tool của mình" sang "tool cộng đồng dùng + đóng góp upstream":

- [ ] Viết docs user-facing đầy đủ (vi + en).
- [ ] Tổ chức issue triage process; publish response SLA (72h).
- [ ] Bug fix dựa trên feedback 3 phase.
- [ ] Dựa trên Bench matrix, pick 3 bug đau nhất, viết fix + PR vào `ibus-bamboo` / `fcitx5-bamboo`.
- [ ] Đưa `fcitx5-bamboo` (nếu thiếu) vào Debian/Ubuntu official repo (coordinate với Debian Input Method team).
- [ ] Mentor 1–2 contributor mới.

**Exit criteria** (năm đầu): đạt metric ở `00` §9 cho mốc 12 tháng.

---

## 3. Decision gates

Tại **cuối mỗi phase**, trả lời 3 câu hỏi:

1. **Value**: có user thật dùng không? (DL count, issue count, Doctor report được dán)
2. **Health**: maintainer còn motivation? (commit/tuần ≥ 2, không mở stale issue)
3. **Direction**: phase tiếp theo còn hợp lý? (có khi insight từ phase trước đổi hướng)

Nếu **2/3 không** → pause 1 tuần, reassess. Có thể đổi thứ tự phase, scope nhỏ lại, hoặc stop.

---

## 4. Risk register (consolidated)

Ký hiệu: **Likelihood (L)**: 1–5, **Impact (I)**: 1–5, **Score** = L × I. Priority > 12 → có mitigation plan cụ thể.

| # | Risk | L | I | Score | Mitigation | Owner |
|---|---|---|---|---|---|---|
| R1 | Scope creep → viết IME engine | 4 | 5 | **20** | Non-goals trong `00`, ask trước khi bắt đầu: "cái này có đẩy ta sang non-goal không?"; weekly self-review | author |
| R2 | Motivation drain (system code chậm có dopamine) | 4 | 5 | **20** | Release hàng tháng, post public → feedback; timebox phase; không gộp release |
| R3 | Xung đột với maintainer upstream | 2 | 5 | **10** | Phase 0 gửi msg sớm; framing "tool bổ trợ" không "thay thế"; luôn link upstream trong docs |
| R4 | Wayland IME ecosystem thay đổi (GTK4/Qt6 mới) giữa phase | 3 | 4 | **12** | Follow mailing list Wayland/text-input-v3; buffer 2 tuần mỗi phase cho adapt |
| R5 | `/etc/environment` edit làm user mất tiếng Anh | 3 | 5 | **15** | Dry-run mandatory, snapshot mandatory, section marker, pre-flight check; docs nói rõ rollback |
| R6 | Electron/Chromium upstream đổi Ozone IME flag | 3 | 3 | 9 | Version-pin trong Bench; Doctor có warning nếu version mới |
| R7 | ydotool yêu cầu `/dev/uinput` mà Docker default không có | 3 | 3 | 9 | Docs config explicit; fallback `wtype`; x11-only matrix cho CI tạm thời |
| R8 | Flatpak Flathub review lâu/reject | 3 | 2 | 6 | Bắt đầu submit sớm (tuần 6 Phase 1); tarball release luôn có sẵn |
| R9 | User Linux Việt không hiểu Flatpak/deb, expect GUI Windows-style | 3 | 3 | 9 | Installer có TUI đơn giản; docs video YouTube; FAQ tiếng Việt cụ thể |
| R10 | Tôi bị burnout sau 6 tháng solo | 3 | 5 | **15** | Part-time cứng 10–15h/tuần, không vượt; weekly recap; 1 week off mỗi quarter |
| R11 | Bug test vector sai Unicode (NFC vs NFD) gây false failure | 4 | 3 | **12** | Validator tool `vietime-bench validate` check NFC; CI enforce |
| R12 | Package `fcitx5-bamboo` không có trong Fedora/Arch chính thức | 4 | 3 | **12** | Planner detect, gợi ý COPR/AUR; có fallback build-from-source (v0.2) |
| R13 | D-Bus interface IBus/Fcitx5 break giữa distro versions | 2 | 4 | 8 | Feature-detect; wrap gracefully; test matrix |
| R14 | Snap package (VS Code snap) không kết nối được IM | 4 | 2 | 8 | Doctor warn, Installer gợi ý .deb thay thế |
| R15 | Không thu hút được contributor → stall | 3 | 4 | **12** | Good-first-issue tagging; phản hồi issue nhanh; làm docs contribute tiếng Việt |
| R16 | Rò rỉ PII trong Doctor report | 2 | 5 | **10** | Redaction tự động, `--no-redact` explicit; fuzz test redactor |
| R17 | Chi phí hosting dashboard nếu traction cao | 1 | 2 | 2 | GitHub Pages miễn phí; Cloudflare free tier |
| R18 | Dependency bị yank (cargo-deny fail) | 3 | 2 | 6 | `cargo audit` weekly; lock file commit |
| R19 | User Ubuntu LTS stale dùng bản cũ Bamboo → Installer cài xong vẫn buggy | 4 | 3 | **12** | Doctor warn version outdated; Installer gợi ý PPA BambooEngine |
| R20 | Test vectors copyright/đạo văn | 2 | 3 | 6 | Viết vectors từ đầu, CC0/MIT; cite nguồn khi dùng từ điển |

---

## 5. Decision log — mọi quyết định lớn ghi ở đây

Mỗi decision một entry, format ngắn:

```
## YYYY-MM-DD: <tiêu đề quyết định>
Context: 2–3 câu bối cảnh.
Options considered: bullet.
Decision: dòng rõ ràng.
Consequences: gì sẽ đổi.
```

File: `docs/dev/decisions.md`. Bắt đầu rỗng, thêm dần.

Decisions to seed khi Phase 0:
- D1: chọn Rust vs Go — Rust.
- D2: chọn monorepo vs multirepo — monorepo.
- D3: chọn GPLv3 vs MIT — GPLv3.
- D4: hỗ trợ Windows/macOS — không ở MVP.
- D5: telemetry MVP — không.

---

## 6. Budget check — cost of each phase (time + money)

| Phase | Time (h) | Infra cost | Learning curve |
|---|---|---|---|
| 0 | 40–60 | $0 | Low |
| 1 | 80–120 | $0 | Medium (D-Bus, distro conventions) |
| 2 | 80–120 | $0 | High (atomic rollback, privilege) |
| 3 | 120–160 | $0 (GH Actions free tier) | High (headless X11/Wayland) |
| 4 | 80–120 | $0–$50 (domain) | Low (mostly docs + upstream) |
| **Total** | **400–580** | **<$100** | |

Nếu **thời gian available < 300h/năm**: downscope. Options:
- Drop Phase 3 (Bench), giữ Doctor + Installer.
- Drop Installer auto-mode, giữ TUI guide.
- Giữ X11 only, drop Wayland ở Phase 3.

---

## 7. Escape hatches — khi nào stop và merge về upstream

Nếu tại bất kỳ thời điểm nào, một trong các tình huống sau xảy ra, nên **dừng phát triển riêng và merge tool vào dự án upstream**:

- Maintainer `ibus-bamboo` hoặc `fcitx5-bamboo` chủ động mời co-maintain + merge Doctor vào chính dự án họ.
- Cộng đồng Fcitx5 upstream (quốc tế) adopt Doctor-like tool — đóng góp vào đó thay vì duy trì riêng.
- Canonical hoặc GNOME dev release official "Linux IME setup tool" cho mọi ngôn ngữ — Doctor của mình tích hợp/plugin vào đó.

Đây **không phải thất bại**. Đây là success — giá trị được internalize vào ecosystem lớn hơn.

---

## 8. Sanity checks hàng tuần

Mỗi Chủ nhật tối, tự hỏi 5 câu:

1. Tuần này mình có commit được ít nhất 2 lần không?
2. Có issue nào tồn đọng > 14 ngày chưa trả lời không?
3. Phase hiện tại có trễ timebox không? Nếu có > 1 tuần → trigger rescope.
4. Mình có đang sa đà vào feature không thuộc MVP không?
5. Tuần sau ưu tiên 3 việc cụ thể gì?

Ghi vào `docs/dev/weekly-log.md`. 10 dòng/tuần là đủ.

---

## 9. Kill criteria — khi nào tuyên bố dự án thất bại

Nếu sau **12 tháng**:
- Downloads < 500,
- Contributor non-author = 0,
- Không feedback có ý nghĩa từ maintainer upstream,
- Personal motivation < "sẵn sàng làm tiếp",

→ Tuyên bố dự án **thành công ở mức học hỏi, dừng công khai**. Viết post-mortem. Archive repo. Không giả vờ maintain.

Đây là điều khoản lành mạnh, không tiêu cực. Solo OSS stall sau 12 tháng là mặc định; nói rõ tiêu chí giúp thoát khỏi sunk cost.
