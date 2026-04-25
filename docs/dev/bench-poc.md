# Bench PoC — "type `aa` into gedit via IBus + Bamboo"

> BEN-04 (Phase 3 Week 1). Non-runnable this week: the actual end-to-end
> integration test lands in BEN-14 (Week 2). This doc describes the target
> workflow so anyone picking up Week 2 already knows which Week-1 APIs to
> call and which failure modes to expect.

## Goal

Prove the bench harness can:

1. Bring up a headless X11 server without touching the developer's desktop.
2. Load an IM framework (IBus + ibus-bamboo) pointed at that server.
3. Launch a real GUI app (gedit) inside that server.
4. Inject keystrokes (`aa`) and observe that the composed text buffer reads
   `â`.

If the full loop works for this one combo, the matrix runner arriving in
BEN-14 only has to parametrise over combos and test vectors — the risky
parts (headless display, IM daemon lifecycle, keystroke timing, at-spi
readback) are already proven.

## Why this specific combo

| Choice | Why |
|---|---|
| IBus + Bamboo | Bamboo is the most common community choice; IBus has slightly richer at-spi introspection than Fcitx5. |
| gedit | Pure GTK, no custom text widget, at-spi tree is trivial. |
| `aa` → `â` | The minimal Vietnamese composition: one doubled vowel, one tone-free output, no clipboard interaction. |
| Telex | Default mode for Bamboo. Gives the shortest path from install to observable output. |

We deliberately avoid Fcitx5, Firefox, and Unikey for the PoC — each adds a
failure mode that's useful to test in the full matrix but not on Day 1.

## Host prerequisites

Ubuntu 24.04 LTS is the reference box (matches `PreState::fixture_ubuntu_24_04`
in `vietime-installer`). Required packages:

```
Xvfb openbox                # display server + minimal WM
ibus ibus-bamboo            # IM framework + engine
gedit                       # target app
xdotool                     # keystroke injector
at-spi2-core                # a11y bus for readback
dbus-x11                    # dbus-run-session wrapper
```

macOS developers **cannot** run this PoC locally. The integration test that
codifies it (arriving in BEN-14) is gated behind `#[ignore]`; CI (Week 8,
BEN-70) runs it on an Ubuntu runner with the packages above pre-installed.

## Run flow (using Week-1 library APIs)

Every Rust snippet below uses only symbols the Week-1 library exports.
The orchestrator and test-vector loader are deferred to Week 2.

```rust
use vietime_bench::{SessionDriver, XvfbDriver, KeystrokeInjector, XdotoolInjector};

# async fn run() -> anyhow::Result<()> {
// 1. Headless X server.
let mut xvfb = XvfbDriver::new();
let handle = xvfb.start().await?;                 // → DISPLAY=":99"
let display = handle.display.clone();

// 2. IBus daemon. (Spawned directly with tokio::process — no driver yet;
//    an `ImDriver` trait arrives in BEN-10, Week 2.)
let ibus = tokio::process::Command::new("ibus-daemon")
    .args(["-drxR"])
    .env("DISPLAY", &display)
    .kill_on_drop(true)
    .spawn()?;

// 3. Select the Bamboo engine. Needs the daemon's dbus to be live, hence
//    the retry — ibus's "ready" condition is famously unreliable.
wait_for(|| run(["ibus", "engine", "Bamboo"])).await?;

// 4. Target app.
let mut gedit = tokio::process::Command::new("gedit")
    .env("DISPLAY", &display)
    .env("GTK_IM_MODULE", "ibus")
    .env("QT_IM_MODULE", "ibus")
    .env("XMODIFIERS", "@im=ibus")
    .kill_on_drop(true)
    .spawn()?;

// 5. Type. `type_raw` handles the xdotool `--` guard for us.
let injector = XdotoolInjector::new(&display);
injector.type_raw("aa", 30).await?;

// 6. Readback (Week-2 territory; the PoC sketches it here so the
//    integration test has a known target).
//    at_spi_read_focused_text().await?  →  expect "â"

// 7. Teardown. Every child is `kill_on_drop`, so just dropping is enough,
//    but the driver's `stop()` is the public API.
xvfb.stop().await?;
# Ok(())
# }
```

## Failure modes and which Week-1 error variants surface them

| Symptom | Cause | Surfaces as |
|---|---|---|
| `Xvfb` binary absent | `apt install xvfb` missing | `SessionError::BinaryMissing("Xvfb")` |
| `:99` socket already exists | another Xvfb still running | auto-picker in `XvfbDriver::new()` steps to `:100`; if the whole `99..256` range is taken, `start()` returns `SessionError::StartupTimeout { what: "Xvfb", secs: 2 }` |
| `openbox` missing | package not installed | `SessionError::BinaryMissing("openbox")` |
| Xvfb crashes before creating socket | broken xorg stack | `SessionError::StartupTimeout { what: "Xvfb", secs: 2 }` |
| `xdotool` missing | package not installed | `InjectorError::BinaryMissing("xdotool")` |
| `xdotool type` can't find a window | gedit not yet focused | `InjectorError::NonZeroExit { binary: "xdotool", .. }` |
| `ms_per_key` is 30 000 (caller confused seconds and milliseconds) | user error | silently clamped to 2 000 inside `clamp_ms` — run stays responsive |

IBus-specific failures (engine not registered, daemon crash) are **not**
covered by Week-1 types. The `ImDriver` trait arriving in BEN-10 / BEN-11
(Week 2) introduces `ImError::EngineNotRegistered { name }` and friends;
until then the PoC simply surfaces them as `std::io::Error` from the raw
`tokio::process::Command`.

## Cadence for turning this into CI

1. **BEN-14 (Week 2):** promote the sketch above into a
   `tests/integration_bench.rs` test, gated `#[ignore]`. The test types `aa`,
   asserts the at-spi buffer says `â`, and tears down. macOS dev boxes keep
   ignoring it; CI stays unchanged.
2. **BEN-41 (Week 5):** add the Fcitx5 and Unikey siblings once the
   `ImDriver` trait is real.
3. **BEN-70 (Week 8):** GitHub Actions workflow runs the full matrix on an
   Ubuntu 24.04 runner nightly. `cargo test --workspace -- --include-ignored`
   only runs on that job, never on PRs.

## Reference

- `spec/03-phase3-test-suite.md` §B.3 — SessionDriver contract.
- `spec/03-phase3-test-suite.md` §B.4 — KeystrokeInjector contract.
- `spec/03-phase3-test-suite.md` §B.6 — full run-flow pseudocode.
- `tasks/phase-3-bench.md` Week 1 — acceptance criteria for BEN-01…04.
