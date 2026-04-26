# VietIME Bench — Vietnamese Input Compatibility Matrix for Linux

## Overview

VietIME Bench is an automated tool that tests Vietnamese text input across
Linux applications. It produces a **compatibility matrix** showing how
accurately each combination of (IME engine × application × session × input
mode) works.

## Reading the Matrix

The dashboard displays:

| Column | Meaning |
|--------|---------|
| Engine | IME combo: `ibus-bamboo`, `fcitx5-bamboo`, `ibus-unikey`, `fcitx5-unikey` |
| App | Target application: gedit, kate, firefox, chromium, vscode, libreoffice, … |
| Session | Display session: `x11` or `wayland` |
| Mode | Input method: `telex`, `vni`, `viqr`, `simple-telex` |
| Accuracy | Exact match percentage — vectors matched perfectly / total vectors |
| Exact | Exact match count / total |
| Edit Dist | Total Levenshtein edit distance across all mismatches |

### Colour coding

- **Green (≥95%)**: Works well, safe to use.
- **Yellow (80-95%)**: Minor issues, verify for your use case.
- **Red (<80%)**: Significant problems, avoid this combo.

## Usage

```bash
# Install
cargo install vietime-bench

# Quick run
vietime-bench run --profile smoke

# View results
vietime-bench report --format markdown

# Compare two runs
vietime-bench compare --base <run-1> --head <run-2>

# Inspect a failure
vietime-bench inspect <run-id> <vector-id>
```

## Test Vectors

The current vector set includes 500+ Vietnamese sentences covering:
- Tone marks: acute, grave, hook above, tilde, dot below
- Modifiers: circumflex (â, ê, ô), horn (ơ, ư), breve (ă), stroke (đ)
- Combined tone + modifier: ấ, ầ, ẩ, ẫ, ậ, …
- Common words: tiếng Việt, xin chào, đường, người, …
- Edge cases: mixed numbers, uppercase, punctuation

## Contributing Test Vectors

See [contributing-test-vectors.md](../vi/contributing-test-vectors.md).

## FAQ

**Q: Why isn't accuracy 100%?**
A: Some engine + app combinations have real bugs (e.g. Electron + IBus on
Wayland). The bench faithfully records the actual behaviour.

**Q: Which combo should I pick?**
A: Check the matrix for your primary application. Generally `fcitx5-bamboo`
on X11 has the most stable results.
