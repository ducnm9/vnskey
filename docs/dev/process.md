# Quy trình phát triển — VietIME Suite

> **Audience**: maintainer (solo, part-time) và contributor tương lai.
> **Nguyên tắc chủ đạo**: "Solo + Automation-as-Team". Tự động hoá đảm nhiệm vai trò các vị trí bị thiếu.
> **Trạng thái**: proposal v1. Review lại cuối Phase 0 (P0-30 checkpoint).

---

## 0. Tension căn bản

Yêu cầu người dùng đặt ra 4 mục tiêu **không thể cùng tối đa hoá**:

| Mục tiêu | Nghĩa cụ thể | Xung đột với |
|---|---|---|
| **Solo part-time** | 10–15h/tuần, 1 người | "đủ vị trí", "chất lượng nhất" |
| **Đủ vị trí** | PM, Dev, QA, DevOps, Security, Writer, Designer | "solo", "hoàn thiện nhanh" |
| **Chất lượng nhất** | ≥ 80% coverage, zero bug production, full audit | "hoàn thiện nhanh" |
| **Hoàn thiện nhanh** | MVP v0.1 trong 4–5 tháng | "đủ vị trí", "chất lượng nhất" |

**Lựa chọn chiến lược**: ưu tiên **solo + hoàn thiện nhanh** làm hard constraint; dùng **automation + process rigor** để mô phỏng "đủ vị trí" và đảm bảo **chất lượng đủ dùng** (không phải "nhất"). "Chất lượng nhất" là mục tiêu v1.0, không phải v0.1.

Từ chối rõ ràng:
- ❌ Không thuê/tuyển QA/designer full-time cho MVP.
- ❌ Không chờ 100% coverage hoặc formal verification.
- ❌ Không chạy security audit bên thứ ba cho v0.1.
- ✅ Thay vào đó: **CI pipeline nghiêm + scope cut rule cứng + timebox mỗi phase**.

---

## 1. Mapping vai trò truyền thống → solo + automation

Trong team chuẩn enterprise có ~10 vai trò. Solo part-time phải compress xuống 1 người + tooling.

| Vai trò chuẩn | Trách nhiệm | Cách cover trong VietIME Suite |
|---|---|---|
| **Product Manager** | Roadmap, prioritize, kill features | Maintainer + `spec/05-roadmap-risks.md` + checkpoint P0-30/POL-50 |
| **Tech Lead / Architect** | Spec, design decisions | Maintainer + `spec/*.md` + ADR tại `docs/dev/decisions.md` |
| **Developer** | Code | Maintainer |
| **Code Reviewer** | Review PR, catch bug early | **CI bot** (clippy, fmt, deny, audit, test) + 48h-rule: chính maintainer tự review PR của mình sau 48h (self-review as a stranger) |
| **QA Engineer** | Test plan, manual test, regression | **Test suite tự động** (unit + integration + Bench vector) + smoke profile ≤ 15 phút |
| **DevOps / SRE** | CI/CD, release pipeline | **GitHub Actions** + release-plz (BL-72) + reproducible build |
| **Security Engineer** | Threat model, audit | `SECURITY.md` + `cargo-audit` + `cargo-deny` trong CI + quarterly review (POL-43) |
| **Tech Writer** | User docs, API docs | Maintainer + mdBook + rule "PR không có docs = không merge" |
| **Designer / UX** | UI/UX, brand | `spec/04-cross-cutting.md` §2 brand rule + CLI output convention |
| **Release Manager** | Version bump, CHANGELOG, tag | **release-plz** (BL-72) automate hoàn toàn |
| **Community Manager** | Triage issue, mentor contributor | POL-01 (72h SLA) + POL-30 (good-first-issue) |

Nguyên tắc: **không có vị trí nào bị bỏ trống. Hoặc maintainer đảm nhiệm, hoặc tooling đảm nhiệm.** Bất kỳ vai trò nào không có owner rõ ràng là bug process.

---

## 2. Quality gates — non-negotiable vs conditional

Chia tất cả kiểm tra chất lượng thành 3 tier theo chi phí / giá trị.

### Tier 1 — Blocking gate (mọi PR phải pass)

Chạy trong < 5 phút. Fail = không merge.

