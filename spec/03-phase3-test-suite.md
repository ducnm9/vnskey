# Phase 3 — VietIME Bench (Compatibility Matrix Runner)

> Automated test harness mô phỏng gõ Telex/VNI vào các app thực trong headless session, so sánh output với expected, publish "App × IME × Mode × Session" compatibility matrix.
> Đọc `00-vision-and-scope.md` trước.

---

## A. PRD

### A.1. Problem statement

Hôm nay, câu hỏi "VS Code trên Ubuntu 24.04 Wayland gõ Telex bằng fcitx5-bamboo có bị loạn chữ không?" chỉ có thể trả lời bằng:
- Cài cả 2 bộ gõ, thử thủ công trong VS Code.
- Đọc GitHub issues và suy luận.
- Hỏi trên Facebook nhóm Linux Vietnam.

Không ai có **dữ liệu khách quan, đo được, reproducible**. Maintainer không biết bug fix có regression không. User không biết nên chọn combo nào.

**Bench giải quyết**: một headless runner chạy trong CI/VM, gõ một bộ test vector Telex vào 20+ app, so sánh output, xuất bảng compatibility matrix. Publish dashboard công khai cập nhật sau mỗi release IME.

### A.2. User stories

**US-1 (Maintainer)**: Là maintainer ibus-bamboo, khi tôi release v0.9.0 tôi chạy `vietime-bench run --engine ibus-bamboo@0.9.0 --profile full` → được báo cáo "VS Code regression: accuracy giảm từ 98% → 72%".

