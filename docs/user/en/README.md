<!--
SPDX-License-Identifier: GPL-3.0-or-later
-->

# VietIME Doctor — user guide (English)

`vietime-doctor` inspects your Linux desktop for the things that make
Vietnamese typing break: missing input-method daemons, mismatched
environment variables, Electron apps that don't route IME events, and
so on. It never changes your system — it only reports.

> **Scope:** Phase 1 ships the Doctor (diagnose only). The Installer
> (automated fixes) lands in Phase 2.

## Quick start

```bash
vietime-doctor          # full Markdown report to stdout
vietime-doctor check    # 1-line status + exit code (0/1/2)
vietime-doctor list     # every detector and checker this build knows
```

Exit codes:

| Code | Meaning |
|------|---------|
| 0    | All clear (or Info-only findings) |
| 1    | Warnings present |
| 2    | Errors or Critical issues present |
| 64   | Usage error (bad flag / subcommand) |
| 70   | Internal error (Doctor itself crashed) |

## Output formats

* `--plain` — strips Markdown so the report is readable in a bare tty.
* `--json` — the stable wire format. Validated against
  [`schemas/report.v1.json`](../../../schemas/report.v1.json); safe to
  parse in CI.
* `--verbose` — appends a one-line summary footer with schema version
  and issue counts.

## The redactor (privacy)

By default, `vietime-doctor` scrubs your username, hostname, and raw
home paths from the report. When you share a report in a bug tracker,
you're sharing a redacted version. Pass `--no-redact` to see the raw
output — Doctor prints a warning on stderr so you never forget the
escape hatch is on.

## Check catalogue

Phase 1 ships 15 checks (VD001 – VD015). Every check has a stable id,
a severity, and (for Warn/Error/Critical) a VR### recommendation with
concrete shell commands.

| ID | Severity | Trigger |
|----|----------|---------|
| VD001 | Critical | Vietnamese engine installed but no IM daemon running |
| VD002 | Error    | Both IBus and Fcitx5 daemons running at the same time |
| VD003 | Error    | IM env vars disagree with the active framework |
| VD004 | Warn     | `SDL_IM_MODULE` is unset |
| VD005 | Warn     | Engine is installed but not registered with its framework |
| VD006 | Warn     | IBus on Wayland (Fcitx5 is more reliable there) |
| VD007 | Error    | Electron app launched without `--ozone-platform=wayland` |
| VD008 | Warn     | Chrome/Chromium on Wayland without Ozone |
| VD009 | Warn     | Same env var set to different values in two config files |
| VD010 | Warn     | VS Code installed via Snap (IME events are confined) |
| VD011 | Warn     | Flatpak app without the IBus/Fcitx IM portal |
| VD012 | Info     | `INPUT_METHOD` is unset (legacy hint) |
| VD013 | Warn     | Fcitx5 missing a platform-appropriate addon (wayland-im / xim) |
| VD014 | Warn     | Active locale is not a UTF-8 one |
| VD015 | Info     | No Vietnamese engine installed at all |

Recommendation ids are `VR001`…`VR014` (no VR012 / VR015 — Info-level
checks have no auto-fix suggestions).

## Typical workflows

**"Why doesn't my Vietnamese typing work?"**

```bash
vietime-doctor | less
```

Read the `## Checks` section top-down. Critical/Error rows point at the
#1 cause; the Recommendations section has the exact command to fix it.

**"Is my setup sane?" (CI check)**

```bash
vietime-doctor check
# exit 0 / 1 / 2 matches your CI's severity budget
```

**"Something's off with VS Code specifically"**

```bash
vietime-doctor --app vscode
```

This gates the `app.generic` and `app.electron` detectors on — Doctor
inspects the running process for Ozone flags and reports VD007/VD010
accordingly.

## Getting help

* File bugs: <https://github.com/vietime/vietime/issues>
* Discussion: <https://github.com/vietime/vietime/discussions>
* Check the [glossary](GLOSSARY.md) for acronyms.
