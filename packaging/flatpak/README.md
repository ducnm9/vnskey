<!--
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Flatpak packaging

This directory holds the Flatpak manifest for `vietime-doctor`.

## Files

| File | Purpose |
| ---- | ------- |
| `io.vietime.Doctor.yml` | `flatpak-builder` manifest — builds Doctor against the Freedesktop 23.08 runtime with the `rust-stable` SDK extension. |
| `cargo-sources.json` | **Generated.** Transitive Cargo dependency tree; keeps the Flatpak build hermetic. Regenerate on `Cargo.lock` change. |

`cargo-sources.json` lives next to the manifest (not in this directory yet — it is produced out-of-band before each Flathub submission).

## Why Flatpak for a CLI?

Most Flatpaks ship GUI apps, but a diagnostic tool benefits from the sandbox too:

- Users on Silverblue/Kinoite (Fedora atomic) have no other way to install a CLI without layering.
- The sandbox proves the Doctor is read-only — users can audit `finish-args` and see there is no write access, no network, no device access.
- A Flatpak build catches ABI drift: if the Doctor starts depending on a glibc symbol newer than the Freedesktop runtime's, CI fails.

## Building locally

```sh
# Prerequisites (Ubuntu / Fedora / Arch all install these from their usual repos):
# - flatpak
# - flatpak-builder
# - org.freedesktop.Sdk//23.08   (flatpak install org.freedesktop.Sdk//23.08)
# - org.freedesktop.Sdk.Extension.rust-stable//23.08

# Generate Cargo sources (one-time per Cargo.lock change):
pip install --user toml requests aiohttp
curl -O https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
python3 flatpak-cargo-generator.py ../../Cargo.lock -o cargo-sources.json

# Build and install into the user Flatpak:
flatpak-builder --user --install --force-clean build-dir io.vietime.Doctor.yml

# Run:
flatpak run io.vietime.Doctor report --plain
flatpak run io.vietime.Doctor check
```

## Flathub submission checklist

- [ ] Tag v0.1.0 cut (AUR-bin and cargo-deb artefacts already signed).
- [ ] `cargo-sources.json` regenerated and committed for the release commit.
- [ ] `type: dir` swapped for `type: archive` + `url: https://github.com/vietime/vietime/archive/refs/tags/v0.1.0.tar.gz` + `sha256: …`.
- [ ] Test on Silverblue 40 and Kinoite 40 (IBus + Fcitx5 paths, Wayland session).
- [ ] Submit PR to `flathub/flathub` with the manifest + a short description.