| Gate | Tool | Chi phí | Lý do |
|---|---|---|---|
| Format | `cargo fmt --check` | < 5s | Zero debate về style |
| Lint | `cargo clippy -- -D warnings` | 30s | Catch bug common |
| Build | `cargo build --all-targets` | 1–2m | Obvious |
| Unit test | `cargo test` | 1–3m | Regression tối thiểu |
| License/deps | `cargo deny check` | 10s | Tránh GPL incompat vô tình |
| Security advisory | `cargo audit` | 20s | Tránh CVE đã biết |
| Typo | `typos` | 5s | Docs/code spelling |

**Không có exception**. Nếu fail do flaky → fix flaky, không retry.

### Tier 2 — Merge gate (chạy trước mỗi release, không phải mỗi PR)

Chạy ≤ 30 phút. Fail = delay release, không block work-in-progress.

| Gate | Tool | Chu kỳ | Spec ref |
|---|---|---|---|
| Snapshot test | `insta` | Mỗi PR chạm renderer | spec/01 §5.5 |
| Docker integration (Installer) | GH Actions matrix 5 distro | Mỗi release | spec/02 §B.12 |
| Bench smoke profile | `vietime-bench --profile smoke` | Nightly | spec/03 §B.11 |
| Coverage report | `cargo-llvm-cov` | Mỗi release | — |
| Binary size check | `cargo bloat` | Mỗi release | spec/04 §11 |
| Cold-start benchmark | `hyperfine` | Mỗi release | POL-42 |

### Tier 3 — Deferrable (v0.2+ hoặc khi có resource)

Không làm cho MVP. Ghi vào backlog.

- Fuzzing (`cargo-fuzz`) — cho parser env file.
- Mutation testing (`cargo-mutants`).
- Formal threat model (STRIDE).
- Third-party security audit.
- End-to-end test trên bare-metal hardware.
- Load testing (Bench với 10k vector).

**Rule**: không bị cám dỗ làm Tier 3 sớm. Mỗi lần thêm gate phải nói rõ "remove what?" để giữ budget.

---

## 3. Daily / weekly workflow

### 3.1 Daily (mỗi session làm việc, 1–3h)

```
1. Mở task board (`tasks/phase-X.md`), chọn 1 task TODO với priority cao nhất.
2. Mark IN-PROGRESS. Ghi start time vào daily log.
3. Branch: `git checkout -b <phase>/<task-id>-<short-desc>`
   Ví dụ: `doc/DOC-12-env-file-parser`.
4. Code → test local → commit theo Conventional Commits.
5. Push + mở draft PR ngay từ commit đầu tiên (để CI chạy liên tục).
6. Khi task xong:
   - Mark acceptance criteria trong task file (check box).
   - Mark DONE.
   - Undraft PR, self-review checklist (§3.3).
   - Merge sau khi Tier 1 gate xanh.
7. Kết session: ghi 3 bullet vào `docs/dev/log/YYYY-MM-DD.md`:
   - Done: ...
   - Blocked: ...
   - Next: ...
```

**Hard rule**: chỉ 1 task IN-PROGRESS tại 1 thời điểm. Không WIP song song.

### 3.2 Weekly (Chủ nhật, 30–60 phút)

Ritual bắt buộc. Không skip.

```
1. Review `docs/dev/log/` của tuần — đã done bao nhiêu task?
2. Burndown check: phase đang chạy còn n task vs còn m tuần. Nếu n/m > capacity
   (thường 3–4 task/tuần) → TRIGGER scope cut:
   - Demote task P1+ sang backlog.
   - Hoặc giảm acceptance criteria.
3. Risk register (`spec/05-roadmap-risks.md`): 1 risk nào có score đổi tuần này?
   Ghi note + mitigation nếu cần.
4. Update CHANGELOG.md (dòng `## [Unreleased]`) với feature / fix đã merge.
5. Kiểm tra `cargo update` + `cargo audit` manual (CI nightly cover, nhưng check
   trước khi Monday).
6. Viết 1 post ngắn (Twitter/Mastodon/nhật ký private) — "tuần này làm gì". Tạo
   accountability + build-in-public.
