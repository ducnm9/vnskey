# Phase 3 — VietIME Bench v0.1 (Tháng 6–8, 10 tuần)

> **Goal**: release Bench v0.1.0 + public dashboard, CI nightly xanh 14 ngày.
>
> **Exit criteria**: spec/03 §B.16.
>
> **Budget**: 120–160h.

---

## Week 1 — X11 session driver + keystroke injection PoC

### BEN-01 — CLI skeleton
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: DOC-01
- **Spec ref**: spec/03 §A.4
- **Acceptance**:
  - [ ] Subcommands: `run`, `list`, `report`, `compare`, `validate`, `inspect`.
  - [ ] Flags: `--profile`, `--engine`, `--app`, `--mode`, `--session`.

### BEN-02 — `SessionDriver` trait + `XvfbDriver`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (6h)
- **Depends on**: BEN-01
- **Spec ref**: spec/03 §B.2.1
- **Acceptance**:
  - [ ] Spawn `Xvfb :99 -screen 0 1920x1080x24` + openbox window manager.
  - [ ] `SessionHandle` với display `:99`.
  - [ ] `stop()` clean up processes.
  - [ ] Test: spawn xmessage, verify running.

### BEN-03 — `XdotoolInjector`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: BEN-02
- **Spec ref**: spec/03 §B.5
- **Acceptance**:
  - [ ] `type_raw(keys: &str, ms_per_key: u32)` wraps `xdotool type --delay`.
  - [ ] Test: spawn xterm, inject "hello", verify via `xdotool getselection` or getwindowname.

### BEN-04 — PoC: gõ "aa" vào gedit via IBus+Bamboo
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (8h)
- **Depends on**: BEN-03
- **Spec ref**: spec/03 §B.6
- **Acceptance**:
  - [ ] Trong Xvfb session: start ibus-daemon, activate bamboo, launch gedit.
  - [ ] xdotool focus gedit text area, inject "aa".
  - [ ] Read back via AT-SPI (at-spi2 D-Bus) → expect "â".
  - [ ] Document approach trong `docs/dev/bench-poc.md`.

---

## Week 2 — IBus driver + gedit runner + scoring

### BEN-10 — `ImDriver` trait + `IbusDriver`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: BEN-04
- **Spec ref**: spec/03 §B.3
- **Acceptance**:
  - [ ] `start(&SessionHandle)`, `activate_engine("Bamboo")`, `set_mode(Telex)`.
  - [ ] Dùng `ibus engine <name>` + `gsettings set` cho mode config.
  - [ ] Verify qua `ibus engine` query.

### BEN-11 — `AppRunner` trait + `GeditRunner`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: BEN-04
- **Spec ref**: spec/03 §B.4
- **Acceptance**:
  - [ ] `launch`, `focus_text_area`, `clear_text_area`, `read_text`, `close`.
  - [ ] Capture via AT-SPI `get_text`.
  - [ ] Test: launch → inject → read → close.

### BEN-12 — Test vector data model + TOML load
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: BEN-01
- **Spec ref**: spec/03 §A.5, §B.9
- **Acceptance**:
  - [ ] `TestVector { id, input_keys, expected_output, tags }`.
  - [ ] Load `test-vectors/*.toml` với serde.
  - [ ] `vietime-bench validate` check NFC normalization.

### BEN-13 — Scoring engine
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: BEN-12
- **Spec ref**: spec/03 §B.7
- **Acceptance**:
  - [ ] Exact match boolean.
  - [ ] Levenshtein via `strsim` crate.
  - [ ] Normalized edit distance.
  - [ ] `accuracy_pct`, `weighted_score`.
  - [ ] Unit tests.

### BEN-14 — Runner loop + first run result
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (6h)
- **Depends on**: BEN-10, BEN-11, BEN-13
- **Spec ref**: spec/03 §B.6
- **Acceptance**:
  - [ ] Loop: for combo → for vector → inject → capture → score.
  - [ ] Store `ComboResult`.
  - [ ] Run 10 vectors trong gedit, print summary.

---

## Week 3 — More apps: kate + Firefox

