# Cross-cutting Concerns

> Những quyết định áp dụng cho **cả 3 component**. Đọc `00-vision-and-scope.md` trước.

---

## 1. Naming & branding

**Project name**: **VietIME Suite**.

Lý do:
- "Viet" + "IME" self-explanatory trong tiếng Anh.
- Không giống tên hiện có (Bamboo, Unikey) → không gây nhầm hoặc xung đột cộng đồng.
- Ngắn, gõ được.

**Component naming**:
- `vietime-doctor`, `vietime-installer`, `vietime-bench` (kebab-case binary).
- Crate: `vietime-core`, `vietime-doctor`, `vietime-installer`, `vietime-bench`.
- Namespace: `io.github.vietime.*` cho Flatpak app id.

**Alternative considered & rejected**:
- `vniplant`: khó đọc.
- `vntype`: trùng tên product thương mại.
- `goyvanh`, `gotieng`: tiếng Việt dấu không cross-platform.

**Tagline** (vi): "Gõ tiếng Việt Linux không còn đau đầu."
**Tagline** (en): "Vietnamese input on Linux, made painless."

---

## 2. Logo & visual identity

MVP phase không đầu tư identity. Một wordmark đơn giản đủ:

- Font: [JetBrains Mono](https://www.jetbrains.com/lp/mono/) bold cho wordmark.
- Primary color: `#D4342E` (đỏ cờ Việt) — dùng ở CTA.
- Secondary: `#FFC107` (vàng ngôi sao) — accent.
- Dark mode default.

Logo asset: SVG wordmark "vietime" trong `branding/`. Không tạo animated logo, không Lottie.

---

## 3. License

**GPLv3 toàn workspace**.

Lý do:
- Tương thích upstream IBus, Fcitx5, ibus-bamboo, fcitx5-bamboo (đều GPL).
- Ngăn vendor lock-in nếu dự án được fork thương mại.
- Mọi dependency bắt buộc phải GPL-compatible (no OpenSSL-licensed, no proprietary).

File mỗi source: header SPDX `// SPDX-License-Identifier: GPL-3.0-or-later`.

`LICENSE` ở workspace root.
`COPYING` nếu quy ước upstream yêu cầu.

**Third-party**: dùng `cargo-deny` check license/ban list. Allowlist: MIT, Apache-2.0, BSD-2/3, ISC, MPL-2.0, GPL-2+, GPL-3+, LGPL-3+, Unlicense, CC0.

---

## 4. Telemetry (opt-in) — MVP v0.2, NOT v0.1

**Rule**: v0.1 không có telemetry. Không thu thập gì. Tool chạy 100% offline.

**v0.2** (future) nếu có telemetry:
- Opt-in tường minh qua flag `--telemetry=on` hoặc prompt lần đầu chạy.
- Hiển thị **đúng payload** sẽ gửi trước khi gửi.
- Endpoint: `plausible.io` hosted hoặc tự host sau (không custom backend MVP).
- Dữ liệu:
  - Distro, session type, DE.
  - Engine + version được scan.
  - Check failures (IDs, không kèm paths).
  - Tool version.
- **Không** thu thập: username, hostname, IP (server drop), app-specific data, keystroke content.
- User có thể xem dữ liệu thật: `vietime-doctor telemetry preview`.
- Privacy policy page: `docs/PRIVACY.md` trong repo + site.

---

## 5. Internationalization (i18n)

**MVP**: tiếng Việt + tiếng Anh.

- String store: `fluent` (Mozilla) hoặc `i18n-embed`. Chọn `fluent` vì native Rust support.
- File: `locale/vi/main.ftl`, `locale/en/main.ftl`.
- Default: detect `$LANG`. Override: `--locale=vi` hoặc `--locale=en`.

**Report & log**: JSON key bằng tiếng Anh (machine-readable), human-readable text dịch.

Future: thêm `locale/fr`, `locale/zh-CN` nếu có contributor.

---

## 6. Documentation strategy

### 6.1. Cấu trúc

```
docs/
├── vi/
│   ├── README.md              # landing
│   ├── quickstart.md
│   ├── install.md             # how to install VietIME Suite
│   ├── doctor.md              # user guide
│   ├── installer.md
│   ├── bench.md
│   ├── troubleshooting.md     # FAQ lỗi thường gặp
│   └── glossary.md            # thuật ngữ IM (GTK_IM_MODULE...)
├── en/
│   └── ... mirror
├── dev/
│   ├── architecture.md        # link vào spec/
│   ├── contributing.md
│   ├── building.md
│   └── release.md
└── PRIVACY.md
```

### 6.2. Nguyên tắc viết

- Người đọc chính là P1 (dev mới lên Linux) — dùng tiếng Việt dễ hiểu, tránh jargon không giải thích.
- Mỗi doc page ≤ 500 từ. Có mục lục nếu dài hơn.
- Code snippets copy-paste được, test được.
- Mỗi guide có "Sanity check cuối" — 1 lệnh verify xong.

### 6.3. Site

Static site gen với **mdBook** (Rust-native, binary duy nhất, không Node build).

Publish `vietime.io` (mua domain nếu project traction) hoặc `username.github.io/vietime`.

---

## 7. Release process

### 7.1. Versioning

SemVer cho mỗi component độc lập:
- `vietime-doctor 0.1.0`, `vietime-installer 0.1.0`, `vietime-bench 0.1.0`.
- `vietime-core` semver trong workspace, nhưng user không cài trực tiếp.

### 7.2. Release channels

- **stable** (GitHub Releases + Flatpak stable + deb stable-repo).
- **nightly** (GitHub Actions build, không publish Flatpak).

v0.1.x stable sau khi đạt all acceptance criteria của phase.

### 7.3. Release checklist (per component)

```
- [ ] CHANGELOG.md updated
- [ ] Version bumped trong Cargo.toml
- [ ] Git tag vX.Y.Z
- [ ] GitHub Release với binary + signature
- [ ] Flatpak manifest bumped (Doctor + Bench)
- [ ] deb package built, pushed to apt.vietime.io (nếu có)
- [ ] AUR PKGBUILD updated
- [ ] Docs site regenerated
- [ ] Announcement (vi Facebook group, en r/vietnam, r/linux)
- [ ] Close milestone, open next
```

### 7.4. Signing

Release binary SHA256 + GPG sign với key công khai. Publish `KEYS` file trong repo root.

---

## 8. Repository & workflow

### 8.1. Layout

Cargo workspace monorepo (xem `00-vision-and-scope.md` §7).

### 8.2. Branches

- `main`: luôn xanh.
- `phase-N/feature-xxx`: feature branch cho Phase N work.
- `release/X.Y`: stabilization.

Protected `main`: require PR + 1 approval (khi có contributor; 1 người cũng phải PR để log thay đổi).

### 8.3. Commit style

[Conventional Commits](https://www.conventionalcommits.org/):
```
feat(doctor): add Fedora detector
fix(installer): preserve comments in /etc/environment
docs(vi): add troubleshooting section for Wayland
test(bench): cover T047 regression
chore: bump zbus to 5.0
```

Hook: `cargo-husky` hoặc pre-commit script chạy `cargo fmt --check` + `cargo clippy`.

### 8.4. CI

GitHub Actions:
- `ci.yml`: `fmt`, `clippy`, `test`, `cargo-deny`, cho mọi PR.
- `release.yml`: tag triggered, build binary cho linux-x86_64 + linux-aarch64, upload to Release.
- `nightly.yml`: chỉ cho `vietime-bench` matrix run.

Self-hosted runner **không** cần ở MVP.

---

## 9. Contributing

`CONTRIBUTING.md` + `CODE_OF_CONDUCT.md` (Contributor Covenant) ở root.

- Issue template: bug / feature / compat-matrix-entry.
- PR template: checklist test + docs.
- Good first issues tagged `good-first-issue` — prioritize setup + detector + test vector additions.

**Phong cách review**:
- Code review tiếng Việt hoặc tiếng Anh đều OK.
- Không gatekeeper vô lý, prefer merge nếu thay đổi nhỏ và an toàn.
- Test là điều kiện merge — không có test, không merge (ngoại trừ docs).

---

## 10. Security

### 10.1. Attack surface

- Installer đụng `/etc/environment`, `~/.profile`, systemd user units → **phải** được audit.
- Bench chạy app real → sandbox bằng VM hoặc container là tốt nhất.
- Doctor pure read-only → minimal surface.

### 10.2. Policies

- Không bao giờ execute shell string interpolation không escape.
- Không bao giờ `curl | sh` trong bất kỳ doc nào.
- Mọi file write có sha256 pre/post verify khi rollback.
- Supply chain: `cargo-deny` + `cargo-audit` trong CI.
- Signed releases (GPG).
- Report lỗi bảo mật: `SECURITY.md` hướng dẫn email riêng (không issue public).

### 10.3. Sandbox

- Installer **không chạy được trong Flatpak** (cần access `/etc`). User phải cài native.
- Doctor **chạy được trong Flatpak** với minimal permission: read `/proc`, D-Bus session, `/etc/environment` (readonly mount).
- Bench **chạy trong VM/Docker** (test bên trong, không trên host user).

---

## 11. Performance budget

| Component | Cold start | Full run | Binary size |
|---|---|---|---|
| Doctor default | < 500ms | < 2s | < 8 MB |
| Doctor `--app X` | < 800ms | < 3s | — |
| Installer `install --yes` | — | < 90s (không tính apt network) | < 10 MB |
| Installer `rollback` | — | < 5s | — |
| Bench `smoke` | — | < 15min | < 15 MB |
| Bench `full` | — | < 2h | — |

Monitor: CI có step `hyperfine` cho Doctor cold start, fail nếu regression > 20%.

---

## 12. Quality gates (workspace-wide)

Mọi PR phải pass:
- `cargo fmt --check`.
- `cargo clippy -- -D warnings` (profile: default + `--all-features`).
- `cargo test --workspace`.
- `cargo deny check` (license + advisories).
- `cargo audit` (RUSTSEC).
- Coverage (tarpaulin) không giảm > 2% so với main.

Optional checks (không block):
- `cargo mutants` mutation testing.
- `miri` cho code có unsafe (MVP không có unsafe).

---

## 13. Data minimization

- Doctor report redact mặc định.
- Installer logs redact env values (chỉ giữ key).
- Bench không lưu screenshot ngoài failed vectors; xóa sau 30 ngày trong `runs/` (user opt-in giữ).

---

## 14. Accessibility

- CLI output: không dùng màu để chuyển tải thông tin (chỉ accent). Mọi status có ASCII marker (`[OK]`, `[WARN]`, `[ERR]`).
- `--no-color` honored (và tự disable khi `NO_COLOR` env set hoặc stdout không phải TTY).
- Docs site: WCAG AA, font size ≥ 16px.

---

## 15. Community

- Facebook group: tham gia "Linux Việt Nam" & "Ubuntu Việt Nam" — post mỗi release lớn.
- Reddit: `r/vietnam`, `r/linuxvietnam` (nếu có traction).
- Discord/Matrix: **không tạo** server riêng ở MVP. Dùng GitHub Discussions.
- Blog: 1 post mỗi quarter tối thiểu. Viết trên `vietime.io/blog` (mdBook subdir).

---

## 16. Anti-pattern checklist (đừng làm)

- ❌ Tạo Electron app cho Doctor/Installer (đi ngược thông điệp "native, lightweight").
- ❌ Viết IME engine dù chỉ "cho vui" — đi sai scope.
- ❌ Host backend server cho telemetry ở MVP.
- ❌ Buộc user vào Discord/Slack private.
- ❌ Đổi license sau khi có contributor.
- ❌ Breaking API change giữa patch version.
- ❌ Dùng OpenAI/LLM trong tool (không cần, không phù hợp).
- ❌ Ship binary không sign.
- ❌ Silent failure (error → log debug → move on). Luôn bubble up.
- ❌ `unwrap()` trong production code.
