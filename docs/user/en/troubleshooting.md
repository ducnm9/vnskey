<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Troubleshooting Vietnamese Input on Linux

## Frequently Asked Questions

### 1. Cannot type Vietnamese in any application

**Symptom**: Telex input produces plain ASCII (e.g. `aa` outputs `aa` instead of `â`).

**Common causes**:
- IME not installed (ibus-bamboo or fcitx5-bamboo).
- Environment variables `GTK_IM_MODULE`, `QT_IM_MODULE`, `XMODIFIERS` not set.
- IME daemon not running.

**Fix**:
```bash
# Run Doctor to see specific issues
vietime-doctor report

# Or install automatically
vietime-installer install fcitx5-bamboo
```

### 2. Works in gedit but not in Firefox/Chrome

**Cause**: Browser may use a different IM framework or missing environment variables.

**Fix**:
```bash
vietime-doctor report --app firefox
# Check VD003 (EnvVarMismatch) and VD007 (ElectronWaylandNoOzone)
```

### 3. VS Code / Electron apps don't accept IME input

**Cause**: Electron on Wayland requires `--enable-wayland-ime` or `--ozone-platform=wayland`.

**Fix**:
```bash
vietime-doctor report --app vscode
# Check VD007 and VD010
```

If VS Code was installed via Snap:
```bash
# Snap sandbox blocks IM modules. Switch to .deb:
sudo snap remove code
sudo apt install code  # from Microsoft repo
```

### 4. Tone marks placed incorrectly

**Symptom**: Typing `tieesng` produces `tiến` instead of `tiếng`.

**Fix**: Update to the latest engine version:
```bash
vietime-installer install ibus-bamboo
```

### 5. Vietnamese input works in terminal but not in GUI apps

**Cause**: Terminals use direct X11 input while GUI apps use IM modules via GTK/Qt.

**Fix**: Check environment variables:
```bash
echo $GTK_IM_MODULE    # should be "ibus" or "fcitx"
echo $QT_IM_MODULE     # should be "ibus" or "fcitx"
echo $XMODIFIERS       # should be "@im=ibus" or "@im=fcitx"
```

### 6. Installed IME but still can't type — need restart?

**Yes**. After installing an IME, you need to **log out and back in** (or restart) for environment variables to take effect.

### 7. IBus and Fcitx5 running simultaneously — conflict

**Symptom**: Doctor reports VD002 (ImFrameworkConflict).

**Fix**:
```bash
# Choose one, the installer removes the other
vietime-installer install fcitx5-bamboo
```

### 8. Flatpak apps don't accept IME input

**Cause**: Flatpak sandbox requires an IM portal.

**Fix**: Install `xdg-desktop-portal-gtk` (for IBus) or `fcitx5-frontend-gtk3` (for Fcitx5). Doctor reports VD011 if missing.

### 9. Wayland session — IBus is unstable

**Cause**: IBus has known bugs on some Wayland compositors.

**Recommendation**: Switch to Fcitx5 on Wayland:
```bash
vietime-installer install fcitx5-bamboo
```

See the compatibility matrix at [bench.md](bench.md).

### 10. LibreOffice doesn't accept IME input

**Fix**:
```bash
echo 'SAL_USE_VCLPLUGIN=gtk3' >> ~/.profile
# Log out and back in
```

### 11. Chromium/Chrome on Wayland

**Fix**: Add flags to `~/.config/chromium-flags.conf` or `chrome-flags.conf`:
```
--enable-wayland-ime
--ozone-platform=wayland
```

### 12. Benchmark shows low accuracy — should I worry?

Accuracy below 95% means the combo has real bugs. See [bench.md](bench.md) to choose the best combo for your application.

### 13. Want to contribute test vectors

See [contributing-test-vectors.md](../vi/contributing-test-vectors.md).

### 14. Does Doctor report leak personal information?

**No** by default. Doctor automatically redacts username, hostname, and home directory paths. Use `--no-redact` only when sharing with a maintainer for debugging.

### 15. "ibus-daemon not running" but ibus is installed

**Fix**:
```bash
ibus-daemon -drx   # start the daemon
# To auto-start on login:
cp /usr/share/applications/ibus-daemon.desktop ~/.config/autostart/
```

### 16. Fedora/RHEL — fcitx5-bamboo not in official repos

Use COPR:
```bash
sudo dnf copr enable phuongdong/fcitx5-bamboo
sudo dnf install fcitx5-bamboo
```

### 17. Arch Linux — install from AUR

```bash
yay -S ibus-bamboo  # or fcitx5-bamboo
```

### 18. How to check if IME is active?

```bash
# IBus
ibus engine     # shows active engine

# Fcitx5
fcitx5-remote -n   # shows current input method
```

### 19. Fast typing drops characters

**Cause**: Inter-key delay too low for the IME to process.

**Suggestion**: In ibus-bamboo settings, increase "Delay" to 30-50ms.

### 20. Want to test IME across multiple apps at once

Use vietime-bench:
```bash
vietime-bench run --profile smoke
vietime-bench report --format markdown
```

## Need more help?

- Open an issue on GitHub with `vietime-doctor report --json` output attached.
- Check the [compatibility matrix](bench.md) for the best combo.