### BEN-20 — `KateRunner` (Qt)
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: BEN-11
- **Spec ref**: spec/03 §B.4
- **Acceptance**:
  - [ ] Launch `kate --new`.
  - [ ] AT-SPI capture.
  - [ ] Verify Qt app hoạt động cùng IBus.

### BEN-21 — `FirefoxRunner` via CDP
- **Status**: TODO
- **Priority**: P1
- **Estimate**: L (6h)
- **Depends on**: BEN-11
- **Spec ref**: spec/03 §B.4
- **Acceptance**:
  - [ ] Launch với `--remote-debugging-port=9222`.
  - [ ] Preload `tests/fixtures/bench/textarea.html` URL.
  - [ ] CDP client (`chromiumoxide` crate) connect, focus textarea, eval `document.querySelector('textarea').value`.
  - [ ] Clear text via CDP `focus().value = ''`.

### BEN-22 — `ChromiumRunner` (Chromium/Chrome)
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (3h)
- **Depends on**: BEN-21
- **Spec ref**: spec/03 §B.4
- **Acceptance**:
  - [ ] Reuse CDP logic.
  - [ ] Different binary path.

### BEN-23 — App registry dispatcher
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: BEN-20, BEN-21, BEN-22
- **Spec ref**: spec/03 §B.4
- **Acceptance**:
  - [ ] `resolve_app(id: &str) -> Box<dyn AppRunner>`.
  - [ ] `vietime-bench run --app kate` chọn runner đúng.

---

## Week 4 — Wayland session driver

### BEN-30 — `WestonDriver` (headless Wayland)
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (8h)
- **Depends on**: BEN-02
- **Spec ref**: spec/03 §B.2.2
- **Acceptance**:
  - [ ] Spawn `weston --backend=headless-backend.so`.
  - [ ] Set `WAYLAND_DISPLAY=wayland-0`.
  - [ ] Alternative `cage` fallback nếu weston không có.
  - [ ] Test: spawn foot/gedit, verify running.

### BEN-31 — `YdotoolInjector`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: BEN-30
- **Spec ref**: spec/03 §B.5
- **Acceptance**:
  - [ ] Wrap `ydotool type --key-delay`.
  - [ ] Document `/dev/uinput` permission setup.
  - [ ] Fallback `wtype` binary.
  - [ ] Test: inject into foot terminal, verify capture.

### BEN-32 — Session driver dispatcher
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (2h)
- **Depends on**: BEN-30, BEN-31
- **Spec ref**: spec/03 §B.2
- **Acceptance**:
  - [ ] `--session wayland` dùng Weston + ydotool.
  - [ ] `--session x11` dùng Xvfb + xdotool.
  - [ ] `SessionType` enum.

---

## Week 5 — Fcitx5 driver + full matrix logic

### BEN-40 — `Fcitx5Driver`
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: BEN-10
- **Spec ref**: spec/03 §B.3
- **Acceptance**:
  - [ ] Start `fcitx5 -d`.
  - [ ] `fcitx5-remote -s bamboo` activate.
  - [ ] Edit `~/.config/fcitx5/conf/bamboo.conf` set mode.
  - [ ] Verify qua `fcitx5-remote -n`.

### BEN-41 — Matrix orchestrator
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: BEN-14, BEN-32, BEN-40
- **Spec ref**: spec/03 §B.6
- **Acceptance**:
  - [ ] Combos from `--engine`, `--app`, `--session`, `--mode` flags.
  - [ ] Serialize combo execution (1 VM shared).
  - [ ] Per-combo timeout 20min.
  - [ ] Aggregate `RunResult`.

### BEN-42 — `Profile` definition
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: BEN-41
- **Spec ref**: spec/03 §B.9
- **Acceptance**:
  - [ ] `profiles/smoke.toml` (50 vectors × 3 app × 1 engine).
  - [ ] `profiles/full.toml` (500 × 10+ app × 2 engine).
  - [ ] `profiles/bugs.toml` regression.
  - [ ] `--profile smoke` chạy được.

---

## Week 6 — Electron apps