**US-2 (User)**: Là user trước khi cài, tôi xem [vietime.io/matrix](https://vietime.io/matrix) thấy Fcitx5-Bamboo + VS Code trên Wayland có accuracy 99%, còn IBus-Bamboo + VS Code chỉ 85% → quyết định Fcitx5.

**US-3 (Dev)**: Là contributor, tôi muốn thêm test case reproduce bug mất dấu "nghiêng" → tôi viết thêm entry vào `test-vectors/bugs.toml`.

**US-4 (CI)**: Là maintainer VietIME, tôi muốn CI chạy Bench hàng đêm trên master branch bamboo-core, fail nếu regression > 5%.

### A.3. Scope

**In-scope (MVP v0.1)**:
- Test vector: 500+ câu tiếng Việt (Telex) cover dấu thanh, dấu phụ, các case edge (ư/ơ, double chữ cái, số xen kẽ).
- App coverage ban đầu: gedit/Kate/Kwrite (GTK/Qt native), Firefox (Gecko), Chromium (textarea), VS Code, Slack, Discord, Obsidian (Electron).
- Protocol coverage: X11 và Wayland.
- Framework coverage: IBus + Fcitx5.
- Engine coverage: Bamboo + Unikey.
- Input simulation: `xdotool` (X11) + `ydotool`/`wtype` (Wayland).
- Output capture: theo mỗi app, method riêng (accessibility tree, xdotool getactivewindow/getwindowname, hoặc app-specific inspection).
- Report: JSON schema, markdown table, HTML dashboard (static site).
- Run env: headless VM (QEMU + Xvfb/weston), hoặc CI GitHub Actions.
- Scoring: edit distance (Levenshtein) normalized + exact match rate.

**Out-of-scope (MVP)**:
- Real user typing speed simulation (đo latency per keystroke — v0.2).
- Web-based test vector editor.
- Distributed runner.
- Android/iOS (nonsensical).
- Windows/macOS.

**In-scope v0.2+**:
- Latency measurement (ms từ keystroke → hiển thị).
- Undo history test (gõ xong Ctrl+Z, verify đúng).
- Fake-backspace counting (đếm số backspace thật sinh ra).
- Chromium autocomplete xung đột test (trigger intellisense + gõ Telex cùng lúc).

### A.4. Command surface

```
vietime-bench run                       # run full default profile
vietime-bench run --profile smoke       # 50 câu, 3 app
vietime-bench run --profile full        # 500 câu, 15 app
vietime-bench run --profile bugs        # chỉ test case regression
vietime-bench run --engine ibus-bamboo  # chỉ engine này
vietime-bench run --app vscode --engine fcitx5-bamboo --mode telex --session wayland
vietime-bench list                      # list profiles/apps/engines
vietime-bench report --output report.md # render last run
vietime-bench report --format html      # html dashboard
vietime-bench compare --base <run-id> --head <run-id>   # diff 2 runs
vietime-bench validate                  # validate test vectors file
vietime-bench inspect <run-id>          # detail 1 test case
```

### A.5. Test vector format

File `test-vectors/telex.toml` (UTF-8, versioned):

```toml
version = 1
engine_mode = "telex"
description = "Core Telex tones and modifiers"

[[vectors]]
id = "T001"
input_keys = "tieesng Vieejt"      # theo Telex
expected_output = "tiếng Việt"
tags = ["basic", "tone", "two-word"]

[[vectors]]
id = "T002"
input_keys = "aa"
expected_output = "â"
tags = ["modifier", "letter-a-circumflex"]

[[vectors]]
id = "T003"
input_keys = "nghieengs"
expected_output = "nghiếng"
tags = ["tone-acute", "nh"]
# ... 500 entries
```

File `test-vectors/bugs.toml`:

```toml
version = 1

[[vectors]]
id = "BUG-VSCode-2024-01"
input_keys = "xin chaof cacs banj"
expected_output = "xin chào các bạn"
tags = ["electron", "vscode"]
known_failing_on = ["ibus-bamboo@<=0.8.2 + vscode + wayland"]
upstream_issue = "https://github.com/BambooEngine/ibus-bamboo/issues/NNN"
```

### A.6. Output schema

Run summary `runs/<id>/summary.json`:

```json
{
  "schema_version": 1,
  "run_id": "2026-03-14T12-00-00Z-abc1234",
  "started_at": "2026-03-14T12:00:00Z",
  "finished_at": "2026-03-14T12:18:32Z",
  "env": {
    "distro": "Ubuntu 24.04",
    "session": "Wayland",
    "compositor": "weston 13.0",
    "tool_versions": {
      "vietime-bench": "0.1.0",
      "ibus": "1.5.29",
      "fcitx5": "5.1.7",
      "bamboo": "0.8.2"
    }
  },
  "matrix": [
    {
      "engine": "fcitx5-bamboo",
      "app": "vscode",
      "session": "wayland",
      "mode": "telex",
      "vectors_tested": 500,
      "exact_match": 486,
      "edit_distance_total": 42,
      "accuracy_pct": 97.2,
      "failed_ids": ["T047", "T132", ...],
      "duration_ms": 72400
    }
  ],
  "anomalies": [
    {
      "kind": "CaptureFailure",
      "detail": "Could not read VS Code accessibility tree after 5s",
      "retry_count": 3
    }
  ]
}
```

Failed case detail `runs/<id>/failures/<vector_id>.json`:

```json
{
  "vector_id": "T047",
  "expected": "người",
  "actual": "nnggười",
  "edit_distance": 2,
  "screenshot": "screenshots/T047.png",
  "key_sequence_sent": ["n", "g", "u", "o", "w", "i", "f"],
  "notes": "Suspected fake-backspace race with Electron IPC"
}
```

### A.7. Public dashboard

- Static HTML site generated from latest run.
- Deployed GitHub Pages `vietime.io/matrix` (hoặc subdomain).
- View modes:
  - Matrix view: rows = apps, cols = engines × sessions, cell = accuracy %.
  - Per-app detail: failed vectors list + copy-paste reproducer.
  - Historical chart: accuracy over engine versions.

### A.8. Success criteria (Phase 3)

- Chạy profile `smoke` trong CI GitHub Actions < 15 phút.
- Profile `full` < 2 giờ trên VM 4-core.
- Compatibility matrix cho ≥ 4 combo × ≥ 10 app.
- Maintainer ibus-bamboo xác nhận matrix hữu ích cho regression triage.
- Dashboard công khai update mỗi release engine.

---

## B. Technical Design

### B.1. Architecture

```
┌───────────────────────────────────────────────────────┐
│                   CLI (clap)                           │
└─────────────────────────┬─────────────────────────────┘
                          │
            ┌─────────────▼─────────────┐
            │       Orchestrator        │
            │  - parses profile         │
            │  - spawns VM/session      │
            │  - coordinates runners    │
            └─┬───────────────────────┬─┘
              │                       │
     ┌────────▼──────┐      ┌─────────▼─────────┐
     │ SessionDriver │      │  ResultCollector  │
     │ (X11/Wayland  │      │  (aggregate, save)│
     │  headless)    │      └───────────────────┘
     └────────┬──────┘
              │
   ┌──────────┴──────────┐
   │                     │
   ▼                     ▼
AppRunner             KeystrokeInjector
(gedit, firefox, ...)    (xdotool/ydotool/wtype)
   │                     │
   └──────────┬──────────┘
              │
              ▼
         OutputCapture
     (a11y tree / x-selection / app RPC)
```

### B.2. Session drivers

Hai variant:

#### B.2.1. X11 headless
- `Xvfb :99 -screen 0 1920x1080x24`
- `DISPLAY=:99`, session env export.
- Window manager: `openbox` (nhẹ, support EWMH).
- IM: start `ibus-daemon` hoặc `fcitx5` ngầm.

#### B.2.2. Wayland headless
- Compositor: `weston` với `--backend=headless-backend.so`.
- `WAYLAND_DISPLAY=wayland-0`.
- Alternative: `labwc` hoặc `cage` cho compositor đơn giản.
- Tool inject: `ydotool` (uinput, cần `/dev/uinput` — trong Docker phải `--device`).
- `wtype` fallback cho Wayland desktop.

Driver trait:

```rust
#[async_trait]
pub trait SessionDriver: Send + Sync {
    async fn start(&mut self) -> Result<SessionHandle>;
    async fn stop(&mut self) -> Result<()>;
    fn session_type(&self) -> SessionType;
    fn env_vars(&self) -> HashMap<String, String>;
}

pub struct SessionHandle {
    pub display: String,          // ":99" hoặc "wayland-0"
    pub pid: u32,
}
```

### B.3. IM framework drivers

```rust
#[async_trait]
pub trait ImDriver: Send + Sync {
    async fn start(&mut self, session: &SessionHandle) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn activate_engine(&self, engine_name: &str) -> Result<()>;
    async fn set_mode(&self, mode: InputMode) -> Result<()>;   // Telex/VNI/VIQR
    fn framework(&self) -> ImFramework;
}

pub enum InputMode { Telex, Vni, Viqr, SimpleTelex }
```

Implementations: `IbusDriver`, `Fcitx5Driver`.

**IbusDriver** gọi `ibus engine <name>` và configure via `gsettings org.freedesktop.ibus.engine.bamboo input-method telex`.

**Fcitx5Driver** gọi `fcitx5-remote -s bamboo` và edit `~/.config/fcitx5/conf/bamboo.conf`.

### B.4. App runners

Mỗi app có một runner tuân thủ trait:

```rust
#[async_trait]
pub trait AppRunner: Send + Sync {
    fn id(&self) -> &'static str;
    fn kind(&self) -> AppKind;
    async fn launch(&mut self, session: &SessionHandle) -> Result<AppInstance>;
    async fn focus_text_area(&self, inst: &AppInstance) -> Result<()>;
    async fn clear_text_area(&self, inst: &AppInstance) -> Result<()>;
    async fn read_text(&self, inst: &AppInstance) -> Result<String>;
    async fn close(&mut self, inst: AppInstance) -> Result<()>;
}

pub struct AppInstance {
    pub pid: u32,
    pub window_id: Option<String>,
    pub extra: Value,        // per-runner state
}
```

**Apps & capture methods**:

| App | Launch | Focus text area | Read text |
|---|---|---|---|
| gedit | `gedit --new-document` | AT-SPI focus to Gtk.TextView | AT-SPI `get_text` |
| kate | `kate --new` | AT-SPI | AT-SPI |
| Firefox | `firefox --headless` + page với `<textarea>` via `--remote-debugging-port` | CDP focus | CDP eval `document.querySelector('textarea').value` |
| Chromium/Chrome | Same pattern with CDP | CDP | CDP |
| VS Code | `code --no-sandbox --user-data-dir=tmp tmp.txt` | CDP (Electron có DevTools port) | CDP `document.querySelector('.view-line').innerText` hoặc read file từ disk (nếu save) |
| Slack | Electron → CDP | CDP | Read DOM input value |
| Discord | Electron → CDP | CDP | Read DOM |
| Obsidian | Electron → CDP | CDP | CDP read markdown editor content |
| LibreOffice Writer | `soffice --writer --headless` (chú ý: headless có thể bỏ IM — verify) | UNO API | UNO |
| Neovide | skip — console app | | |
| xterm | `xterm -e cat` | xdotool focus | xdotool getselection |

**Capture priority**:
1. AT-SPI (GTK/Qt native) → chuẩn nhất.
2. CDP (Chromium/Electron) → chuẩn cho web app.
3. `xdotool getactivewindow + xdotool key ctrl+a ctrl+c` rồi `xclip -o` → fallback, không reliable cho Wayland.

**Wayland caveat**: X11 helper không work. Dùng:
- `atspi2` qua D-Bus (chạy được trong Wayland).
- CDP (vẫn work).
- `wtype` + xác nhận qua a11y.

### B.5. Keystroke injector

```rust
#[async_trait]
pub trait KeystrokeInjector: Send + Sync {
    async fn type_raw(&self, keys: &str, ms_per_key: u32) -> Result<()>;
    async fn type_keysyms(&self, syms: &[KeySym], ms_per_key: u32) -> Result<()>;
}

// X11: XdotoolInjector (wraps `xdotool type --delay`)
// Wayland: YdotoolInjector (wraps `ydotool type --key-delay`) or WtypeInjector
```

**Timing**: default 30ms/ký tự (~2000 chars/min, realistic). Adjustable per-test để test "typing fast" scenario.

**Important**: inject qua keysym giống bàn phím thật, không bypass IME. Mục tiêu là test IME, không test output văn bản.

### B.6. Test vector runner flow

```
for combo in (engine × app × mode × session):
    driver.start_session()
    im_driver.start()
    im_driver.activate_engine()
    im_driver.set_mode()
    app.launch()
    for vector in profile.vectors:
        app.clear_text_area()
        app.focus_text_area()
        injector.type(vector.input_keys, 30ms)
        sleep(200ms)   # wait for IME to flush
        actual = app.read_text()
        compare(actual, vector.expected_output) → store result
        if crash: capture screenshot, mark anomaly
    app.close()
    im_driver.stop()
    driver.stop_session()
```

**Parallelism**: combos serialize (VM là tài nguyên shared). Vectors trong 1 combo cũng serialize. Nếu CI nhiều runner, phân chia combos song song.

### B.7. Scoring

Per-vector:
- **Exact match** (1/0).
- **Edit distance** (Levenshtein) normalized: `1 - dist/max(len(expected), len(actual))`.
- **Tone preservation**: so sánh chỉ ký tự có dấu.

Per-combo:
- `accuracy_pct = exact_match_count / total_vectors * 100`.
- `weighted_score = avg(normalized_edit_distance)`.

Both stored. Dashboard hiển thị accuracy_pct primary, edit-distance secondary.

### B.8. Failure modes

1. **Injection fail** (ydotool permission denied): skip vector, mark anomaly, retry 3x.
2. **App không launch**: skip app, mark in report.
3. **IM không activate engine**: fail-fast, abort run (misconfig).
4. **Capture timeout**: retry 2x, then skip vector.
5. **Output read trả về rỗng**: retry 1x sau 500ms (IME chưa flush).

Max per-combo timeout: 20 phút. Vượt → abort combo.

### B.9. Data model

```rust
pub struct RunConfig {
    pub profile: Profile,
    pub combos: Vec<Combo>,          // reuse từ core
    pub vectors: Vec<TestVector>,
    pub ms_per_key: u32,
    pub capture_screenshots_on_fail: bool,
}

pub struct Profile {
    pub name: String,
    pub description: String,
    pub app_ids: Vec<String>,
    pub engines: Vec<String>,
    pub sessions: Vec<SessionType>,
    pub vector_filter: VectorFilter,
}

pub enum VectorFilter {
    All,
    Tagged(Vec<String>),
    Ids(Vec<String>),
    Profile(String),
}

pub struct TestVector {
    pub id: String,
    pub input_keys: String,
    pub expected_output: String,
    pub tags: Vec<String>,
}

pub struct RunResult {
    pub run_id: String,
    pub env: RunEnv,
    pub matrix: Vec<ComboResult>,
    pub anomalies: Vec<Anomaly>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

pub struct ComboResult {
    pub engine: String,
    pub app: String,
    pub session: SessionType,
    pub mode: InputMode,
    pub vectors_tested: u32,
    pub exact_match: u32,
    pub edit_distance_total: u32,
    pub accuracy_pct: f32,
    pub failures: Vec<VectorFailure>,
    pub duration: Duration,
}

pub struct VectorFailure {
    pub vector_id: String,
    pub expected: String,
    pub actual: String,
    pub edit_distance: u32,
    pub screenshot_path: Option<PathBuf>,
    pub key_sequence_sent: Vec<String>,
}
```

### B.10. Dashboard generation

- Post-run step: `vietime-bench report --format html --output site/`
- Static HTML + CSS + minimal JS (no framework, no build step).
- Template: `askama` (Rust) or hand-written.
- Data: read `runs/*.json` + `runs/latest.json`.
- Publish: GitHub Actions push to `gh-pages` branch after successful nightly run.

**Pages**:
- `index.html`: matrix table.
- `combo/<engine>/<app>/<session>.html`: drill-down.
- `history.html`: time series chart (Chart.js CDN or inline).
- `about.html`: methodology + reproducibility instructions.

### B.11. CI integration

GitHub Actions workflow `.github/workflows/bench-nightly.yml`:

```yaml
on:
  schedule: [{ cron: "0 2 * * *" }]       # 2am UTC daily
  workflow_dispatch:
jobs:
  bench:
    runs-on: ubuntu-22.04
    strategy:
      matrix:
        combo:
          - { engine: "ibus-bamboo", session: "x11" }
          - { engine: "fcitx5-bamboo", session: "x11" }
          - { engine: "fcitx5-bamboo", session: "wayland" }
    steps:
      - uses: actions/checkout@v4
      - name: Install deps
        run: sudo apt install -y xvfb weston xdotool ydotool at-spi2-core
      - name: Start /dev/uinput (for ydotool)
        run: sudo modprobe uinput && sudo chmod 666 /dev/uinput
      - name: Run bench
        run: vietime-bench run --profile smoke --engine ${{ matrix.combo.engine }} --session ${{ matrix.combo.session }}
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: bench-${{ matrix.combo.engine }}-${{ matrix.combo.session }}
          path: runs/latest
  publish:
    needs: bench
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/download-artifact@v4
      - name: Generate HTML
        run: vietime-bench report --format html --output site/
      - name: Deploy
        uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: site/
```

### B.12. Test vector authoring guidelines

- ID stable, không renumber khi xóa/thêm.
- Tag phong phú để filter (basic, tone, modifier, double-letter, punctuation, emoji-adjacent, ...).
- Không hardcode tên engine version (để run cross-engine).
- Bugs file chỉ chứa reproducer, reference upstream issue.
- Review PR vector file: kiểm tra `expected_output` Unicode normalization (NFC).

### B.13. Reproducibility

- Mọi run ghi đầy đủ tool versions.
- `vietime-bench inspect <run-id> <vector-id>` in ra toàn bộ input + actual + expected + screenshot path.
- `docs/reproduce-locally.md` hướng dẫn user rebuild 1 test case thủ công.

### B.14. Roadmap Phase 3

| Tuần | Milestone |
|---|---|
| 1 | Session driver X11 (Xvfb), keystroke injector xdotool, PoC gõ vào gedit |
| 2 | IM driver IBus, run 10 vector trong gedit, score |
| 3 | App runner framework trait, add kate (Qt), add Firefox via CDP |
| 4 | Session driver Wayland (weston), ydotool injector |
| 5 | IM driver Fcitx5, full engine × framework matrix logic |
| 6 | Electron apps (VS Code, Slack) via CDP |
| 7 | Report JSON/markdown, dashboard HTML template |
| 8 | GitHub Actions nightly workflow, first public dashboard |
| 9 | 500 test vectors curated, tagging, validation tool |
| 10 | Docs vi+en, blog post, v0.1.0 release |

Timebox **10 tuần** (Phase 3 phức tạp hơn).

### B.15. Risks

| Risk | Mitigation |
|---|---|
| Wayland headless + IME chưa mature, ydotool permission phức tạp | Fallback X11-only matrix cho MVP, Wayland v0.2 |
| Electron CDP đổi protocol giữa versions | Version-pin test rig Electron apps, document version |
| AT-SPI không work trong headless Weston | Dùng CDP cho Electron, accept không cover native Qt/GTK ở Wayland trong MVP |
| Test vector bị outdated khi engine fix bug | Vector giữ nguyên expected; "fix" engine cải thiện accuracy dần theo thời gian — đó là insight |
| Flaky test → noise trong matrix | Median of 3 runs; retry logic; flag "flaky" trong vector |
| Maintainer không care về matrix → chỉ mình mình dùng | Chấp nhận. Matrix vẫn giúp user chọn combo |

### B.16. Acceptance criteria

- [ ] Profile `smoke` (50 vectors × 3 app × 2 engine × 1 session) chạy < 15 phút.
- [ ] JSON schema publish + validate.
- [ ] Dashboard HTML render đúng với 2 run khác nhau trong `runs/`.
- [ ] CI GitHub Actions chạy nightly, không flaky > 2% vectors.
- [ ] 500 test vectors đã review, Unicode normalized NFC.
- [ ] `vietime-bench inspect` hiển thị đầy đủ reproducer.
- [ ] README vi+en với hướng dẫn chạy locally (Docker compose).