```

### 3.3 PR self-review checklist (stranger test)

Khi mở PR và sau 24–48h, đọc lại PR **như thể ai đó khác viết**. Check:

- [ ] Title theo Conventional Commits (`feat(doctor): detect ibus-daemon`).
- [ ] Mô tả PR trả lời: **What changed? Why? How tested?**
- [ ] Mỗi acceptance criterion trong task tương ứng ✓.
- [ ] Có ít nhất 1 test mới (nếu là code). Nếu không → ghi lý do.
- [ ] Error message mới có actionable hint (không chỉ "failed").
- [ ] Nếu chạm public API → docs có cập nhật?
- [ ] Nếu chạm config file format → spec version bump?
- [ ] Có commit nào > 400 LOC? Nếu có → split.

Nếu fail ≥ 2 item → không merge, chỉnh trước.

---

## 4. Release cadence

### 4.1 Versioning per component

Mỗi crate semver độc lập (spec/04 §7). Tag format: `vietime-doctor-v0.1.0`.

### 4.2 Release rhythm

| Release type | Tần suất | Trigger |
|---|---|---|
| **Dev snapshot** | Mỗi commit | Main branch, không tag |
| **Alpha/Beta** | Monthly trong phase | `v0.1.0-alpha.N` |
| **Minor (v0.x.0)** | Cuối mỗi phase | Exit checklist pass |
| **Patch (v0.x.y)** | Ad-hoc | Bug P0 từ user |

### 4.3 Release steps (automated qua release-plz)

1. `release-plz` PR tự động bump version + generate CHANGELOG từ commits.
2. Maintainer review PR, merge.
3. CI chạy Tier 2 gate.
4. Nếu xanh → auto tag + GH Release + upload binary + Flatpak submit.
5. Social post (FB/Reddit/mailing list).

Manual release không được phép (trừ hotfix emergency).

---

## 5. Fast-path prioritization

"Hoàn thiện nhanh" cụ thể hoá thành 5 quy tắc cứng.

### R1 — Timebox cứng mỗi phase

| Phase | Budget | Exit |
|---|---|---|
| 0 — Validate | 4 tuần | P0-30 go/no-go |
| 1 — Doctor | 8 tuần | spec/01 exit checklist |
| 2 — Installer | 8 tuần | spec/02 §B.16 |
| 3 — Bench | 10 tuần | spec/03 §B.16 |
| 4 — Polish | 16 tuần | spec/00 §9 metrics |

Vượt budget > 20% → **trigger scope cut**, không extend.

### R2 — Scope cut rule

Khi trigger:
1. List task còn lại. Phân P0 (ship blocker) vs P1+ (nice-to-have).
2. Demote tất cả P2/P3 còn lại sang backlog.
3. Nếu P0 vẫn overflow → cắt từng acceptance criterion (vd: chỉ X11, bỏ Wayland sang v0.2).
4. Không được thêm P0 mới ở giai đoạn này.

### R3 — Ship over polish

Rule of 3:
- Bug gặp bởi ≥ 3 user → P0 fix.
- Bug gặp < 3 user trong 2 tuần → ghi issue, defer v0.2.

### R4 — 80/20 trên config/distro

MVP chỉ support 80% user (Ubuntu LTS, Debian 12, Fedora 39+, Arch, Pop!_OS). Các distro còn lại: chấp nhận không work, document rõ.

### R5 — Không tự viết khi upstream cover

Trước mỗi task, hỏi: "Có crate/tool/service nào đã làm 80% việc này?" Nếu có → wrap/adapt, không reimplement. Ví dụ: dùng `chromiumoxide` thay tự viết CDP client.

---

## 6. Contributor on-ramp (khi có người thứ 2)

Rào cản vào thấp nhất có thể, bù lại review rigorous.

### 6.1 Onboarding docs

- `CONTRIBUTING.md` với "First PR in 30 minutes" guide.
- `docs/dev/architecture.md` — 1-page tour của workspace.
- `good-first-issue` label với ≥ 10 issue live (POL-30).
- `docs/dev/process.md` (file này) — bắt buộc đọc.

### 6.2 PR template

```markdown
## Task
Closes #NNN (hoặc DOC-##/INS-##/...).

## What & Why
<1-2 câu>

## How tested
- [ ] `cargo test` pass local
- [ ] Thêm test mới: <name>
- [ ] Manual: <steps>