### BEN-50 — `VsCodeRunner`
- **Status**: TODO
- **Priority**: P1
- **Estimate**: L (8h)
- **Depends on**: BEN-21
- **Spec ref**: spec/03 §B.4
- **Acceptance**:
  - [ ] Launch `code --no-sandbox --inspect-brk --user-data-dir=tmp tmp.txt`.
  - [ ] Electron inspector port CDP.
  - [ ] Focus editor view, read `.view-line` innerText hoặc đọc file từ disk sau save.
  - [ ] Handle Electron autocomplete interference (disable via settings).

### BEN-51 — `SlackRunner` + `DiscordRunner`
- **Status**: TODO
- **Priority**: P2
- **Estimate**: M (6h)
- **Depends on**: BEN-50
- **Spec ref**: spec/03 §B.4
- **Acceptance**:
  - [ ] Electron CDP pattern.
  - [ ] Text area: Slack messenger, Discord input.
  - [ ] Note: cần login hoặc dùng local offline mode (document caveat).

### BEN-52 — `ObsidianRunner`
- **Status**: TODO
- **Priority**: P2
- **Estimate**: M (4h)
- **Depends on**: BEN-50
- **Spec ref**: spec/03 §B.4
- **Acceptance**:
  - [ ] Launch với empty vault.
  - [ ] CDP read markdown editor.

### BEN-53 — `LibreOfficeRunner`
- **Status**: TODO
- **Priority**: P2
- **Estimate**: M (4h)
- **Depends on**: BEN-11
- **Spec ref**: spec/03 §B.4
- **Acceptance**:
  - [ ] `soffice --writer`.
  - [ ] UNO API hoặc AT-SPI capture.
  - [ ] Note caveat: `--headless` có thể bỏ IM.

---

## Week 7 — Report & dashboard

### BEN-60 — Report JSON + Markdown renderer
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: BEN-41
- **Spec ref**: spec/03 §A.6, §B.10
- **Acceptance**:
  - [ ] `vietime-bench report --format json|markdown`.
  - [ ] Schema v1 `schemas/bench-result.v1.json`.
  - [ ] Save to `runs/<id>/summary.json` + `runs/<id>/failures/<vid>.json`.
  - [ ] Screenshot on failure (optional flag).

### BEN-61 — Static HTML dashboard
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (8h)
- **Depends on**: BEN-60
- **Spec ref**: spec/03 §B.10, §A.7
- **Acceptance**:
  - [ ] `askama` templates cho 3 page: index (matrix), combo detail, history.
  - [ ] Vanilla CSS, no JS framework (Chart.js CDN cho history chart).
  - [ ] Data loaded from `runs/*.json`.
  - [ ] Build: `vietime-bench report --format html --output site/`.

### BEN-62 — `compare` command (diff 2 runs)
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (3h)
- **Depends on**: BEN-60
- **Spec ref**: spec/03 §A.4
- **Acceptance**:
  - [ ] `compare --base A --head B` table: accuracy delta per combo.
  - [ ] Highlight regression > 5%.

### BEN-63 — `inspect` command
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (2h)
- **Depends on**: BEN-60
- **Spec ref**: spec/03 §A.4, §B.13
- **Acceptance**:
  - [ ] `inspect <run-id> <vector-id>` print input, expected, actual, key sequence, screenshot path.
  - [ ] Reproducer snippet.

---

## Week 8 — GitHub Actions nightly + first public dashboard

### BEN-70 — `bench-nightly.yml` workflow
- **Status**: TODO
- **Priority**: P0
- **Estimate**: L (8h)
- **Depends on**: BEN-41, BEN-61
- **Spec ref**: spec/03 §B.11
- **Acceptance**:
  - [ ] Cron nightly 2am UTC + manual dispatch.
  - [ ] Matrix combos ibus-bamboo × fcitx5-bamboo × x11 × wayland.
  - [ ] Install deps: Xvfb, weston, xdotool, ydotool, at-spi2-core.
  - [ ] `modprobe uinput` + permission.
  - [ ] Upload artifacts per combo.
  - [ ] Publish job → gh-pages deploy.

### BEN-71 — Retry + flaky detection
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (3h)
- **Depends on**: BEN-41
- **Spec ref**: spec/03 §B.8
- **Acceptance**:
  - [ ] Capture timeout retry 2x.
  - [ ] Injection fail retry 3x.
  - [ ] Flag "flaky" nếu retry thành công.

