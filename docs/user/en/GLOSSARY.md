<!--
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Glossary (English)

**IM / Input Method**
Software that turns keystrokes into composed characters. Required for
Vietnamese typing because `đ`, tone marks, and diacritics need more
than one keystroke.

**IM framework**
The plumbing an input method uses to talk to apps. On Linux this is
usually **IBus** or **Fcitx5**. Only one should be active at a time.

**IBus**
Intelligent Input Bus. GNOME's default IM framework. Simple; works
best on X11 and modern Wayland GNOME.

**Fcitx5**
Popular alternative to IBus. Better Wayland story on KDE Plasma and
Sway; has richer add-on ecosystem.

**Engine**
The per-language module inside a framework. For Vietnamese: `Bamboo`,
`Unikey`, `TCVN`, etc. Installing an engine's package does not
automatically register it — you often still need `ibus-setup` or
`fcitx5-configtool`.

**Ozone / Ozone Platform**
Chromium's abstraction layer for the windowing system. Pass
`--ozone-platform=wayland` to run an Electron or Chromium app natively
on Wayland; otherwise it falls back to XWayland, which routes IME
events through a stub that often drops tone marks.

**XWayland**
Compatibility layer that lets X11 apps run on Wayland compositors.
IME input through XWayland is unreliable — most Vietnamese typing
bugs trace back here.

**Session (X11 / Wayland)**
The display server protocol your desktop talks. `$XDG_SESSION_TYPE`
is where Doctor reads the answer.

**Locale**
The language + encoding your shell advertises. The
**`LC_ALL` / `LC_CTYPE` / `LANG`** variables pick it; Doctor needs a
UTF-8 locale or characters outside ASCII break in unrelated ways.

**Environment variables Doctor looks at**

* `GTK_IM_MODULE` — which IM GTK3/GTK4 apps use (`ibus` / `fcitx`).
* `QT_IM_MODULE` — same, for Qt apps.
* `XMODIFIERS` — legacy X11 selector (`@im=ibus` / `@im=fcitx`).
* `SDL_IM_MODULE` — games and SDL apps.
* `GLFW_IM_MODULE` — GLFW-backed apps.
* `CLUTTER_IM_MODULE` — GNOME Clutter apps.
* `INPUT_METHOD` — legacy catch-all; rarely needed.

**VD### / VR###**
Stable identifiers Doctor prints for checks and recommendations. Safe
to grep in bug reports — they don't change across patch releases.

**Redaction**
The default privacy pass that scrubs `$USER`, `$HOSTNAME`, and
home-directory paths from the report. Disabled with `--no-redact`.

**Snap / Flatpak / AppImage**
Sandboxed packaging formats. Each has its own IM-portal story:
Flatpak needs the `org.freedesktop.portal.IBus` portal; Snap's IME
story is fragile and is why Doctor surfaces VD010.