## Checklist
- [ ] Conventional commit
- [ ] Docs updated (nếu cần)
- [ ] No new warnings
```

### 6.3 Review SLA

- Draft PR: không SLA.
- Ready-for-review PR: maintainer phản hồi trong 72h (POL-01).
- Nếu > 72h: contributor được tự bump + ping — không phải lỗi của contributor.

### 6.4 Trust levels

| Level | Quyền | Cách đạt |
|---|---|---|
| Drive-by | Open PR | Default |
| Regular | Triage issue | ≥ 2 PR merged |
| Co-maintainer | Merge quyền | POL-71 invite |

Không phức tạp hơn. Avoid premature governance.

---

## 7. Metrics tracking quality

Đo process hiệu quả không? 3 metric thôi, check monthly.

1. **Cycle time**: từ task IN-PROGRESS → merged. Target: median ≤ 3 ngày cho S/M task.
2. **Escape rate**: % bug được user report mà không bị CI catch. Target: < 20% trong v0.1.
3. **Backlog health**: tỷ lệ task P0 quá hạn / total P0. Target: < 10%.

Nếu 2/3 metric xấu 2 tháng liên tiếp → process có vấn đề, review lại file này.

---

## 8. Anti-patterns (đã gặp, phải tránh)

Đối chiếu với spec/04 §16 + bổ sung.

- ❌ **Bikeshedding CI config** — nếu Tier 1 gate đã cover, đừng tối ưu 5s build time thay vì làm feature.
- ❌ **Premature tooling** — không setup Sentry/Datadog/Jira cho solo. GitHub Issues đủ.
- ❌ **Perfectionism trên error message** — ship "working + ugly", polish khi có user feedback.
- ❌ **Rewrite khi stuck** — stuck > 2 ngày → hỏi cộng đồng (upstream Discord/Matrix), đừng rewrite.
- ❌ **Meetings với chính mình** — daily standup solo là waste. Weekly review đủ.
- ❌ **Over-documenting internal APIs** — doc public API + spec đủ. Rust code + test = doc.
- ❌ **Bỏ weekly review 3 tuần liên tiếp** — kỷ luật biến mất nhanh nhất ở đây.

---

## 9. Escalation / when it breaks

### 9.1 Khi stuck > 2 ngày trên 1 task

1. Viết "rubber duck" note trong `docs/dev/log/`: state problem, approach đã thử, block ở đâu.
2. Hỏi upstream (BambooEngine GitHub, IBus/Fcitx5 channel).
3. Nếu 3–5 ngày vẫn stuck → **DROP task**, document lý do, chuyển task khác. Không hero.

### 9.2 Khi burnout (R10 spec/05)

Signal: skip weekly review 2 tuần liên tiếp, hoặc cycle time tăng 3x.

Action:
1. Pause commit 1 tuần. Không feel guilty.
2. Viết hoặc không viết — bất kỳ thứ gì helpful.
3. Sau 1 tuần, review scope. Nếu vẫn nặng → trigger R2 scope cut aggressive.
4. Không announce pause công khai trừ khi > 2 tuần (tránh lo cộng đồng vô ích).

### 9.3 Khi nhận bug report security

1. Không public ngay. Reply private email.
2. Fix trên branch riêng.
3. Release patch, announce đồng thời public disclosure.
4. `SECURITY.md` quy định flow này.

---

## 10. Appendix — Decision Record

Mỗi quyết định lớn (chọn crate, thay đổi architecture) → 1 entry ADR tại `docs/dev/decisions.md` format:

```markdown
## D-YYYY-MM-DD-NN — <Title>

**Status**: accepted | superseded by D-...
**Context**: <what forced the decision>
**Decision**: <what we chose>
**Alternatives considered**: <brief list>
**Consequences**: <positive + negative>
```

Lợi ích: 6 tháng sau nhìn lại biết **vì sao** code như vậy. Rất quan trọng cho solo (memory không scale).

---

## 11. TL;DR

Nếu đọc vội, ghi nhớ 7 dòng:

1. **Solo + Automation-as-Team**: CI đóng vai trò mọi vị trí thiếu.
2. **Tier 1 gate** (fmt, clippy, test, deny, audit) chạy mỗi PR — **zero exception**.
3. **1 task IN-PROGRESS** tại 1 thời điểm. Daily log 3 bullets.
4. **Weekly review Chủ nhật** không skip — burndown + risk + CHANGELOG.
5. **Timebox cứng mỗi phase**, vượt 20% → scope cut, không extend.
6. **Release monthly alpha + per-phase minor** via release-plz automation.
7. **ADR mỗi quyết định lớn** — bù cho memory solo không scale.

**Mục tiêu MVP v0.1**: "đủ tốt để 1000 user dùng", không phải "chất lượng nhất tuyệt đối". Đạt "chất lượng nhất" là công việc của v1.0, khi có contributor + telemetry.