### BEN-72 — `vietime.io/matrix` domain + DNS
- **Status**: TODO
- **Priority**: P2
- **Estimate**: S (2h)
- **Depends on**: BEN-70
- **Spec ref**: spec/03 §A.7
- **Acceptance**:
  - [ ] Domain mua (nếu dự án đủ traction) hoặc dùng `<user>.github.io/vietime`.
  - [ ] CNAME trong gh-pages.

---

## Week 9 — Test vectors curation

### BEN-80 — 500 Telex test vectors
- **Status**: TODO
- **Priority**: P0
- **Estimate**: XL (16h)
- **Depends on**: BEN-12
- **Spec ref**: spec/03 §A.5
- **Acceptance**:
  - [ ] 500 câu Telex cover: dấu thanh (6 dấu), dấu phụ (â, ê, ô, ơ, ư, ă, đ), double chữ cái, punctuation, numbers mixed, common words, edge cases.
  - [ ] Tag phong phú.
  - [ ] Review 2 lượt: self + 1 reviewer.
  - [ ] Validator pass (NFC normalized).

### BEN-81 — Bugs regression vectors
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (5h)
- **Depends on**: BEN-80
- **Spec ref**: spec/03 §A.5
- **Acceptance**:
  - [ ] Ít nhất 20 bug report thật từ upstream issue tracker được convert thành test vector.
  - [ ] `known_failing_on` annotation.
  - [ ] Link upstream.

### BEN-82 — `validate` command
- **Status**: TODO
- **Priority**: P1
- **Estimate**: S (2h)
- **Depends on**: BEN-12
- **Spec ref**: spec/03 §A.4
- **Acceptance**:
  - [ ] Check Unicode NFC, duplicate IDs, tag consistency.
  - [ ] CI step.

---

## Week 10 — Docs + release

### BEN-90 — `docs/reproduce-locally.md`
- **Status**: TODO
- **Priority**: P1
- **Estimate**: M (4h)
- **Depends on**: BEN-70
- **Spec ref**: spec/03 §B.13
- **Acceptance**:
  - [ ] Hướng dẫn Docker compose chạy Bench locally.
  - [ ] Steps reproduce 1 failed vector.

### BEN-91 — User docs vi + en
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (5h)
- **Depends on**: BEN-90
- **Spec ref**: spec/04 §6
- **Acceptance**:
  - [ ] `docs/vi/bench.md`: giới thiệu compatibility matrix, cách đọc dashboard.
  - [ ] `docs/en/bench.md` mirror.
  - [ ] `docs/vi/contributing-test-vectors.md` hướng dẫn thêm vector.

### BEN-92 — Release v0.1.0
- **Status**: TODO
- **Priority**: P0
- **Estimate**: S (3h)
- **Depends on**: BEN-70, BEN-91
- **Spec ref**: spec/04 §7.3
- **Acceptance**:
  - [ ] Tag `v0.1.0-bench`.
  - [ ] CHANGELOG.
  - [ ] GH Release binary tar.gz (không Flatpak vì Bench chạy trong Docker/VM).

### BEN-93 — Blog post #4 + launch
- **Status**: TODO
- **Priority**: P0
- **Estimate**: M (4h)
- **Depends on**: BEN-92
- **Spec ref**: spec/05 §2 (Phase 3 release)
- **Acceptance**:
  - [ ] Blog "Bảng tương thích gõ tiếng Việt Linux — công khai lần đầu".
  - [ ] Screenshot dashboard.
  - [ ] Post FB + Reddit + r/linux.
  - [ ] DM maintainer upstream với matrix data.

---

## Phase 3 — Exit checklist (spec/03 §B.16)

- [ ] Profile `smoke` chạy < 15min trong CI.
- [ ] JSON schema publish + validate.
- [ ] Dashboard HTML render đúng với ≥ 2 run.
- [ ] CI nightly không flaky > 2% vectors, xanh 14 ngày liên tiếp.
- [ ] 500 vectors review, NFC normalized.
- [ ] `inspect` hiển thị reproducer.
- [ ] README vi + en + Docker compose.

**Timebox rule**: cuối W4 nếu Wayland driver không stable → cắt BEN-30/31/32 sang v0.2, chỉ ship X11 matrix v0.1.
