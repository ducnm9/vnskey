<!--
SPDX-License-Identifier: GPL-3.0-or-later
-->

# VietIME Installer — user guide (English)

`vietime-installer` is the one-click setup tool for Vietnamese typing on
Linux. Pick a combo (e.g. Fcitx5 + Bamboo), and the installer takes
care of package installation, environment variables, service enablement,
and `im-config` registration — all backed by a snapshot store so you can
roll back any time.

> **Scope:** Phase 2 ships the `install` / `uninstall` / `switch`
> workflow for four MVP combos on Debian-family distros (Ubuntu, Pop!_OS,
> Debian). Fedora and Arch support lands in v0.2 (INS-50 / INS-51).

## Quick start

```bash
# See what the tool will do, without touching the system.
vietime-installer install fcitx5-bamboo --dry-run

# Install interactively (you'll be asked for sudo once).
vietime-installer install fcitx5-bamboo

# Or pick via the wizard.
vietime-installer install

# Verify the result.
vietime-installer verify
```

The happy-path sequence ends with a one-line reminder to log out and log
back in — this is the only manual step, because GTK/Qt pick up the new
env vars at session start.

## Commands

| Command | What it does |
|---------|--------------|
| `install [combo]`          | Plan + execute; runs the wizard when `combo` is omitted |
| `uninstall`                | Roll back the most recent snapshot (alias for `rollback`) |
| `switch <combo>`           | Uninstall the active combo, then install the new one |
| `rollback [--to ID] [--force]` | Undo a specific snapshot, or the latest; `--force` overrides `incomplete=true` manifests |
| `snapshots`                | List every snapshot on disk, newest first |
| `status`                   | One-line summary of the currently-active snapshot |
| `verify`                   | Shell out to `vietime-doctor check` (exit 0/1/2) |
| `list`                     | List the combos this build supports |
| `doctor [args…]`           | Pass arguments through to `vietime-doctor` |
| `version`                  | Print the installer version |
| `hello`                    | Smoke check (prints version + the `vietime-core` banner) |

### Global flags

* `--dry-run` — plan the work but don't mutate anything. Every step prints
  its intended action; no snapshot is written.
* `-y`, `--yes` — skip every confirmation prompt. Pairs with cached sudo
  credentials (run `sudo -v` first) for fully-unattended CI use.
* `-v`, `--verbose` — extra tracing on stderr.
* `--log-file PATH` — append run logs to `PATH` instead of stderr.

## Exit codes

| Code | Meaning |
|------|---------|
| 0    | Success |
| 64   | Usage error (bad flag / unknown combo) |
| 70   | Internal error (snapshot I/O, package manager failure, Ctrl+C) |

## Supported combos

The four MVP combos are:

| Slug               | Framework | Engine  | Packages (Ubuntu/Debian) |
|--------------------|-----------|---------|---------------------------|
| `fcitx5-bamboo`    | Fcitx5    | Bamboo  | `fcitx5`, `fcitx5-bamboo` |
| `fcitx5-unikey`    | Fcitx5    | Unikey  | `fcitx5`, `fcitx5-unikey` |
| `ibus-bamboo`      | IBus      | Bamboo  | `ibus`, `ibus-bamboo`     |
| `ibus-unikey`      | IBus      | Unikey  | `ibus`, `ibus-unikey`     |

Fcitx5 is strongly recommended on Wayland sessions. IBus combos are
offered for users on X11/GNOME who want the stock stack.

## Snapshots & rollback

Every mutating run writes a snapshot manifest to
`~/.config/vietime/snapshots/<timestamp>/` plus any file backups needed
to restore the system. The layout is:

```
~/.config/vietime/snapshots/
├── 2026-04-26T10-15-00Z/
│   ├── manifest.toml            # plan + artifacts + incomplete flag
│   └── files/
│       ├── etc_environment.bak
│       └── etc_environment.bak.sha256
└── latest -> 2026-04-26T10-15-00Z
```

* `manifest.toml` records the plan that was executed, the list of
  artifacts (backups, installed packages, service state changes), and an
  `incomplete = true` bit that's flipped to `false` only on a clean
  finish. A SIGKILL between writes leaves an `incomplete` manifest —
  `rollback` will refuse to touch it without `--force`.
* `latest` points at the most recent snapshot, so `uninstall` /
  `rollback` can find it without arguments.
* SHA-256 sidecar files guard against bit-rot when restoring backups.

### Typical rollback flows

```bash
# Undo the last install.
vietime-installer uninstall

# List snapshots, then roll back to a specific one.
vietime-installer snapshots
vietime-installer rollback --to 2026-04-24T09-31-00Z

# Force rollback of an interrupted run.
vietime-installer rollback --force
```

## Sudo handling

The installer never caches a password and never shells out with piped
stdin. It uses `sudo`'s own credential cache: on the first mutating
step, `sudo -v` prompts once on your TTY; subsequent package-manager
calls reuse the cached grant.

* `--yes` switches to `sudo -n` (non-interactive). If sudo would
  prompt, the installer exits with an error asking you to run `sudo -v`
  first.
* No plan that only touches your home directory (e.g. `~/.profile`,
  `systemctl --user`) asks for sudo at all.

## Typical workflows

**"I just installed Ubuntu, set me up with Fcitx5-Bamboo"**

```bash
vietime-installer install fcitx5-bamboo
# … one sudo prompt, ~30 seconds …
# Log out, log back in, type away.
```

**"Switch me from IBus to Fcitx5"**

```bash
vietime-installer switch fcitx5-bamboo
```

This rolls back your current snapshot (reverting the IBus env vars)
and then installs the new combo atomically.

**"CI wants to verify a headless image"**

```bash
sudo -v                                 # prime the credential cache
vietime-installer install fcitx5-bamboo --yes
vietime-installer verify                # exit 0 on success
```

**"Something went wrong, give me my old machine back"**

```bash
vietime-installer uninstall
```

## Troubleshooting

* **"no snapshots found"** — nothing was ever installed by VietIME on this
  machine. `status` and `uninstall` both report this cleanly.
* **"snapshot `…` is flagged incomplete"** — a previous run was killed
  mid-flight. Inspect `~/.config/vietime/snapshots/<id>/manifest.toml`
  for context, then re-run with `--force` to roll it back.
* **Package manager errors** — the installer surfaces the raw stderr
  from `apt-get`. Common cases: no network, broken
  `/etc/apt/sources.list`, held packages. Fix those, then re-run
  `install`.
* **Still can't type Vietnamese after install** — run `vietime-doctor`
  to see which check fires. VD001 / VD002 / VD003 cover the 80% cases.

## Getting help

* File bugs: <https://github.com/vietime/vietime/issues>
* Discussion: <https://github.com/vietime/vietime/discussions>
* Doctor user guide: [README.md](README.md)
