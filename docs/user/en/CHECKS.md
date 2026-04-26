<!--
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Check reference (VD001 – VD015)

One page per check, with the trigger, the evidence Doctor emits, and
what the linked recommendation actually does. IDs are stable across
patch releases — safe to cite in bug reports.

## VD001 — No IM framework active (Critical)

* **Trigger:** A Vietnamese engine package is installed (e.g.
  `ibus-bamboo`) but neither `ibus-daemon` nor `fcitx5` is running.
* **Fix (VR001):** Start the framework you prefer.
  * IBus: `systemctl --user enable --now ibus`
  * Fcitx5: `systemctl --user enable --now fcitx5`

## VD002 — Both frameworks running (Error)

* **Trigger:** IBus and Fcitx5 daemons are both alive.
* **Why:** They fight for the same IM socket; the loser silently
  drops keystrokes.
* **Fix (VR002):** Disable one. Usually Fcitx5 wins on KDE / Sway,
  IBus on GNOME.

## VD003 — IM env vars disagree with active framework (Error)

* **Trigger:** `GTK_IM_MODULE`, `QT_IM_MODULE`, or `XMODIFIERS`
  points at a framework that isn't the active one (or the four vars
  disagree with each other).
* **Fix (VR003):** Export all four to the same value, persisted in
  `/etc/environment` or `~/.config/environment.d/`.

## VD004 — Missing `SDL_IM_MODULE` (Warn)

* **Trigger:** `SDL_IM_MODULE` is unset and a framework is active.
* **Why:** SDL-based games (plus plenty of Electron apps) won't get
  IME events without this.
* **Fix (VR004):** Set it to the same value as `GTK_IM_MODULE`.

## VD005 — Engine installed but not registered (Warn)

* **Trigger:** A Vietnamese engine is available on disk but the
  framework's registered list doesn't include it.
* **Fix (VR005):** Run `ibus-setup` or `fcitx5-configtool`, tick the
  engine, log out / in.

## VD006 — IBus on Wayland (Warn)

* **Trigger:** Session is Wayland; active framework is IBus.
* **Why:** IBus's Wayland support has improved but still lags Fcitx5
  on KDE Plasma and Sway. If input feels "sticky", this is often why.
* **Fix (VR006):** Switch to Fcitx5 (optional, not always needed).

## VD007 — Electron app without Ozone/Wayland (Error, `--app` gated)

* **Trigger:** Target Electron app is running without
  `--ozone-platform=wayland`.
* **Fix (VR007):** Relaunch with the flag, or persist via the
  app's desktop file / `electron-flags.conf`.

## VD008 — Chrome on Wayland without Ozone (Warn, `--app` gated)

* **Trigger:** Chrome/Chromium binary + Wayland session + no Ozone flag.
* **Fix (VR008):** `google-chrome --ozone-platform=wayland` (and add
  the flag to the desktop launcher).

## VD009 — Env var set differently in two files (Warn)

* **Trigger:** The same key appears with different values in two
  different config sources (e.g. `/etc/environment` vs `~/.profile`).
* **Fix (VR009):** Consolidate to one file (the spec recommends
  `/etc/environment` for system-wide, `~/.config/environment.d/` for
  per-user).

## VD010 — VS Code Snap (Warn, `--app vscode`)

* **Trigger:** VS Code binary resolves to a Snap.
* **Why:** Snap sandboxes block IM traffic.
* **Fix (VR010):** Install the `.deb` or `.rpm` build instead.

## VD011 — Flatpak app without IM portal hint (Warn)

* **Trigger:** A Flatpak app lacks the `--talk-name=org.freedesktop.portal.IBus`
  or equivalent permission.
* **Fix (VR011):** Run `flatpak override --user --talk-name=…`.

## VD012 — `INPUT_METHOD` unset (Info)

* **Trigger:** Purely informational — very old apps used this.
* **Fix:** None. Info-only.

## VD013 — Fcitx5 missing platform addon (Warn)

* **Trigger:** Fcitx5 active but the right addon isn't enabled:
  * Wayland session → needs `wayland-im`
  * X11 session → needs `xim`
* **Fix (VR013):** Enable the addon in `fcitx5-configtool` → Addons.

## VD014 — Active locale is not UTF-8 (Warn)

* **Trigger:** `LC_ALL` / `LC_CTYPE` / `LANG` don't resolve to a
  UTF-8 locale (or none are set).
* **Fix (VR014):** `sudo locale-gen en_US.UTF-8` (or your preferred
  UTF-8 locale), export it in `/etc/default/locale` or your shell
  profile.

## VD015 — No Vietnamese engine installed (Info)

* **Trigger:** None of the well-known Vietnamese engines are on disk.
* **Fix:** Install one (`ibus-bamboo`, `fcitx5-bamboo`, etc.).
  Info-only; Doctor doesn't lecture users who just haven't set up
  Vietnamese yet.
