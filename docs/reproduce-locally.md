# Reproduce Bench Results Locally

This guide shows how to run the VietIME Bench compatibility matrix on your own
machine or in a Docker container.

## Prerequisites

- Rust 1.75+
- Linux with X11 or Wayland
- `xvfb`, `openbox`, `xdotool`, `xclip` (for X11 session)
- `weston`, `ydotool` or `wtype` (for Wayland session)
- `ibus` or `fcitx5` with a Vietnamese engine (e.g. `ibus-bamboo`, `fcitx5-bamboo`)
- Target app(s): `gedit`, `kate`, `firefox`, etc.

## Quick Start

```bash
# Build
cargo build --release -p vietime-bench

# Run smoke profile (3 apps × 1 engine × X11)
./target/release/vietime-bench run --profile smoke

# Run a specific combo
./target/release/vietime-bench run \
  --engine ibus-bamboo \
  --app gedit \
  --session x11 \
  --mode telex

# View results
./target/release/vietime-bench report --format markdown
```

## Docker Compose

```yaml
version: "3.8"
services:
  bench:
    image: ubuntu:22.04
    privileged: true
    volumes:
      - .:/workspace
    working_dir: /workspace
    command: |
      bash -c "
        apt-get update && apt-get install -y \
          curl build-essential pkg-config \
          xvfb openbox xdotool xclip \
          ibus ibus-daemon \
          gedit kate firefox &&
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y &&
        source ~/.cargo/env &&
        cargo build --release -p vietime-bench &&
        ./target/release/vietime-bench run --profile smoke
      "
```

Run with:

```bash
docker compose run --rm bench
```

## Reproduce a Specific Failed Vector

```bash
# 1. Find the failure
./target/release/vietime-bench inspect <run-id> <vector-id>

# 2. The output includes a reproducer command, e.g.:
./target/release/vietime-bench run \
  --engine ibus-bamboo --app vscode --session x11 --mode telex

# 3. Or run just the vector manually:
# Start Xvfb + openbox + ibus-daemon + gedit, then:
xdotool type --delay 30 "tieesng Vieejt"
# Expected output in gedit: "tiếng Việt"
```

## Validate Test Vectors

```bash
./target/release/vietime-bench validate
# Checks: unique IDs, NFC normalization, non-empty fields
```

## Profiles

| Profile | Description | Combos | Est. time |
|---------|-------------|--------|-----------|
| smoke   | 3 apps × 1 engine × X11 | 3 | ~5 min |
| full    | 6 apps × 4 engines × 2 sessions | 48 | ~2 hr |
| bugs    | Regression vectors only | 4 | ~3 min |

Custom profiles can be placed in `profiles/<name>.toml`.
