# vietime-fuzz

Fuzz harnesses for parsers that ingest untrusted-but-usually-well-formed
system files (`/etc/environment`, `/etc/os-release`).

## Why a separate crate?

`cargo-fuzz` needs nightly Rust and injects its own `libfuzzer-sys` build
flags, which clash with the top-level workspace's stable lint policy. This
crate lives outside `[workspace].members` so the main `cargo test` matrix
stays clean.

## Setup

```bash
rustup install nightly
cargo install cargo-fuzz
```

## Running a target

```bash
cd fuzz
cargo +nightly fuzz run parse_etc_environment -- -runs=1000000
cargo +nightly fuzz run os_release            -- -runs=1000000
```

Each target has an `assert!` inside the body that encodes the parser's
documented invariants — any panic from libFuzzer means we found either a
real bug or the invariants need updating.

## CI

GitHub Actions runs each target for 60 s on every PR and on a nightly
schedule (see `.github/workflows/fuzz.yml`). The pipeline does NOT gate
merges on fuzz hits — false-positive seeds from upstream clap / regex
churn would make the project miserable. Maintainers triage hits manually.
